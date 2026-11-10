//! Exercise the public sidecar adapter against real loopback HTTP sockets.
//! These fixtures verify SDK integration and desktop policy, not backend rules.

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use api::{auth, project};
use common::{ApiInfo, Error, api_types::ProjectSaveResult};
use serde_json::{Value, json};
use uuid::Uuid;

const TOKEN: &str = "loopback-sdk-token";
const ID: &str = "9e12fe62-ad38-471b-a625-7ed9960ab3e4";

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    fn accepts_json(&self) -> bool {
        self.header("accept")
            .unwrap_or("*/*")
            .split(',')
            .any(|range| {
                matches!(
                    range.split(';').next().unwrap().trim(),
                    "application/json" | "application/*" | "*/*"
                )
            })
    }

    fn assert_sidecar_headers(&self, authenticated: bool) {
        assert_eq!(
            self.header("user-agent"),
            Some(concat!(
                "Tauri/SpeleoDB-Compass-Sidecar/v",
                env!("CARGO_PKG_VERSION")
            ))
        );
        assert_eq!(
            self.header("authorization"),
            authenticated.then_some("Token loopback-sdk-token")
        );
    }
}

struct Response {
    status: u16,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    json_renderer_gate: bool,
    truncate_body: bool,
}

impl Response {
    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            headers: vec![],
            body: serde_json::to_vec(&body).unwrap(),
            json_renderer_gate: false,
            truncate_body: false,
        }
    }

    fn bytes(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "application/octet-stream",
            headers: vec![],
            body: body.into(),
            json_renderer_gate: false,
            truncate_body: false,
        }
    }

    fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    fn require_json_renderer(mut self) -> Self {
        self.json_renderer_gate = true;
        self
    }

    fn truncated(mut self) -> Self {
        self.truncate_body = true;
        self
    }
}

struct Server {
    address: SocketAddr,
    worker: JoinHandle<Vec<Request>>,
}

impl Server {
    fn new(responses: Vec<Response>) -> Self {
        Self::with_disconnects(responses.into_iter().map(Some).collect())
    }

    fn with_disconnects(responses: Vec<Option<Response>>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let worker = thread::spawn(move || {
            responses
                .into_iter()
                .map(|response| {
                    let deadline = Instant::now() + Duration::from_secs(5);
                    let mut stream = loop {
                        match listener.accept() {
                            Ok((stream, _)) => break stream,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(Instant::now() < deadline, "expected HTTP request");
                                thread::sleep(Duration::from_millis(2));
                            }
                            Err(error) => panic!("loopback accept failed: {error}"),
                        }
                    };
                    // Accepted sockets can inherit the listener's nonblocking
                    // flag on macOS; reads/writes below use bounded blocking I/O.
                    stream.set_nonblocking(false).unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                    stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
                    let request = read_request(&mut stream);
                    let Some(response) = response else {
                        // Close after receiving the request, before any response.
                        return request;
                    };
                    // DRF negotiates its JSON renderer before invoking the
                    // download view, even when that view returns a redirect.
                    let response = if response.json_renderer_gate && !request.accepts_json() {
                        Response::json(406, json!({"detail": "Could not satisfy request Accept header."}))
                    } else {
                        response
                    };
                    write!(
                        stream,
                        "HTTP/1.1 {} Test\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                        response.status,
                        response.content_type,
                        response.body.len()
                    ).unwrap();
                    for (name, value) in response.headers {
                        write!(stream, "{name}: {value}\r\n").unwrap();
                    }
                    stream.write_all(b"\r\n").unwrap();
                    let body = if response.truncate_body {
                        &response.body[..response.body.len() / 2]
                    } else {
                        &response.body
                    };
                    stream.write_all(body).unwrap();
                    request
                })
                .collect()
        });
        Self { address, worker }
    }

    fn url(&self, path: &str) -> url::Url {
        format!("http://{}{path}", self.address).parse().unwrap()
    }

    fn api_info(&self, prefix: &str) -> ApiInfo {
        ApiInfo::new(
            self.url(prefix),
            Some("test@example.invalid".into()),
            Some(TOKEN.into()),
        )
    }

    fn finish(self) -> Vec<Request> {
        self.worker.join().expect("loopback HTTP server failed")
    }
}

