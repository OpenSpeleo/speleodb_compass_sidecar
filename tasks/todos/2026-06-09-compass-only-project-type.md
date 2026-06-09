# Compass-only project type deserialization

## Problem

Authenticated project loading fails when SpeleoDB returns a project whose
`type` is not accepted by the client enum. The observed response includes
`"type": "OTHER"` at project `9e12fe62-ad38-471b-a625-7ed9960ab3e4`
(`South Pole Cave`), and the client currently deserializes the whole list
before filtering to Compass projects.

## Plan

- [x] Change `ProjectType` so only `COMPASS` is treated as supported.
- [x] Deserialize every non-`COMPASS` project type into an ignored value.
- [x] Keep `ProjectType::Compass` serialization as `"COMPASS"` for create calls.
- [x] Filter project lists to `ProjectType::Compass`.
- [x] Add unit tests for `COMPASS`, `ARIANE`, and `OTHER` project types.
- [x] Document the Compass-only project-type contract.
- [x] Run targeted tests and lint.

## Review

Implemented `ProjectType` custom Serde handling so only `"COMPASS"` maps to a
supported value; all other strings map to `Ignored`. `fetch_projects` now
retains only Compass projects after decoding, so unsupported server project
types cannot fail the entire authenticated project load.

Follow-up test hardening: API integration tests now preflight the configured
SpeleoDB host and OAuth token once. If the configured host is unreachable or
the token is rejected, the suite fails with one setup-focused preflight message
and later real-HTTP tests skip instead of producing many endpoint-looking
failures. Tauri backend tests also use a process-scoped temp `.compass` home
under `cfg(test)` so they do not write to the real user home directory.

Verification:

- `cargo test -p common api_types`
- `cargo test -p api retain_compass_projects_ignores_unsupported_types`
- `cargo test -p api fetch_projects_no_token_returns_no_auth_token`
- `make lint`
- `TEST_SPELEODB_INSTANCE= TEST_SPELEODB_OAUTH= cargo test -p api --lib`
- `cargo test -p api --lib -- --nocapture` fails once with the expected
  SpeleoDB integration preflight message when the configured host is
  unreachable.
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
