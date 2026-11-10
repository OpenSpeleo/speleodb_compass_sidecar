# SpeleoDB API v2 — Architecture & Testing

## Feature intent

Every SpeleoDB operation uses the published `speleodb-api` Rust SDK. The SDK
owns endpoint paths, request serialization, response schemas and HTTP execution.
The sidecar's `api` crate adapts that contract to desktop permissions, project
synchronization and existing UI messages. The frontend continues to use Tauri
IPC; application updates and Sentry use their own vendor integrations.

## Engineering scope

| File                      | Ownership                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------- |
| `api/src/auth.rs`         | Token/email authentication through SDK operations and friendly authentication errors                    |
| `api/src/project.rs`      | SDK project operations and desktop lock/save/no-data semantics                                          |
| `api/src/http.rs`         | Reusable SDK transport policies, Token credentials, SDK error conversion and anonymous signed downloads |
| `api/src/models.rs`       | Explicit SDK-to-desktop projection preserving IPC and persisted models                                  |
| `api/src/test_support.rs` | Credential-gated live tests, including SDK authentication preflight                                     |

The standalone workspace manifest and `Cargo.lock` pin the crates.io release.
Inside the SpeleoDB monorepo, the parent's `.cargo/config.toml` uses Cargo's
native `paths` override to compile the local SDK checkout. The sidecar has no
sibling path dependencies, conditional manifests or parent workspace dependency.
The registry source/checksum stays in this repository's lockfile. Run Cargo from
the monorepo root or one of its descendants to discover that override;
`--manifest-path` alone does not change Cargo configuration discovery. Native
path overrides require matching published dependency graphs; coordinate SDK
dependency changes with a release and consumer lockfile update.

## SDK operation inventory

| Desktop operation                       | SDK method                                |
| --------------------------------------- | ----------------------------------------- |
| Token authentication and test preflight | `v2_user_auth_token_retrieve`             |
| Email/password authentication           | `v2_user_auth_token_create_with_json`     |
| List projects                           | `v2_projects_list`                        |
| Create Compass project                  | `v2_projects_create_with_json`            |
| Retrieve project                        | `v2_project_retrieve`                     |
| Acquire mutex                           | `v2_projects_acquire_create`              |
| Release mutex                           | `v2_projects_release_create_with_json`    |
| Download Compass ZIP                    | `v2_projects_download_retrieve_by_format` |
| Upload Compass ZIP                      | `v2_projects_upload`                      |

To add an operation, select its generated SDK method and request types, convert
its error through `http::sdk_error`, and project successful models at the
adapter boundary. Keep domain-specific remapping in the corresponding endpoint
function. Do not add manual SpeleoDB request builders or duplicate wire models.

## Transport and download behavior

### Binary and JSON content negotiation

The SDK includes both the binary success media type and JSON error media type in
the download operation's Accept header. The SpeleoDB download view selects its
JSON renderer before returning a signed-storage redirect, so accepting only
`application/octet-stream` makes renderer negotiation fail with HTTP 406 before
the endpoint runs. Standalone clones and the local monorepo source must both
include this negotiation behavior.

The signed-download loopback regression applies a JSON renderer gate before
issuing its redirect, mirroring that negotiation boundary. Binary-only Accept
headers receive 406, so an unconditional fixture success cannot hide this
regression. Existing live download tests continue to verify backend behavior.

### Adapter transport policy

SDK clients use the selected instance origin and optional path prefix, removing
trailing slashes, query strings and fragments before adding generated paths.
They send `Authorization: Token …` and the versioned
`Tauri/SpeleoDB-Compass-Sidecar/v…` user agent. Do not use the SDK's Bearer
API-key helper for these credentials. Requests fail before networking if no
token is available.

Ordinary operations retain a 10-second timeout. Uploads and downloads retain a
60-second timeout. SDK response buffering has no additional size limit so the
SDK's default 8 MiB limit does not reject existing large project downloads.
Authenticated transports disable redirect following.