fn read_request(stream: &mut TcpStream) -> Request {
    let mut wire = vec![];
    let header_end = loop {
        if let Some(index) = wire.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break index + 4;
        }
        assert!(
            wire.len() < 64 * 1024,
            "request headers exceeded fixture limit"
        );
        let mut chunk = [0; 4096];
        let length = stream.read(&mut chunk).unwrap();
        assert_ne!(length, 0, "request ended before its headers");
        wire.extend_from_slice(&chunk[..length]);
    };
    let head = std::str::from_utf8(&wire[..header_end]).unwrap();
    let mut lines = head.split("\r\n");
    let mut start = lines.next().unwrap().split_whitespace();
    let method = start.next().unwrap().to_owned();
    let path = start.next().unwrap().to_owned();
    let headers: BTreeMap<_, _> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    assert!(
        !headers.contains_key("transfer-encoding"),
        "fixture expects known-length requests"
    );
    let length: usize = headers
        .get("content-length")
        .map_or(0, |length| length.parse().unwrap());
    assert!(length < 1024 * 1024, "unexpectedly large fixture request");
    let mut body = wire[header_end..].to_vec();
    body.resize(length, 0);
    let received = wire.len() - header_end;
    assert!(received <= length);
    stream.read_exact(&mut body[received..]).unwrap();
    Request {
        method,
        path,
        headers,
        body,
    }
}

fn id() -> Uuid {
    ID.parse().unwrap()
}

fn project_json() -> Value {
    json!({
        "id": ID, "name": "Survey", "description": "Compass survey",
        "is_active": true, "permission": "ADMIN", "active_mutex": null,
        "country": "US", "created_by": "test@example.invalid",
        "creation_date": "2026-02-27T10:08:23-05:00",
        "modified_date": "2026-02-27T10:18:12-05:00",
        "fork_from": null, "visibility": "PRIVATE", "exclude_geojson": false,
        "latest_commit": null, "type": "COMPASS", "color": "#4daf4a", "commit_count": 0
    })
}

fn upload_response() -> Value {
    json!({
        "browser_url": null, "files": ["project.zip"],
        "hexsha": "0123456789abcdef0123456789abcdef01234567", "message": "Saved",
        "project": project_json()
    })
}

fn zip_file() -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"PK\x03\x04\0\xffsurvey bytes").unwrap();
    file
}

#[tokio::test]
async fn token_and_email_authorization_use_sdk_routes_and_preserve_credentials() {
    let server = Server::new(vec![
        Response::json(
            200,
            json!({"token": "issued-token", "user": "test@example.invalid"}),
        ),
        Response::json(
            201,
            json!({"token": "email-token", "user": "test@example.invalid"}),
        ),
    ]);
    let instance = server.url("/speleodb/?ignored=1#ignored");
    let token_info = auth::authorize_with_token(instance.clone(), TOKEN)
        .await
        .unwrap();
    assert_eq!(token_info.oauth_token(), Some("issued-token"));
    assert_eq!(token_info.email(), Some("test@example.invalid"));
    assert_eq!(token_info.instance(), &instance);
    let email_info = auth::authorize_with_email(instance, "test@example.invalid", "password")
        .await
        .unwrap();
    assert_eq!(email_info.oauth_token(), Some("email-token"));
    let requests = server.finish();
    for request in &requests {
        assert_eq!(request.path, "/speleodb/api/v2/user/auth-token/");
    }
    requests[0].assert_sidecar_headers(true);
    requests[1].assert_sidecar_headers(false);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[1].method, "POST");
    assert_eq!(requests[1].header("content-type"), Some("application/json"));
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].body).unwrap(),
        json!({"email": "test@example.invalid", "password": "password"})
    );
}

#[tokio::test]
async fn token_authorization_recovers_from_transient_transport_failures() {
    let server = Server::with_disconnects(vec![
        None,
        None,
        Some(Response::json(
            200,
            json!({"token": "issued-token", "user": "test@example.invalid"}),
        )),
    ]);
    let info = auth::authorize_with_token(server.url(""), TOKEN)
        .await
        .unwrap();
    assert_eq!(info.oauth_token(), Some("issued-token"));
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    for request in requests {
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/v2/user/auth-token/");
        request.assert_sidecar_headers(true);
    }
}

