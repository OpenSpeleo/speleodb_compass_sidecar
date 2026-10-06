# Frontend test contract map

This map retains the 79 named tests from the former Rust UI crate as behavioral
contracts. Parameterized JavaScript cases may replace several Rust functions;
that does not remove their individual obligations. A source mapping records
coverage intent and is not a passing test result or proof of native parity.

Run unit/component coverage with `cd app && bun run test`, real-browser coverage
with `bun run test:browser`, and native/shared contracts with `make test-rust`.
The browser and native checks remain distinct; jsdom cannot validate layout,
native focus containment or operating-system integration.

## Authentication controller (22)

Replacement:
[`app/src/lib/controller.test.ts`](../app/src/lib/controller.test.ts). OAuth
rows use the `OAuth validation (former controller tests)` parameterized suite
(`%s → %s`). Email/password rows use
`email/password validation (former controller tests)`
(`%s accepts supplied password: %s`).

| Previous test                    | Replacement cases                                    |
| -------------------------------- | ---------------------------------------------------- |
| `oauth_valid_examples`           | OAuth table: oauth valid examples.                   |
| `oauth_uppercase_hex_valid`      | OAuth table: oauth uppercase hex valid.              |
| `oauth_mixed_case_valid`         | OAuth table: oauth mixed case valid.                 |
| `oauth_invalid_length_too_short` | OAuth table: oauth invalid length too short.         |
| `oauth_invalid_length_too_long`  | OAuth table: oauth invalid length too long.          |
| `oauth_empty_string`             | OAuth table: oauth empty string.                     |
| `oauth_non_hex_chars`            | OAuth table: oauth non hex chars.                    |
| `oauth_all_zeros`                | OAuth table: oauth all zeros.                        |
| `oauth_all_fs`                   | OAuth table: oauth all fs.                           |
| `email_password_validation`      | Email/password table: email password validation.     |
| `email_password_both_empty`      | Email/password table: email password both empty.     |
| `email_password_empty_email`     | Email/password table: email password empty email.    |
| `email_password_empty_password`  | Email/password table: email password empty password. |
| `email_missing_at_symbol`        | Email/password table: email missing at symbol.       |
| `email_multiple_at_symbols`      | Email/password table: email multiple at symbols.     |
| `email_missing_dot_in_domain`    | Email/password table: email missing dot in domain.   |
| `email_valid_formats`            | Email/password table: email valid formats.           |
| `email_ending_with_at`           | Email/password table: email ending with at.          |
| `email_starting_with_at`         | Email/password table: email starting with at.        |
| `password_single_char`           | Email/password table: password single char.          |
| `password_long`                  | Email/password table: password long.                 |
| `password_special_chars`         | Email/password table: password special chars.        |

## Command errors (6)

Dedicated normalization assertions cover each previous error contract.
Additional table cases cover every Rust BackendError display variant; command
transport tests remain separate.

| Previous test                                             | Replacement                                                                                                      |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `project_import_permission_error_is_humanized`            | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `project_import_permission_error_is_humanized`            |
| `non_permission_import_error_uses_backend_message`        | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `non_permission_import_error_uses_backend_message`        |
| `format_backend_error_non_import_errors_use_display`      | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `format_backend_error_non_import_errors_use_display`      |
| `js_string_command_error_is_forwarded`                    | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `js_string_command_error_is_forwarded`                    |
| `unknown_js_error_payload_falls_back_to_generic_message`  | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `unknown_js_error_payload_falls_back_to_generic_message`  |
| `source_changed_error_remains_typed_for_preview_recovery` | [`lib/errors.test.ts`](../app/src/lib/errors.test.ts): `source_changed_error_remains_typed_for_preview_recovery` |

## components/project_listing.rs (5)

Replacement:
[`app/src/components/project-listing.test.tsx`](../app/src/components/project-listing.test.tsx).

