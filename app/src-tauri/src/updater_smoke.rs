//! Verify updater downloads with the shipping binary, before desktop startup.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::Url;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri_plugin_updater::UpdaterExt;

const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub(crate) enum SmokeError {
    #[error("{0}")]
    Config(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Download and verify locally served release artifacts without installing them.
pub(crate) fn verify() -> Result<(), SmokeError> {
    let endpoint = required_env("SIDECAR_UPDATER_SMOKE_ENDPOINT")?;
    let targets = required_env("SIDECAR_UPDATER_SMOKE_TARGETS")?;
    let assets_path = required_env("SIDECAR_UPDATER_SMOKE_ASSETS")?;
    let assets: HashMap<String, PathBuf> = serde_json::from_reader(File::open(assets_path)?)
        .map_err(|error| {
            SmokeError::Config(format!("parse updater smoke asset manifest: {error}"))
        })?;
    let config: serde_json::Value = serde_json::from_str(TAURI_CONF)
        .map_err(|error| SmokeError::Config(format!("parse tauri.conf.json: {error}")))?;
    let updater_config = config["plugins"]["updater"].clone();
    let targets = targets
        .split(',')
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .collect::<Vec<_>>();
    verify_updates(
        &endpoint,
        &targets,
        &assets,
        updater_config,
        REQUEST_TIMEOUT,
    )
}

fn required_env(name: &str) -> Result<String, SmokeError> {
    std::env::var(name).map_err(|error| {
        SmokeError::Config(format!("{name} must be set for updater smoke: {error}"))
    })
}

fn local_endpoint(endpoint: &str) -> Result<Url, SmokeError> {
    let url: Url = endpoint
        .parse()
        .map_err(|error| SmokeError::Config(format!("invalid updater smoke endpoint: {error}")))?;
    if !endpoint.starts_with("http://127.0.0.1:")
        || url.host_str() != Some("127.0.0.1")
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.port(), Some(port) if port != 0)
    {
        return Err(SmokeError::Config(
            "updater smoke endpoint must use http://127.0.0.1 with an explicit nonzero port".into(),
        ));
    }
    Ok(url)
}

fn verify_updates(
    endpoint: &str,
    targets: &[&str],
    assets: &HashMap<String, PathBuf>,
    mut updater_config: serde_json::Value,
    request_timeout: Duration,
) -> Result<(), SmokeError> {
    let endpoint = local_endpoint(endpoint)?;
    if targets.is_empty() || targets.iter().any(|target| target.is_empty()) {
        return Err(SmokeError::Config(
            "updater smoke must specify at least one nonempty target".into(),
        ));
    }
    if assets.is_empty() || assets.values().any(|path| !path.is_absolute()) {
        return Err(SmokeError::Config(
            "updater smoke assets must map artifact names to absolute paths".into(),
        ));
    }
    let config = updater_config.as_object_mut().ok_or_else(|| {
        SmokeError::Config("plugins.updater must be a configuration object".into())
    })?;
    // Release-mode updater clients reject HTTP. Enable it only in this headless
    // context, after restricting the endpoint to the local smoke server.
    config.insert("dangerousInsecureTransportProtocol".into(), true.into());

    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), updater_config);
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .map_err(|error| SmokeError::Config(format!("build updater smoke context: {error}")))?;

    tauri::async_runtime::block_on(async {
        for target in targets {
            let updater = app
                .updater_builder()
                .target(*target)
                .version_comparator(|_, _| true)
                .no_proxy()
                .timeout(request_timeout)
                // UpdaterBuilder's timeout is not copied into Update.timeout.
                // Configure both clients so a stalled package also times out.
                .configure_client(move |client| {
                    client
                        .timeout(request_timeout)
                        .redirect(reqwest::redirect::Policy::none())
                })
                .endpoints(vec![endpoint.clone()])
                .map_err(|error| SmokeError::Config(format!("set smoke endpoint: {error}")))?
                .build()
                .map_err(|error| {
                    SmokeError::Config(format!("build updater for {target}: {error}"))
                })?;
            let update = updater
                .check()
                .await
                .map_err(|error| {
                    SmokeError::Config(format!("check updater metadata for {target}: {error}"))
                })?
                .ok_or_else(|| {
                    SmokeError::Config(format!("expected update metadata for target {target}"))
                })?;
            if update.download_url.origin() != endpoint.origin() {
                return Err(SmokeError::Config(format!(
                    "target {target} download must use the local smoke server"
                )));
            }
            let asset_name = update
                .download_url
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    SmokeError::Config(format!("target {target} URL has no asset name"))
                })?;
            let expected_path = assets.get(asset_name).ok_or_else(|| {
                SmokeError::Config(format!(
                    "target {target} artifact {asset_name} is absent from the smoke asset manifest"
                ))
            })?;

            let mut downloaded = 0usize;
            let bytes = update
                .download(|length, _| downloaded += length, || {})
                .await
                .map_err(|error| {
                    SmokeError::Config(format!("download and verify target {target}: {error}"))
                })?;
            if bytes.is_empty() || bytes.len() != downloaded {
                return Err(SmokeError::Config(format!(
                    "target {target} download must be nonempty and match its progress count"
                )));
            }
            compare_artifact(&bytes, expected_path)?;
            println!("Verified updater target {target}: {asset_name}");
        }
        Ok(())
    })
}