#[tokio::test]
async fn token_authorization_stops_after_three_transport_failures() {
    let server = Server::with_disconnects(vec![None, None, None]);
    let error = auth::authorize_with_token(server.url(""), TOKEN)
        .await
        .unwrap_err();
    assert!(error.contains("Network request failed"), "{error}");
    assert!(error.contains("connection closed"), "{error}");
    assert_eq!(server.finish().len(), 3);
}

#[tokio::test]
async fn email_authorization_does_not_retry_transport_failures() {
    let server = Server::with_disconnects(vec![None]);
    let error = auth::authorize_with_email(server.url(""), "test@example.invalid", "password")
        .await
        .unwrap_err();
    assert!(error.contains("Network request failed"), "{error}");
    assert_eq!(server.finish().len(), 1);
}

#[tokio::test]
async fn project_reads_recover_from_transport_and_interrupted_body_failures() {
    let server = Server::with_disconnects(vec![
        None,
        Some(Response::json(200, project_json()).truncated()),
        Some(Response::json(200, project_json())),
        Some(Response::json(200, json!([project_json()])).truncated()),
        Some(Response::json(200, json!([project_json()]))),
    ]);
    let info = server.api_info("");
    assert_eq!(
        project::fetch_project_info(&info, id()).await.unwrap().id,
        id()
    );
    let projects = project::fetch_projects(&info).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, id());
    let requests = server.finish();
    assert_eq!(requests.len(), 5);
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(request.method, "GET");
        assert_eq!(
            request.path,
            if index < 3 {
                format!("/api/v2/projects/{ID}/")
            } else {
                "/api/v2/projects/".into()
            }
        );
        request.assert_sidecar_headers(true);
    }
}

#[tokio::test]
async fn project_metadata_operations_keep_routes_json_and_desktop_projection() {
    let mut ignored = project_json();
    ignored["type"] = "ARIANE".into();
    let mut acquired = project_json();
    acquired["active_mutex"] = json!({
        "user": "test@example.invalid", "creation_date": "2026-02-27T10:08:23-05:00",
        "modified_date": "2026-02-27T10:18:12-05:00"
    });
    let server = Server::new(vec![
        Response::json(200, json!([ignored, project_json()])),
        Response::json(201, project_json()),
        Response::json(201, project_json()),
        Response::json(200, project_json()),
        Response::json(200, acquired),
        Response::json(200, project_json()),
    ]);
    let info = server.api_info("/speleodb");
    let projects = project::fetch_projects(&info).await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, id());
    assert_eq!(projects[0].permission, "ADMIN");
    assert_eq!(projects[0].creation_date, "2026-02-27T15:08:23+00:00");
    project::create_project(
        &info,
        "Survey".into(),
        "Description".into(),
        "US".into(),
        Some("28.5".into()),
        Some("-82.5".into()),
    )
    .await
    .unwrap();
    project::create_project(
        &info,
        "Survey".into(),
        "Description".into(),
        "US".into(),
        Some(String::new()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        project::fetch_project_info(&info, id()).await.unwrap().id,
        id()
    );
    assert!(
        project::acquire_project_mutex(&info, id())
            .await
            .unwrap()
            .active_mutex
            .is_some()
    );
    assert!(
        project::release_project_mutex(&info, id())
            .await
            .unwrap()
            .active_mutex
            .is_none()
    );
    let requests = server.finish();
    for request in &requests {
        request.assert_sidecar_headers(true);
    }
    for (request, (method, suffix)) in requests.iter().zip([
        ("GET", ""),
        ("POST", ""),
        ("POST", ""),
        ("GET", &format!("{ID}/")),
        ("POST", &format!("{ID}/acquire/")),
        ("POST", &format!("{ID}/release/")),
    ]) {
        assert_eq!(request.method, method);
        assert_eq!(request.path, format!("/speleodb/api/v2/projects/{suffix}"));
    }
    let populated: Value = serde_json::from_slice(&requests[1].body).unwrap();
    assert_eq!(
        populated,
        json!({"name": "Survey", "description": "Description",
        "country": "US", "type": "COMPASS", "latitude": "28.5", "longitude": "-82.5"})
    );
    let omitted: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert!(omitted.get("latitude").is_none());
    assert!(omitted.get("longitude").is_none());
    assert!(requests[4].body.is_empty());
    assert!(requests[5].body.is_empty());
}