| Previous test                                      | JavaScript test                               |
| -------------------------------------------------- | --------------------------------------------- |
| `project_name_sort_is_case_insensitive_ascending`  | `compares names case-insensitively ascending` |
| `modified_date_sort_is_descending`                 | `compares modified dates descending`          |
| `sort_projects_by_name_orders_case_insensitively`  | `sorts full projects by name`                 |
| `sort_projects_by_modified_puts_most_recent_first` | `sorts full projects most recent first`       |
| `sort_projects_by_name_is_stable_for_equal_keys`   | `retains incoming order for case-equal names` |

## components/project_details.rs (17)

Replacement:
[`app/src/components/project-details-model.test.ts`](../app/src/components/project-details-model.test.ts).

| Previous test                                                           | JavaScript test                                                         |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `import_message_validation_rejects_empty_or_too_long`                   | `rejects empty or too-long import messages`                             |
| `import_message_validation_accepts_trimmed_non_empty_message`           | `accepts nonempty trimmed content and counts Unicode scalars`           |
| `reimport_flow_transitions_for_cancel_and_file_pick`                    | `transitions after cancellation and file selection`                     |
| `project_action_buttons_disabled_when_compass_is_open`                  | `disables project actions while Compass is open`                        |
| `project_action_buttons_disabled_when_busy`                             | `disables project actions while busy`                                   |
| `project_action_buttons_enabled_when_idle_and_compass_closed`           | `enables actions while idle and Compass closed`                         |
| `commit_relative_time_is_sanitized_when_in_future`                      | `sanitizes future relative commit dates`                                |
| `commit_relative_time_keeps_past_times`                                 | `keeps past relative commit dates`                                      |
| `commit_date_candidates_keep_explicit_offset`                           | `retains explicit timezones`                                            |
| `commit_date_candidates_add_server_timezone_offsets`                    | `adds server timezone candidates after the original`                    |
| `active_processing_overlay_prefers_import_state`                        | `prefers the import processing overlay`                                 |
| `active_processing_overlay_shows_save_while_uploading`                  | `shows saving only while uploading`                                     |
| `save_completion_state_for_saved_project`                               | `clears the message and shows success after a saved project`            |
| `save_completion_state_for_no_changes`                                  | `clears the message and shows no-changes after an identical project`    |
| `section_selection_is_available_only_before_the_initial_import`         | `offers section selection only for initial empty imports`               |
| `an_automated_creation_commit_does_not_block_initial_section_selection` | `does not make initial import availability depend on a creation commit` |
| `initial_import_results_distinguish_upload_from_local_sync_recovery`    | `distinguishes uploaded, local-only and refresh-needed outcomes`        |

## components/update_notification.rs (14)

Replacement:
[`app/src/components/update-notification.test.tsx`](../app/src/components/update-notification.test.tsx).

| Previous test                                     | JavaScript test                                                     |
| ------------------------------------------------- | ------------------------------------------------------------------- |
| `checking_message_matches_ux`                     | `matches the checking message`                                      |
| `downloading_message_without_progress_matches_ux` | `matches downloading without progress`                              |
| `downloading_message_with_progress_matches_ux`    | `matches downloading with progress`                                 |
| `installing_message_matches_ux`                   | `matches installing`                                                |
| `relaunching_message_matches_ux`                  | `matches relaunching`                                               |
| `up_to_date_message_matches_ux`                   | `matches up-to-date`                                                |
| `failed_message_matches_ux`                       | `matches failure`                                                   |
| `failed_is_the_only_error_phase`                  | `marks only failure as an error`                                    |
| `working_phases_match_in_flight_phases_only`      | `marks only in-flight phases as working`                            |
| `renders_nothing_when_notification_is_none`       | `renders nothing when absent`                                       |
| `failed_phase_renders_retry_and_download_latest`  | `renders retry and download only for failure`                       |
| `checking_phase_omits_action_buttons`             | `omits action buttons during checking`                              |
| `dismiss_button_has_accessible_name`              | `gives dismissal an accessible name and sends its phase key`        |
| `message_lives_in_polite_non_atomic_region`       | `scopes the polite non-atomic live region to its label and message` |

## Import selector (15)

