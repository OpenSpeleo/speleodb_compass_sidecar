use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn updater_smoke_dispatch_fails_without_starting_the_desktop() {
    let dir = tempfile::tempdir().unwrap();
    let assets = dir.path().join("assets.json");
    std::fs::write(&assets, "{}").unwrap();

    for endpoint in [None, Some("https://example.com/latest.json")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_speleodb-compass-sidecar"));
        command
            .arg("--verify-updater-runtime")
            .env_remove("SIDECAR_UPDATER_SMOKE_ENDPOINT")
            .env("SIDECAR_UPDATER_SMOKE_TARGETS", "smoke-target")
            .env("SIDECAR_UPDATER_SMOKE_ASSETS", &assets)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(endpoint) = endpoint {
            command.env("SIDECAR_UPDATER_SMOKE_ENDPOINT", endpoint);
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("updater smoke did not terminate before desktop startup");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            stderr.contains("Updater runtime verification failed:"),
            "{stderr}"
        );
        let expected = if endpoint.is_some() {
            "endpoint must use http://127.0.0.1"
        } else {
            "SIDECAR_UPDATER_SMOKE_ENDPOINT must be set"
        };
        assert!(stderr.contains(expected), "{stderr}");
    }
}