#[tokio::test]
async fn zip_upload_preserves_multipart_wire_bytes_and_no_changes_status() {
    let server = Server::new(vec![
        Response::json(200, upload_response()),
        Response::bytes(304, vec![]),
    ]);
    let file = zip_file();
    let info = server.api_info("");
    assert_eq!(
        project::upload_project_zip(&info, id(), "Save survey".into(), file.path())
            .await
            .unwrap(),
        ProjectSaveResult::Saved
    );
    assert_eq!(
        project::upload_project_zip(&info, id(), "Save survey".into(), file.path())
            .await
            .unwrap(),
        ProjectSaveResult::NoChanges
    );
    for request in server.finish() {
        request.assert_sidecar_headers(true);
        assert_eq!(request.method, "PUT");
        assert_eq!(
            request.path,
            format!("/api/v2/projects/{ID}/upload/compass_zip/")
        );
        assert!(
            request
                .header("content-type")
                .unwrap()
                .starts_with("multipart/form-data; boundary=")
        );
        let text = String::from_utf8_lossy(&request.body);
        assert!(text.contains("name=\"artifact\"; filename=\"project.zip\""));
        assert!(
            text.to_ascii_lowercase()
                .contains("content-type: application/zip")
        );
        assert!(text.contains("name=\"message\"\r\n\r\nSave survey\r\n"));
        let artifact = b"PK\x03\x04\0\xffsurvey bytes";
        assert!(
            request
                .body
                .windows(artifact.len())
                .any(|part| part == artifact)
        );
    }
}

#[tokio::test]
async fn direct_zip_download_keeps_bodies_larger_than_the_sdk_default_limit() {
    let length = 8 * 1024 * 1024 + 17;
    let server = Server::new(vec![Response::bytes(200, vec![0x5a; length])]);
    let bytes = project::download_project_zip(&server.api_info("/speleodb"), id())
        .await
        .unwrap();
    assert_eq!(bytes.len(), length);
    assert!(bytes.iter().all(|byte| *byte == 0x5a));
    let requests = server.finish();
    requests[0].assert_sidecar_headers(true);
    assert_eq!(
        requests[0].path,
        format!("/speleodb/api/v2/projects/{ID}/download/compass_zip/")
    );
}

#[tokio::test]
async fn signed_storage_handoff_negotiates_json_then_uses_an_anonymous_transport() {
    let storage = Server::new(vec![Response::bytes(200, b"PK signed archive".to_vec())]);
    let api = Server::new(vec![
        Response::bytes(302, vec![])
            .header(
                "Location",
                storage
                    .url("/signed.zip?signature=storage-only")
                    .to_string(),
            )
            .header("Set-Cookie", "api-session=do-not-forward; Path=/")
            .require_json_renderer(),
    ]);
    let bytes = project::download_project_zip(&api.api_info("/speleodb/"), id())
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), b"PK signed archive");
    let api_requests = api.finish();
    api_requests[0].assert_sidecar_headers(true);
    assert!(api_requests[0].accepts_json());
    let requests = storage.finish();
    requests[0].assert_sidecar_headers(false);
    assert_eq!(requests[0].path, "/signed.zip?signature=storage-only");
    assert!(requests[0].header("cookie").is_none());
    assert!(requests[0].header("x-session-token").is_none());
}

#[tokio::test]
async fn signed_download_transport_errors_do_not_expose_signature_queries() {
    let server = Server::new(vec![Response::bytes(302, vec![]).header(
        "Location",
        "http://127.0.0.1:0/archive.zip?signature=keep-private",
    )]);
    let error = project::download_project_zip(&server.api_info(""), id())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::NetworkRequest(_)));
    assert!(!error.to_string().contains("keep-private"));
    server.finish();
}

#[tokio::test]
async fn relative_download_locations_resolve_against_the_sdk_endpoint_without_auth() {
    let server = Server::new(vec![
        Response::bytes(302, vec![]).header("Location", "./signed.zip?signature=relative"),
        Response::bytes(200, b"relative archive".to_vec()),
    ]);
    let bytes = project::download_project_zip(&server.api_info("/speleodb"), id())
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), b"relative archive");
    let requests = server.finish();
    requests[0].assert_sidecar_headers(true);
    requests[1].assert_sidecar_headers(false);
    assert_eq!(
        requests[1].path,
        format!(
            "/speleodb/api/v2/projects/{ID}/download/compass_zip/signed.zip?signature=relative"
        )
    );
}