Token validation and project metadata GETs share a bounded retry policy for
transport failures, including interrupted response bodies. They retry at most
twice, waiting 250 ms and 500 ms between attempts. Each attempt retains the
10-second timeout, so an unreachable server fails in about 31 seconds at most.
The live test authentication preflight uses this same policy. HTTP errors
(including credential rejection and rate limiting) and malformed responses fail
immediately. Email/password authentication and project writes are not replayed:
repeating those operations can create tokens or duplicate server-side changes.
Successful requests incur no retry delay or additional network traffic. Uploads
and downloads retain their existing 60-second transfer budgets without these
retries.

A download may return ZIP bytes directly or a redirect to signed storage. The
adapter reads the SDK's error-envelope status and `Location`, resolves relative
URLs against the download endpoint, and downloads through a separate client
without API Authorization or cookies. Only HTTP(S) URLs without embedded
credentials are accepted. The signed request uses the remaining portion of the
60-second download budget and permits up to ten storage redirects. The endpoint
path retained in this helper serves only to resolve relative `Location` headers;
the initial API request always uses the SDK.

## Error handling strategy

`http::sdk_error` retains transport error causes (such as timeout, DNS, TLS or
connection closure) in `NetworkRequest`, and maps malformed or unexpected
successful responses to `Deserialization`. Failed HTTP responses retain the
existing status-to-domain mapping:

| HTTP status | Domain error              | Endpoint refinement                                  |
| ----------- | ------------------------- | ---------------------------------------------------- |
| 401, 403    | `Unauthorized(String)`    | Friendly invalid-credentials authentication message  |
| 404         | `NotFound(String)`        | —                                                    |
| 422         | `Unprocessable(String)`   | Downloads return `NoProjectData(uuid)`               |
| 409, 423    | `Conflict(String)`        | Mutex acquisition returns `ProjectMutexLocked(uuid)` |
| Other       | `Api { status, message }` | Upload 304 returns `ProjectSaveResult::NoChanges`    |

Messages use the server's JSON `error` string, falling back to body text and the
HTTP canonical reason. Error text remains capped at 512 bytes with a UTF-8-safe
truncation marker. Upload success requires the SDK's declared response schema;
malformed 2xx bodies are reported rather than treated as a saved project.

## Project type handling

The SDK decodes generated wire models; `models.rs` moves their fields into the
existing desktop types. Permissions remain required, mutex and commit objects
retain lock/tree state, UUIDs become desktop strings where required, and dates
are normalized to equivalent UTC RFC3339 timestamps. `commit_date` remains
optional because the v2 wire model exposes the separate `authored_date`
timestamp in UTC ISO 8601 format. The desktop retains the server's `dt_since`
presentation without substituting the authored timestamp for its commit date.
String-valued mutex/commit alternatives remain invalid desktop snapshots.

Only `COMPASS` projects appear in the desktop list. Other project types declared
by the SDK are decoded and ignored. Future values outside the generated enum
require a reviewed SDK upgrade; they cause a decoding error. Project creation
uses the typed SDK request, omits empty coordinates and serializes Compass as
`"COMPASS"`. Country values must belong to the SDK's generated country enum.

## Testing strategy

`api/tests/sdk_adapter.rs` sends real local HTTP requests through the SDK to
controlled loopback servers. These tests verify method/path/header/body
contracts, typed model conversion, domain errors, multipart upload fields, large
ZIPs and credential isolation across signed download handoffs. They do not
establish backend business validation or replace live integration coverage.
Disconnecting and truncated-body loopback fixtures also verify bounded token and
metadata recovery, failure after three attempts, and immediate email/password
failure. Authentication response fixtures ensure rejected credentials, rate
limits and invalid payloads do not trigger retries.

Unit tests cover desktop model projection, status mapping and authentication
messages. Existing credential-gated integration tests contact a real SpeleoDB
instance, using shared infrastructure in
[api/src/test_support.rs](../api/src/test_support.rs).

### Coverage matrix

Every endpoint is exercised in both success and failure modes:

