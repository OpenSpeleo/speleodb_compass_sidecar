# Compass process monitoring

On Windows, Rust tracks the PID returned when launching Compass and checks it
once per second in the existing background task. The tracked PID keeps project
actions and application shutdown guarded while Compass is open. Other platforms
open the project folder and do not query processes.

`AppState::check_compass_process` uses the fallible `sysinfo` 0.40
`System::new_all()` API. A successful query clears the PID only when that
process is absent. A query error is logged and preserves the PID: lack of system
information is insufficient evidence that editing has ended. The next background
pass retries without changing polling frequency or adding work when no PID is
tracked. This retains the existing process snapshot cost and native ownership of
application state; no frontend or API contract changes are required.

The process query is injected into `check_compass_process_with` so native unit
tests on every host cover skipping an untracked PID, keeping a running process,
preserving state on error, and clearing it after a subsequent confirmed exit.
Run
`cargo test -p speleodb-compass-sidecar --lib state::tests::compass_process`.
Windows compilation in CI additionally checks the actual sysinfo adapter, which
is platform gated; host unit tests do not claim Windows runtime coverage.