| Previous test                                                           | Replacement test                                                                                                                                                                                                                                                                                               |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `default_all_selection_preserves_explicit_prerequisite`                 | [`lib/import-selection.test.ts`](../app/src/lib/import-selection.test.ts): `preserves explicit prerequisites when dependents are removed`                                                                                                                                                                      |
| `selecting_a_dependent_adds_and_removes_only_automatic_dependencies`    | [`lib/import-selection.test.ts`](../app/src/lib/import-selection.test.ts): `adds only explicit roots and removes automatic dependencies with their root`                                                                                                                                                       |
| `filtering_matches_names_and_paths_without_changing_selection`          | [`lib/import-selection.test.ts`](../app/src/lib/import-selection.test.ts): `matches trimmed case-insensitive names and paths`                                                                                                                                                                                  |
| `full_only_previews_cannot_change_selection`                            | [`lib/import-selection.test.ts`](../app/src/lib/import-selection.test.ts): `cannot toggle full-only previews or unknown IDs`                                                                                                                                                                                   |
| `cancelled_or_replaced_requests_cannot_publish_late_results`            | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `invalidates replaced and unmounted generations`                                                                                                                                                                      |
| `confirmation_guard_blocks_double_submission_before_rerender`           | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `serializes reads and submissions before rendering`                                                                                                                                                                   |
| `file_picker_and_preview_reads_are_serialized_before_rerender`          | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `serializes reads and submissions before rendering`                                                                                                                                                                   |
| `renders_labels_dependency_reason_and_full_import_fallback`             | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `defaults to all, locks dependencies and submits explicit roots only; renders connection details only when expanded; blocks stale previews and full-only changes`                                                     |
| `renders_twenty_sections_with_a_persistent_footer`                      | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `does not mount hidden dependency trees for 500 sections`; [`tests/import-selector.spec.ts`](../app/tests/import-selector.spec.ts): `selection includes dependencies and filtering leaves global bulk actions intact` |
| `hundreds_of_connected_sections_do_not_render_hidden_dependency_trees`  | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `does not mount hidden dependency trees for 500 sections`                                                                                                                                                             |
| `checkbox_interactions_preserve_dependencies_and_submit_explicit_roots` | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `defaults to all, locks dependencies and submits explicit roots only`                                                                                                                                                 |
| `search_and_bulk_actions_keep_hidden_sections_in_the_selection`         | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `keeps filtered sections in bulk selection, and offers search recovery`                                                                                                                                               |
| `native_dialog_focus_and_cancel_respect_busy_state`                     | [`tests/import-selector.spec.ts`](../app/tests/import-selector.spec.ts): `native dialog focuses its title and Escape dismisses it`; `busy native dialog blocks Escape and disables confirmation`                                                                                                               |
| `long_warnings_and_errors_keep_import_actions_reachable`                | [`tests/import-selector.spec.ts`](../app/tests/import-selector.spec.ts): `long warnings and errors keep actions visible at 800×900` and `long warnings and errors keep actions visible at 560×620`                                                                                                             |
| `source_changes_require_refresh_and_full_import_disables_filtering`     | [`components/import-selector.test.tsx`](../app/src/components/import-selector.test.tsx): `blocks stale previews and full-only changes; requires refresh after source change, resets selection and releases previous preview`                                                                                   |

The twenty-row markup contract is covered by the browser fixture and the larger
500-row checkbox/count/footer assertions. Browser tests also exercise real
dialog focus/Escape and sticky-footer geometry before and after scrolling, at
the native window size and a short viewport. These source mappings do not imply
the tests have run successfully; native platform and visual comparison evidence
is still required separately.

## Additional coverage

The React suite also covers root screen routing, auth field presence/reset,
creation validation and wire payloads, project navigation errors, save/discard/
reimport flows, background-refresh state retention, import races and cleanup,
update actions/dismissal, and command argument casing. Shared Rust-generated
selection vectors test TypeScript dependency resolution against Rust, including
invalid graphs, sparse IDs and bitset boundaries. Native API/filesystem/menu/
updater tests remain in their original Rust crates.