| Endpoint                | Success                                   | Failure modes                                                            |
| ----------------------- | ----------------------------------------- | ------------------------------------------------------------------------ |
| `authorize_with_token`  | real OAuth token                          | invalid token → friendly message                                         |
| `authorize_with_email`  | real email/password (skip if vars unset)  | wrong creds → friendly message                                           |
| `fetch_projects`        | fixture appears in list                   | unauthorized; missing token (`NoAuthToken`)                              |
| `fetch_project_info`    | fixture roundtrip                         | unknown UUID → `NotFound`; unauthorized                                  |
| `create_project`        | new project created                       | unauthorized; empty input → `Unprocessable`/4xx                          |
| `acquire_project_mutex` | lifecycle on fixture                      | unknown UUID → `NotFound`; unauthorized                                  |
| `release_project_mutex` | lifecycle on fixture                      | unknown UUID → `NotFound`; unauthorized                                  |
| `download_project_zip`  | upload → download lifecycle returns bytes | empty project → `NoProjectData`; unknown UUID → `NotFound`; unauthorized |
| `upload_project_zip`    | upload to fresh project → `Saved`         | unknown UUID → `NotFound`; unauthorized; missing local file → `FileRead` |

SDK projection and HTTP-status mapping also have deterministic unit coverage.

### Fixture project lifecycle

Endpoints that need an existing project (read-only ones like
`fetch_project_info`, plus the acquire/release happy path) call
`fixture_project_id(&api_info)` from `test_support`. Behind it is a
`tokio::sync::OnceCell<Uuid>` that creates **one** project per `cargo test`
invocation, named `sidecar-ci-fixture-<8-hex>`, and reuses it for every later
call.

There is no project-delete endpoint server-side. The accepted trade-off is that
fixture projects accumulate on the staging server. Tests that need write/state
isolation (`upload_project_zip_success`,
`download_project_zip_no_data_returns_no_project_data`) create their own fresh
project per run rather than touching the shared fixture.

Tests that hold the remote mutex must wrap the critical section with
`with_acquired_project_mutex(...)` so the lock is released before any panic is
re-raised to the test harness.

### Why `#[serial]`

Mutex acquire/release operations on the same project ID would race if run in
parallel. Every async test in `auth.rs` and `project.rs` carries
`#[serial_test::serial]` to force in-order execution. This keeps the fixture's
mutex state predictable and avoids "already locked" false positives.

### Skipping cleanly

Tests early-return when `TEST_SPELEODB_INSTANCE` or `TEST_SPELEODB_OAUTH` are
unset. CI provides them via secrets; local devs ship them via `.env` (see
[TESTING.md](../TESTING.md)). When those variables are configured, the test
harness performs one auth preflight before endpoint tests proceed. If the host
is unreachable or `TEST_SPELEODB_OAUTH` is rejected, the suite fails once with a
setup-focused preflight message and the remaining real-HTTP tests skip, so the
output does not look like many endpoint regressions.

Tests that additionally need `TEST_SPELEODB_EMAIL`/`TEST_SPELEODB_PASSWORD` skip
independently when those are unset.

## Performance implications

- `OnceCell` ensures fixture creation happens at most once per test session (one
  `create_project` POST instead of dozens).
- SDK clients share pooled Reqwest transports for ordinary calls and transfers.
  There are no automatic retries. Transport policy lives in `http.rs`.
- The SDK buffers response bytes and decodes typed models. Desktop projection
  moves model fields without another JSON serialization pass. ZIPs remain fully
  buffered, as required by the existing project-extraction interface.

## Verification commands

```bash
cargo fmt --all -- --check    # lint (zero diff expected)
make test-rust                # full Rust suite incl. all v2 tests
cargo test -p api -- --nocapture   # api-only with stdout
```

Upload success confirms that source files were saved. The current contract omits
the former map-generation status field; optional map processing is managed by
server administrators. The server must not remove that field for deployed native
clients until the regenerated SDK has been published through its authorized
release process and this adapter's SDK pin and lockfile adopt that release.
Verify the adapter from a standalone checkout outside the monorepo's Cargo path
override before deploying the matching server contract. Updating the local SDK
source or these fixtures alone does not update the SDK used by a packaged
client.
