# SpeleoDB API v2 — Architecture & Testing

## Feature intent

The desktop sidecar talks to a single SpeleoDB instance over HTTPS. Every
request is now scoped under the `api/v2/` prefix and every JSON response is
the bare resource itself — no `{data, success, timestamp, url}` envelope.
Error bodies are now `{"error": "<message>"}` only.

This document captures **why** the architecture looks the way it does so
future contributors don't reinvent it.

## Engineering scope

The `api` crate is the only place in the workspace that makes outbound HTTP
calls. Three files own everything:

| File | Responsibility |
| --- | --- |
| [api/src/http.rs](../api/src/http.rs) | URL building, auth header injection, request execution, status-code → typed `Error` mapping. **Single source of truth.** |
| [api/src/auth.rs](../api/src/auth.rs) | `authorize_with_token`, `authorize_with_email`. Translates typed `Error` variants into user-facing strings the UI renders verbatim. |
| [api/src/project.rs](../api/src/project.rs) | All project-scoped endpoints. Each function is ~5 LOC: build URL, attach auth, dispatch through `http::send_json` (or `http::send_raw` for binary). |

### The `http` module contract

```text
v2_url(instance, path)           → fully-qualified Url
authenticated(builder, api_info) → builder + Authorization header (or NoAuthToken)
send_json::<T>(builder)          → T on 2xx, typed Error otherwise
send_raw(builder)                → reqwest::Response on 2xx, typed Error otherwise
map_status_to_error(status, body) → Error (used internally, exported for tests)
```

`send_raw` exists for two callers: `download_project_zip` (returns ZIP bytes,
not JSON) and `upload_project_zip` (only the status code is meaningful;
`304 Not Modified` is mapped to `ProjectSaveResult::NoChanges`).

### Error model

Defined in [common/src/error.rs](../common/src/error.rs). HTTP-derived
variants:

| HTTP status | Variant | Domain remap |
| --- | --- | --- |
| 401, 403 | `Unauthorized(String)` | — |
| 404 | `NotFound(String)` | — |
| 422 | `Unprocessable(String)` | `download_project_zip` → `NoProjectData(uuid)` |
| 409, 423 | `Conflict(String)` | `acquire_project_mutex` → `ProjectMutexLocked(uuid)` |
| anything else | `Api { status, message }` | `upload_project_zip` checks `status == 304` → `NoChanges` |

Every variant carries the server-provided `error` string from the JSON body
when present, with fallback to the raw body text and finally to the HTTP
status canonical reason. Call sites pattern-match on the typed variant; they
should not inspect status codes directly.

## Adding a new endpoint

Drop in five lines. No request boilerplate, no wrapper struct, no manual
status mapping.

```rust
pub async fn cancel_project(api_info: &ApiInfo, id: Uuid) -> Result<ProjectInfo, Error> {
    let url = http::v2_url(api_info.instance(), &format!("projects/{id}/cancel/"));
    let req = http::authenticated(get_api_client().post(url), api_info)?;
    http::send_json(req).await
}
```

If a status code carries domain meaning beyond the generic mapping (e.g.
"locked → already-checked-out"), remap inside the endpoint:

```rust
match http::send_json(req).await {
    Ok(info) => Ok(info),
    Err(Error::Conflict(_)) => Err(Error::AlreadyCheckedOut(id)),
    Err(e) => Err(e),
}
```

## Testing strategy

All tests in the `api` crate hit a real SpeleoDB instance. There are no
mocks; the staging server is the contract. Tests live alongside production
code in `#[cfg(test)] mod tests` blocks, with shared infrastructure in
[api/src/test_support.rs](../api/src/test_support.rs).

### Coverage matrix

Every endpoint is exercised in both success and failure modes:

| Endpoint | Success | Failure modes |
| --- | --- | --- |
| `authorize_with_token` | real OAuth token | invalid token → friendly message |
| `authorize_with_email` | real email/password (skip if vars unset) | wrong creds → friendly message |
| `fetch_projects` | fixture appears in list | unauthorized; missing token (`NoAuthToken`) |
| `fetch_project_info` | fixture roundtrip | unknown UUID → `NotFound`; unauthorized |
| `create_project` | new project created | unauthorized; empty input → `Unprocessable`/4xx |
| `acquire_project_mutex` | lifecycle on fixture | unknown UUID → `NotFound`; unauthorized |
| `release_project_mutex` | lifecycle on fixture | unknown UUID → `NotFound`; unauthorized |
| `download_project_zip` | upload → download lifecycle returns bytes | empty project → `NoProjectData`; unknown UUID → `NotFound`; unauthorized |
| `upload_project_zip` | upload to fresh project → `Saved` | unknown UUID → `NotFound`; unauthorized; missing local file → `FileRead` |

Plus `http.rs` carries unit tests for `map_status_to_error` (no network).

### Fixture project lifecycle

Endpoints that need an existing project (read-only ones like
`fetch_project_info`, plus the acquire/release happy path) call
`fixture_project_id(&api_info)` from `test_support`. Behind it is a
`tokio::sync::OnceCell<Uuid>` that creates **one** project per
`cargo test` invocation, named `sidecar-ci-fixture-<8-hex>`, and reuses it
for every later call.

There is no project-delete endpoint server-side. The accepted trade-off is
that fixture projects accumulate on the staging server. Tests that need
write/state isolation (`upload_project_zip_success`,
`download_project_zip_no_data_returns_no_project_data`) create their own
fresh project per run rather than touching the shared fixture.

Tests that hold the remote mutex must wrap the critical section with
`with_acquired_project_mutex(...)` so the lock is released before any panic is
re-raised to the test harness.

### Why `#[serial]`

Mutex acquire/release operations on the same project ID would race if run in
parallel. Every async test in `auth.rs` and `project.rs` carries
`#[serial_test::serial]` to force in-order execution. This keeps the
fixture's mutex state predictable and avoids "already locked" false
positives.

### Skipping cleanly

Tests early-return when `TEST_SPELEODB_INSTANCE` or `TEST_SPELEODB_OAUTH`
are unset. CI provides them via secrets; local devs ship them via `.env`
(see [TESTING.md](../TESTING.md)). Tests that additionally need
`TEST_SPELEODB_EMAIL`/`TEST_SPELEODB_PASSWORD` skip independently when those
are unset.

## Performance implications

- `OnceCell` ensures fixture creation happens at most once per test session
  (one `create_project` POST instead of dozens).
- Centralized request execution means all retry/timeout policy lives in one
  place — adjusting `PROJECT_DOWNLOAD_TIMEOUT` or adding a global retry
  policy is a one-line change.
- Response bodies are streamed once; the helper does not buffer twice. The
  failure path drains the body via `.text()` only because we need it for
  the error message.

## Verification commands

```bash
cargo fmt --all -- --check    # lint (zero diff expected)
make test-rust                # full Rust suite incl. all v2 tests
cargo test -p api -- --nocapture   # api-only with stdout
```
