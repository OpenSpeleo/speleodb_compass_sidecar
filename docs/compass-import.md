# Selective initial Compass import

## Intent and scope

A MAK file can reference more survey files than a new SpeleoDB project needs.
Initial import lets the user choose MAK sections before any files are copied
into the project or uploaded. A section is a MAK survey-file entry, usually one
DAT file; this selector does not split surveys within a DAT file.

The selector applies to the first import into an empty project, including the
empty-project prompt and the import button in project details. Existing
populated-project **Re-import from Disk** keeps its full-project overwrite and
commit-message flow.

The source MAK and DAT files are read-only inputs. A partial import creates a
new MAK containing the selected sections and copies their associated DAT files
into the local project. The original file order is preserved because Compass
uses that order to resolve station connections.

## User experience

1. Choose **Import from Disk** and select the source MAK in the native picker.
2. Review **Choose sections to import** after analysis completes. Every section
   starts selected. Search narrows the displayed list without changing the
   selection; **Clear selection** is a quick starting point for a small subset.
3. Select the desired sections. Dependencies are included automatically, and a
   required section explains which selected section needs it. Remove the
   dependent selection before removing its prerequisite.
4. Choose **Import N sections**. The app prepares the local project and uploads
   it using the existing initial-import commit message.

Cancelling the file picker or selector leaves the empty project unchanged.
Import is unavailable when nothing is selected. A source change between review
and confirmation disables import and offers **Refresh preview**, which analyzes
the source again and resets the selection to all sections.

Some valid Compass projects use syntax or station relationships that cannot be
filtered safely. Their selector explains why only the complete project can be
imported and offers **Import complete project**. This preserves the existing
full import where validation succeeds. A MAK that cannot be parsed or a
referenced file that cannot be read still produces an error.

## Text encodings

The previous UTF-8-only MAK check rejected legacy Windows text before it could
resolve survey files. For example, `Backshall´s Backdoor.DAT` uses byte `0xB4`
for `´` in Windows-1252; that byte is invalid on its own in UTF-8. This caused
the “MAK file must contain UTF-8 or ASCII text” error even though the filename
was valid for Compass.

Initial import accepts UTF-8 (including a BOM), ASCII, and legacy Windows-1252
MAK and DAT text. Valid UTF-8 takes precedence; otherwise the backend interprets
the whole file as Windows-1252. Each MAK and DAT chooses its encoding
independently, so a project can contain both encodings. This supports Western
Windows filenames without replacing characters or modifying the source. Other
Windows code pages and UTF-16 are not inferred; MAK files containing NUL bytes
or UTF-16 byte-order marks remain rejected.

The MAK scanner tracks byte spans in the original input and decodes fields only
for analysis and filename resolution. Filtering therefore preserves retained
source bytes, even when legacy characters expand into multiple Unicode bytes.
Necessary destination-path rewrites use the source MAK encoding, and generated
MAK validation uses that same encoding. DAT text is decoded only for station
indexing; the copied survey files retain their original contents. This boundary
keeps Compass data intact while allowing Unicode filenames in the selector and
metadata. Encoding detection adds a linear scan during analysis, with no extra
reads when the selection changes.

Dependency matching retains both decoded station names and their original byte
identity. A legacy station can accidentally contain valid UTF-8 bytes, while an
unrelated comment makes another file detect as Windows-1252. Comparing only the
decoded names would then miss a connection between identical station bytes. The
dual comparison conservatively keeps that prerequisite while still matching the
same Unicode station across explicitly different file encodings.

This policy belongs to the initial-import analyzer. The separate
existing-project reimport path still uses the `compass_data` reader and its
encoding restrictions.

## Dependency analysis

Dependency inference is deliberately conservative: a smaller upload must not
silently disconnect cave surveys or move their coordinates. The backend analyzes
the MAK and the stations in its DAT files once during preview. The UI then
resolves selections against that graph without rescanning the files.

Survey labels may contain spaces or accented text, and survey teams may be
empty. Those headers do not define station identity: dependencies use the
explicit `FROM` and `TO` values and MAK links, preserving complete
case-sensitive names without applying an editor's historical length limit or
truncating identifiers. Shot flags are read through their closing `#`, including
empty `#| #` markers; unknown flags and incomplete shot rows still prevent
selective import.

Previously, the analyzer applied station-name restrictions to survey labels.
`SURVEY NAME: Silt in the reg line` in `Abejas Negras.DAT` therefore triggered
the warning about unsupported survey names or prefixes and forced a complete
import. The label's spaces did not make its connections ambiguous. Separating
metadata validation from station indexing avoids that false restriction while
still requiring complete headers and shot tables. Blank teams and delimited
empty flags are valid layout variants, not missing station data. Long station
names remain distinct even when their first characters match.