#[tokio::test]
async fn sdk_errors_keep_http_status_and_project_domain_meanings() {
    let statuses = [401, 403, 404, 409, 423, 422, 500];
    let server = Server::new(
        statuses
            .into_iter()
            .map(|status| Response::json(status, json!({"error": "server explanation"})))
            .collect(),
    );
    let info = server.api_info("");
    for status in statuses {
        let error = project::fetch_project_info(&info, id()).await.unwrap_err();
        let expected = match status {
            401 | 403 => Error::Unauthorized("server explanation".into()),
            404 => Error::NotFound("server explanation".into()),
            409 | 423 => Error::Conflict("server explanation".into()),
            422 => Error::Unprocessable("server explanation".into()),
            _ => Error::Api {
                status,
                message: "server explanation".into(),
            },
        };
        assert_eq!(error, expected);
    }
    assert_eq!(server.finish().len(), statuses.len());
    let server = Server::new(vec![
        Response::json(409, json!({"error": "locked"})),
        Response::json(423, json!({"error": "locked"})),
        Response::json(422, json!({"error": "empty"})),
    ]);
    let info = server.api_info("");
    for _ in 0..2 {
        assert_eq!(
            project::acquire_project_mutex(&info, id())
                .await
                .unwrap_err(),
            Error::ProjectMutexLocked(id())
        );
    }
    assert_eq!(
        project::download_project_zip(&info, id())
            .await
            .unwrap_err(),
        Error::NoProjectData(id())
    );
    assert_eq!(server.finish().len(), 3);
}

#[tokio::test]
async fn malformed_successes_remain_deserialization_failures_and_auth_stays_friendly() {
    let server = Server::new(vec![
        Response::json(200, json!({})),
        Response::json(201, project_json()),
        Response::json(200, json!({})),
    ]);
    let info = server.api_info("");
    for _ in 0..2 {
        assert!(matches!(
            project::fetch_project_info(&info, id()).await,
            Err(Error::Deserialization(_))
        ));
    }
    assert!(
        auth::authorize_with_token(server.url(""), TOKEN)
            .await
            .unwrap_err()
            .contains("Unexpected response body")
    );
    assert_eq!(server.finish().len(), 3);
    let server = Server::new(
        [400, 401, 403, 429]
            .into_iter()
            .map(|status| Response::json(status, json!({"error": "rejected"})))
            .collect(),
    );
    for _ in 0..3 {
        assert!(
            auth::authorize_with_token(server.url(""), TOKEN)
                .await
                .unwrap_err()
                .contains("Invalid credentials")
        );
    }
    assert!(
        auth::authorize_with_token(server.url(""), TOKEN)
            .await
            .unwrap_err()
            .contains("Too many login attempts")
    );
    assert_eq!(server.finish().len(), 4);
}

#[tokio::test]
async fn absent_tokens_fail_before_network_and_missing_files_keep_the_file_error() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let info = ApiInfo::new(
        format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap(),
        None,
        None,
    );
    let file = zip_file();
    tokio::time::timeout(Duration::from_secs(1), async {
        assert_eq!(
            project::fetch_projects(&info).await.unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::fetch_project_info(&info, id()).await.unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::create_project(
                &info,
                "Survey".into(),
                "Description".into(),
                "US".into(),
                None,
                None
            )
            .await
            .unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::acquire_project_mutex(&info, id())
                .await
                .unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::release_project_mutex(&info, id())
                .await
                .unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::download_project_zip(&info, id())
                .await
                .unwrap_err(),
            Error::NoAuthToken
        );
        assert_eq!(
            project::upload_project_zip(&info, id(), "Save".into(), file.path())
                .await
                .unwrap_err(),
            Error::NoAuthToken
        );
    })
    .await
    .expect("missing credentials must fail without waiting for HTTP");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(matches!(
        project::upload_project_zip(
            &info,
            id(),
            "Save".into(),
            Path::new("/definitely/not/a/real/project.zip")
        )
        .await,
        Err(Error::FileRead(_))
    ));
}