fn compare_artifact(bytes: &[u8], path: &Path) -> Result<(), SmokeError> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() != bytes.len() as u64 {
        return Err(SmokeError::Config(format!(
            "downloaded updater artifact size differs from {}",
            path.display()
        )));
    }
    // The updater plugin owns the full download buffer. Compare against the
    // original artifact in chunks instead of allocating another artifact copy.
    let mut buffer = [0; 64 * 1024];
    for chunk in bytes.chunks(buffer.len()) {
        file.read_exact(&mut buffer[..chunk.len()])?;
        if chunk != &buffer[..chunk.len()] {
            return Err(SmokeError::Config(format!(
                "downloaded updater bytes differ from {}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread::{self, JoinHandle};

    // Retain the original fixture bytes: the inherited signature binds this payload.
    const PAYLOAD: &[u8] = b"FrameForge updater smoke fixture\n";
    // Disposable fixture key generated with `tauri signer`; no private key is kept.
    const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQzREM3ODdCQkJCRTgzNUEKUldSYWc3NjdlM2pjMDZmR2pKVEUzTWFMRFcvU2RGMHJ1YjlNV2JxQk9Va1dHZDN3TElBQXlLcHEK";
    const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSYWc3NjdlM2pjMDNaMlFzNGZzRE1XQVBxMVlhcGpnVzNib2w5S1pFZ0J5THNMMlV3S2VNR0hHUk15U0wzb0piWGRpTW5hRDFnTzJKdWpOUjdmTnpzS2FTdXNuQWJBZVFJPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkyMTc2NTQyCWZpbGU6cGF5bG9hZC5iaW4JdmVyc2lvbjo5OTkuMC4wCkR6VENNOFFCZ0dkdDFMUDFxNnpWM3RTMHQzUlJya2hqWHEvdWZqeHFXblZsU2p1VnpMK2pwVDV6RGxpcVZHc0FxdTRmMkZFZFNMNlR5QUhkZEpFOURRPT0K";

    struct Server {
        endpoint: String,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Server {
        fn start(signature: &str, payload: &[u8], valid_metadata: bool) -> Self {
            Self::start_named(signature, payload, valid_metadata, "payload.bin")
        }

        fn start_named(
            signature: &str,
            payload: &[u8],
            valid_metadata: bool,
            asset_name: &str,
        ) -> Self {
            Self::start_with_stalled_download(signature, payload, valid_metadata, asset_name, false)
        }

        fn start_with_stalled_download(
            signature: &str,
            payload: &[u8],
            valid_metadata: bool,
            asset_name: &str,
            stall_download: bool,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let metadata = if valid_metadata {
                serde_json::to_vec(&serde_json::json!({
                    "version": "999.0.0",
                    "platforms": {
                        "smoke-target": {"url": format!("{base}/{asset_name}"), "signature": signature},
                        "smoke-alias": {"url": format!("{base}/{asset_name}"), "signature": signature}
                    }
                }))
                .unwrap()
            } else {
                b"invalid metadata".to_vec()
            };
            let payload = payload.to_vec();
            let stop = Arc::new(AtomicBool::new(false));
            let stopping = stop.clone();
            let thread = thread::spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    let (stream, _) = match listener.accept() {
                        Ok(connection) => connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(error) => panic!("accept fixture request: {error}"),
                    };
                    serve_request(
                        stream,
                        &metadata,
                        &payload,
                        stall_download.then_some(stopping.as_ref()),
                    )
                    .expect("serve fixture request");
                }
            });
            Self {
                endpoint: format!("{base}/latest.json"),
                stop,
                thread: Some(thread),
            }
        }
    }

    fn serve_request(
        mut stream: TcpStream,
        metadata: &[u8],
        payload: &[u8],
        stall_download: Option<&AtomicBool>,
    ) -> std::io::Result<()> {
        // Accepted sockets can inherit the listener's nonblocking mode on
        // macOS and Windows. Timeouts alone do not make reads block.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut reader = BufReader::new(&stream);
        let mut request = String::new();
        reader.read_line(&mut request)?;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                break;
            }
        }
        let body = if request.starts_with("GET /latest.json ") {
            metadata
        } else {
            payload
        };
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )?;
        if !request.starts_with("GET /latest.json ")
            && let Some(stop) = stall_download
        {
            // Keep the advertised payload incomplete until the test releases
            // the server, including on a timeout-regression assertion failure.
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(5));
            }
            return Ok(());
        }
        stream.write_all(body)
    }

    #[test]
    fn serves_delayed_fragmented_requests_on_nonblocking_streams() {
        for (path, body) in [
            ("/latest.json", b"metadata".as_slice()),
            ("/payload.bin", PAYLOAD),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (stream, _) = listener.accept().unwrap();
            // Reproduce inherited nonblocking mode even on hosts where accept()
            // returns a blocking socket. No request bytes have been sent yet.
            stream.set_nonblocking(true).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let (started_tx, started_rx) = mpsc::channel();
            let (finished_tx, finished_rx) = mpsc::channel();
            let worker = thread::spawn(move || {
                started_tx.send(()).unwrap();
                let result = serve_request(stream, b"metadata", PAYLOAD, None);
                let _ = finished_tx.send(result);
            });
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

            // Delay the first byte and each incomplete request/header fragment.
            // The server must wait for more data rather than return WouldBlock.
            let first = format!("GET {path}");
            for fragment in [
                first.as_bytes(),
                b" HTTP/1.1\r\nHost: 127.0.",
                b"0.1\r\n",
                b"\r\n",
            ] {
                let premature = finished_rx.recv_timeout(Duration::from_millis(50));
                assert!(
                    matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)),
                    "fixture completed before the full request arrived: {premature:?}"
                );
                client.write_all(fragment).unwrap();
            }
            let mut response = Vec::new();
            client.read_to_end(&mut response).unwrap();
            finished_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            worker.join().unwrap();
            let mut expected = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            expected.extend_from_slice(body);
            assert_eq!(response, expected);
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    fn run_fixture(
        signature: &str,
        payload: &[u8],
        valid_metadata: bool,
    ) -> Result<(), SmokeError> {
        let server = Server::start(signature, payload, valid_metadata);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, PAYLOAD).unwrap();
        verify_updates(
            &server.endpoint,
            &["smoke-target", "smoke-alias"],
            &HashMap::from([("payload.bin".into(), path)]),
            serde_json::json!({"pubkey": PUBLIC_KEY, "requireSignedVersion": true}),
            REQUEST_TIMEOUT,
        )
    }

    #[test]
    fn verifies_signed_payload_for_each_target_without_installing() {
        run_fixture(SIGNATURE, PAYLOAD, true).unwrap();
    }

    #[test]
    fn verifies_encoded_artifact_names_with_spaces() {
        let asset_name = "SpeleoDB%20Compass%20Sidecar.app.tar.gz";
        let server = Server::start_named(SIGNATURE, PAYLOAD, true, asset_name);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SpeleoDB Compass Sidecar.app.tar.gz");
        std::fs::write(&path, PAYLOAD).unwrap();
        verify_updates(
            &server.endpoint,
            &["smoke-target", "smoke-alias"],
            &HashMap::from([(asset_name.into(), path)]),
            serde_json::json!({"pubkey": PUBLIC_KEY, "requireSignedVersion": true}),
            REQUEST_TIMEOUT,
        )
        .unwrap();
    }

    #[test]
    fn times_out_stalled_package_downloads_after_metadata_succeeds() {
        let server =
            Server::start_with_stalled_download(SIGNATURE, PAYLOAD, true, "payload.bin", true);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, PAYLOAD).unwrap();
        let endpoint = server.endpoint.clone();
        let (finished_tx, finished_rx) = mpsc::channel();
        let verifier = thread::spawn(move || {
            let result = verify_updates(
                &endpoint,
                &["smoke-target"],
                &HashMap::from([("payload.bin".into(), path)]),
                serde_json::json!({"pubkey": PUBLIC_KEY, "requireSignedVersion": true}),
                Duration::from_millis(500),
            );
            finished_tx.send(result).unwrap();
        });
        let result = finished_rx.recv_timeout(Duration::from_secs(5));
        // Release the stalled response before asserting, so a regression fails
        // within five seconds rather than leaving a hanging fixture thread.
        drop(server);
        verifier.join().unwrap();
        let error = result
            .expect("package download did not honor the request timeout")
            .unwrap_err();
        assert!(error.to_string().contains("download and verify"), "{error}");
    }

    #[test]
    fn rejects_empty_targets_and_invalid_asset_manifests() {
        let endpoint = "http://127.0.0.1:43210/latest.json";
        let config = serde_json::json!({"pubkey": PUBLIC_KEY});
        for targets in [&[][..], &[""][..]] {
            let error = verify_updates(
                endpoint,
                targets,
                &HashMap::new(),
                config.clone(),
                REQUEST_TIMEOUT,
            )
            .unwrap_err();
            assert!(error.to_string().contains("at least one nonempty target"));
        }
        for assets in [
            HashMap::new(),
            HashMap::from([("payload.bin".into(), PathBuf::from("relative.bin"))]),
        ] {
            let error = verify_updates(
                endpoint,
                &["smoke-target"],
                &assets,
                config.clone(),
                REQUEST_TIMEOUT,
            )
            .unwrap_err();
            assert!(error.to_string().contains("absolute paths"));
        }
    }

    #[test]
    fn rejects_invalid_signatures_and_tampered_payloads() {
        for (signature, payload) in [
            ("invalid-signature", PAYLOAD),
            (SIGNATURE, b"tampered payload".as_slice()),
        ] {
            let error = run_fixture(signature, payload, true).unwrap_err();
            assert!(error.to_string().contains("download and verify"), "{error}");
        }
    }

    #[test]
    fn rejects_invalid_metadata() {
        let error = run_fixture(SIGNATURE, PAYLOAD, false).unwrap_err();
        assert!(error.to_string().contains("check updater metadata"));
    }

    #[test]
    fn restricts_insecure_transport_to_the_local_smoke_server() {
        local_endpoint("http://127.0.0.1:43210/latest.json").unwrap();
        for endpoint in [
            "http://127.0.0.1/latest.json",
            "http://127.0.0.1:0/latest.json",
            "http://localhost:43210/latest.json",
            "https://127.0.0.1:43210/latest.json",
            "http://127.0.0.1:43210@example.com/latest.json",
            "http://example.com:43210/latest.json",
        ] {
            assert!(local_endpoint(endpoint).is_err(), "accepted {endpoint}");
        }
    }

    #[test]
    fn compares_original_artifacts_across_chunk_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        let bytes = vec![42; 128 * 1024 + 3];
        std::fs::write(&path, &bytes).unwrap();
        compare_artifact(&bytes, &path).unwrap();
        let mut changed = bytes.clone();
        changed[64 * 1024] = 13;
        assert!(compare_artifact(&changed, &path).is_err());
        assert!(compare_artifact(&bytes[..bytes.len() - 1], &path).is_err());
    }
}