Dependencies point to preceding MAK sections within the current station context.
A MAK link list replaces that context: ordinary links retain their earlier
station providers, while fixed stations introduce their own coordinates. The
DAT's stations then extend the context. Every possible earlier provider is
retained conservatively rather than choosing one matching file. This includes
shots with an exclusion flag because Compass settings can override that flag.

A section without its own link list also retains the most recent context reset.
Otherwise, removing the reset could reconnect a station to an earlier section
that the original MAK deliberately disconnected. These requirements appear as
**Preserves station context** in the selector. Transitive requirements are
included: choosing C that needs B that needs A imports A, B, and C in source
order.

Classic flat MAK records, link/fixed-station declarations, comments, and known
global settings can be analyzed. Compass folders, unsupported DAT headers or
shot syntax, unresolved links, or settings whose effects cannot be established
require a complete import. An unknown top-level MAK command is rejected when the
complete survey-file inventory cannot be established. This boundary avoids
silently omitting files under the guise of a complete import.

Selection has two parts: the user's explicit choices and the dependency closure
derived from those choices. A prerequisite explicitly chosen by the user stays
selected after its dependent is removed. An automatically included prerequisite
can disappear when its last selected dependent is removed. Both frontend and
backend use the same shared selection resolver; the backend validates section
IDs and recomputes the closure at confirmation.

## Ownership and import lifecycle

- The Yew selector owns interaction, focus, search, and presentation. Its
  controller transports preview IDs and section IDs through Tauri IPC.
- The shared `common::compass_import` contract describes preview sections,
  dependency explanations, selection resolution, and terminal outcomes.
- The backend owns paths, parsing, source identity, project eligibility,
  staging, and upload. Browser-provided IDs never become arbitrary write paths.
- The existing API crate remains the owner of HTTP upload and project metadata
  requests. No new server endpoint or on-disk metadata schema is required.

The native `pick_compass_project_file` command returns a path before
`preview_compass_import(project_id, mak_path)` starts analysis, so the frontend
can show its analysis state immediately. Preview binds the analyzed source to
its project and authenticated session. Confirmation uses
`confirm_compass_import(preview_id, selected_section_ids)`; cancellation uses
`cancel_compass_import(preview_id)`.

Confirmation validates that binding and checks that the project is still empty,
editable, and locked by the current user, and that Compass is closed. Unexpected
files already present in the local working directory are preserved and block the
initial import.

The import constructs and validates a complete working copy in a unique staging
directory before publishing it. Destination paths stay within the project;
source references are resolved separately from output paths. External references
and filename conflicts are given contained destination paths, with corresponding
MAK path updates. Retained MAK records and global settings preserve their source
bytes except for necessary path rewrites, and DAT contents are copied unchanged.
Publication and upload are coordinated with project actions and background
synchronization so another task cannot replace the files being imported. The
save operation uses the preview's explicit project ID.

Preview and confirmation wait for ongoing project work, then revalidate their
project and session binding. A periodic background refresh therefore cannot
discard a completed analysis just because it is busy. Application update checks
and downloads can continue, but installation and restart use the same operation
gate: an updater cannot terminate the process partway through import or save.

Before publication, validation or filesystem failure leaves the prior project
content intact. After publication, the working copy is retained even if the
network operation fails. Index and revision synchronization follows a successful
upload, using the existing save machinery.

### Completion and recovery

| Outcome                | Meaning                                                                | Recovery                                                                    |
| ---------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `Synced`               | Imported files were uploaded and local synchronization completed.      | Continue editing the project.                                               |
| `LocalOnly`            | A valid local import exists, but packing or uploading failed.          | Review the error and use the normal Save action to retry.                   |
| `UploadedNeedsRefresh` | Upload succeeded, but project refresh or local synchronization failed. | Use Save Project to finish local synchronization after resolving the error. |

A command error before publication keeps the selector available for correction
or a new source choice. A partial completion must not be presented as if no
import occurred: it has already produced a recoverable local project. Retrying
an upload may return the existing `NoChanges` result when the server received a
previous request whose response was lost.

## Testing and verification

Deterministic parser, selection, and filesystem tests use fixtures and temporary
directories; they do not require credentials or contact SpeleoDB. Compatibility
regressions generate small synthetic MAK/DAT inputs, including the offending
filename byte and survey label. They never read a user's Downloads directory or
require the original private project. Important cases include:

- Filtering a larger MAK to a small subset, preserving ordering and retained
  syntax, and verifying the generated MAK, DAT list, metadata, and ZIP agree.
- Direct, transitive, and shared-provider dependencies; required-row selection;
  unsupported relationships falling back to a complete import.
- Missing or changed source files, malformed syntax, nested and
  platform-specific paths, duplicate filenames, safe destinations, and unchanged
  source bytes.
- Spaced survey labels, blank teams, empty or adjacent-comment flag markers, and
  full-length accented station names with exact dependency matching.
- Legacy accented filenames and DAT headers, UTF-8/BOM compatibility, retained
  byte offsets during filtering, and path rewrites in the original encoding.
- Encoding-ambiguous station bytes that must retain prerequisites even when
  per-file decoding produces different Unicode names.
- Cancellation, empty or unknown selections, stale previews, changed project or
  account, concurrent operations, and failures before and after publication.
- Keyboard selection, search, focus containment/restoration, Escape dismissal,
  long filenames, scrolling, and reduced-motion presentation in the selector.
- Background refresh contention and application updates waiting for project
  operations before installation or process exit.

Run the import parser and filesystem regressions from the repository root:

```bash
cargo test -p speleodb-compass-sidecar --lib project_management::import::tests
```

The regression tests live in `app/src-tauri/src/project_management/import.rs`:

| Test                                                                                                   | Contract protected                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `legacy_windows_text_preserves_names_offsets_and_source_bytes`                                         | Windows-1252 filename resolution, complete/subset MAK bytes, unchanged DAT copies and sources.                                                                   |
| `mixed_file_encodings_preserve_dependencies_and_staged_bytes`                                          | Independently encoded MAK/DAT files resolve the same accented station, select prerequisites, and preserve staged payloads, metadata filenames, and source bytes. |
| `remapped_paths_keep_the_mak_encoding`                                                                 | Required destination-path rewrites retain UTF-8 or Windows-1252 encoding.                                                                                        |
| `utf8_bom_and_unicode_names_still_round_trip`, `legacy_encoding_is_selected_per_file_not_per_filename` | UTF-8/BOM compatibility and consistent whole-file decoding, even when an individual legacy filename resembles UTF-8.                                             |
| `survey_labels_and_empty_teams_do_not_change_station_dependencies`                                     | Spaced/accented labels and blank teams permit selective import with the correct prerequisite DAT.                                                                |
| `multi_survey_dat_layouts_preserve_later_station_dependencies`                                         | CRLF surveys with or without form feeds retain later-survey dependencies when spaced labels, blank teams, and empty flags occur together.                        |
| `permissive_metadata_parsing_still_requires_complete_headers`                                          | Empty/control-containing names or missing structural headers still prohibit a subset; complete import preserves the source payload.                              |
| `delimited_flags_allow_empty_values_and_adjacent_comments`                                             | Empty flags and adjacent comments preserve shot indexing; incomplete or unknown flags remain unsupported.                                                        |
| `long_and_accented_station_names_retain_their_complete_identity`                                       | Exact DAT/MAK-link matching without truncating or conflating station names.                                                                                      |
| `utf16_and_binary_mak_files_are_rejected`                                                              | Encoding tolerance does not admit UTF-16 or NUL-containing MAK files.                                                                                            |

For broader validation, run `make lint`, browser-backed `make test-ui`, and
`make build-ui`. Browser testing requires Firefox, geckodriver, and a
wasm-bindgen CLI version matching `Cargo.lock`. The existing CI matrix runs
native tests on Windows and macOS; Windows file locking and native-picker
behavior also need a Windows smoke test.

The real-HTTP API suite is separate. It requires valid test credentials and may
create permanent fixture projects on the configured server. Verify its auth
preflight before running it. An import lifecycle check should use a dedicated
test project, release its remote mutex even after failure, and download the
uploaded ZIP to verify its contents.

## Performance

File parsing and staging run outside the async UI thread. Preview builds the
dependency graph once; selection changes operate on that graph, so typing or
toggling a checkbox does not reread DAT files. Confirmation revalidates the
source before copying to prevent a stale preview from importing different
content. This includes unselected files because they informed the dependency
graph. Only included DAT files enter the working copy and upload archive.

The modal keeps its header and actions visible while the section list scrolls.
At short viewport heights, including browser zoom, the sheet scrolls as one
surface with sticky actions so controls remain reachable without nested
scrolling. Its costs scale with source file analysis and the dependency graph,
without adding file rescans to the application's background status checks.
