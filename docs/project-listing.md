# Project Listing UX

## Feature intent

The project listing is the user's primary surface for navigating SpeleoDB
content from the sidecar. It must:

- Surface the user's projects in a predictable, **user-controlled** order so
  scrolling and selection are habitual rather than a hunt.
- Leave the listing visually breathable inside the 800×900 fixed Tauri
  window so individual rows are easy to scan.

This document captures **why** the layout and sort design look the way
they do; the "what" is in the linked source files.

## Engineering scope

| File | Responsibility |
| --- | --- |
| [app/src/components/project_listing.rs](../app/src/components/project_listing.rs) | Renders the listing, owns `sort_mode` state, dispatches the comparators. |
| [app/src/components/project_listing_item.rs](../app/src/components/project_listing_item.rs) | Renders a single project row with status icon, permission badge, lock badge. |
| [common/src/ui_state.rs](../common/src/ui_state.rs) | Defines `ProjectStatus` and exposes `name()` / `modified_date()` so the UI sorts via accessors instead of reaching into the wrapped `ProjectInfo`. |
| [app/styles.css](../app/styles.css) | The `.container` rule that gives every screen its horizontal gutter. |
| [app/src/components/main_layout.rs](../app/src/components/main_layout.rs) | Hosts `<main class="container">` and uses container-relative widths for its children. |

The backend (`app/src-tauri/src/state.rs`) ships projects already sorted
by `modified_date` descending. That ordering is the **incoming**
ordering the UI receives; the UI then re-sorts according to the user's
chosen `SortMode`.

## Layout: container-relative widths

### Why this exists

Earlier the project listing's `<section>` used `min-width: 96vw`, the
main layout's `<header>` used `width: 96vw`, and the commit textarea
used `max-width: 94vw`. With viewport-relative widths, children opted
out of their parent container's flow entirely — the visible left/right
gutter on the 800 px window collapsed to ~16 px per side and the UI
felt cramped and edge-glued.

### How it works now

The gutter is centralised on `.container` (the class on every screen's
root `<main>` — auth, main layout, loading screen):

```css
.container {
    margin: 0 auto;
    padding: 1rem 1.5rem;
    box-sizing: border-box;
    /* …flex layout omitted… */
}
```

Every direct child opts into that gutter by sizing relatively
(`width: 100%`) instead of using viewport units. Concretely:

- `MainLayout`'s header: `width: 100%`.
- `ProjectListing`'s `<section>`: `width: 100%`.
- `ProjectDetails`' commit textarea: `width: 100%; box-sizing: border-box;`
  (border-box is required so the inline 8 px padding doesn't push the
  textarea past the parent's content edge.)

### Consequences

- Tuning the gutter is now a **one-line CSS edit** on `.container`. No
  per-component inline style needs to follow.
- Modal backdrops in `project_details.rs`, `modal.rs`, and
  `create_project_modal.rs` deliberately keep `width: 100vw` because
  they are full-bleed overlays positioned with `position: fixed` and
  must cover the whole window irrespective of the container.

## Sort modes

### Available modes

Two user-controlled sort modes, with `Name` as the default:

| Mode | Comparator | Behavior |
| --- | --- | --- |
| `Name` | `cmp_project_name` | Case-insensitive ascending. |
| `Modified` | `cmp_modified_date_desc` | Descending lexical order over the ISO-8601 `modified_date` string. |

Both comparators are pure free functions over `&str` so they are
unit-testable without constructing `ProjectStatus` values. The
dispatcher `sort_projects(mode, &mut [ProjectStatus])` selects the
comparator and runs `Vec::sort_by` in place.

### Why `Name` is the default

The backend pushes a fresh `UiState` roughly every second to refresh
local-status indicators (Compass running, file dirty, etc.). If the
default sort were `Modified`, any backend-side modification would
shuffle the visible order, making the listing feel unstable while the
user is just trying to click on a row.

Alphabetical order is invariant under those refreshes unless the user
explicitly opts in. `Modified` remains a one-click toggle for users who
want recency.

### Stability is part of the contract

`sort_projects` uses `Vec::sort_by`, which is stable. Two rows that
compare equal under the active comparator (e.g. case-different names
that collapse under `to_lowercase()`) retain their incoming order from
the backend, which is itself sorted by `modified_date` descending. That
gives a deterministic implicit secondary sort without paying for a
multi-key comparator.

This is enforced by `sort_projects_by_name_is_stable_for_equal_keys`.
A future "optimisation" that swaps `sort_by` for `sort_unstable_by`
would fail that test loudly.

### Why ISO-8601 lexical comparison is correct

`ProjectInfo.modified_date` is emitted by the backend as a fixed-width
ISO-8601 timestamp (e.g. `2026-04-27T14:32:11Z`). For fixed-width
ISO-8601 strings, lexical order matches chronological order — so
`String::cmp` is correct without any date-parsing dependency. The
comparator's `///` doc-comment names this contract; the
`modified_date_sort_is_descending` test pins the behavior.

If the backend ever emits non-ISO-8601 or variable-width dates, the
test will keep passing (it only asserts ordering on ISO-8601 inputs)
but the production behavior will silently degrade. The mitigation is
upstream: keep `state.rs` emitting ISO-8601, full stop.

## Toggle UI

The toggle is a single row of two buttons above the list, prefixed by a
small "Sort by:" label. Active button is filled blue (matching the
existing primary CTA `#2563eb`); inactive is outlined against the dark
background. The styling is centralised in `sort_button_style(active)`
so the active/inactive contrast is one place to tune.

`sort_mode` is a `use_state` cell, so the user's choice survives
backend `UiState` refreshes for the lifetime of the listing screen. It
deliberately resets to `Name` when the screen is unmounted/remounted
(e.g. after navigating into project details and back) — a cross-session
sort preference is out of scope.

## Testing strategy

The non-trivial logic — comparators, dispatcher, and stability — lives
in pure functions in [`app/src/components/project_listing.rs`](../app/src/components/project_listing.rs)
under `#[cfg(test)] mod tests`:

| Test | Asserts |
| --- | --- |
| `project_name_sort_is_case_insensitive_ascending` | `cmp_project_name` collapses case before comparison; covers Less / Greater / Equal. |
| `modified_date_sort_is_descending` | `cmp_modified_date_desc` reverses lexical order over ISO-8601. |
| `sort_projects_by_name_orders_case_insensitively` | End-to-end sort over `ProjectStatus` values constructed via a local builder. |
| `sort_projects_by_modified_puts_most_recent_first` | End-to-end sort over `ProjectStatus`, "newest first" ordering. |
| `sort_projects_by_name_is_stable_for_equal_keys` | Locks in the use of `sort_by` (stable) over `sort_unstable_by` so equal-keyed rows retain their incoming order. |

Every test is dual-targeted (`#[cfg_attr(target_arch = "wasm32",
wasm_bindgen_test)]` + native `#[test]`) so the same suite runs from
both `cargo test -p speleodb-compass-sidecar-ui` (native) and
`make test-ui` (WASM via `wasm-pack`).

### What is **not** tested, and why

- The `sort_button_style(active)` helper returns one of two static
  strings. The bool→str mapping is cosmetic and trivially inspected;
  not worth a dedicated test.
- Default `SortMode::Name` is set in a single `use_state` initializer.
  Asserting it would require component-level rendering tests, which
  are not used elsewhere in the crate. The default is documented in
  the `SortMode` doc-comment and in this file.
- CSS-only changes (`.container` padding and the
  `width: 96vw` → `width: 100%` migrations) carry no automated test;
  they are validated by visual inspection (`make dev`) and by the
  Rust build's `format!` type-checking on the inline-style strings.

## Performance implications

- The full project list is recloned per render (`Vec::to_vec`) and
  sorted in place. With `n` projects of small `ProjectInfo`
  payload, the cost is `O(n log n)` per render and `O(n)` clones.
- The list is small in practice (single-digit to mid-double-digit
  project counts); this is well under any noticeable threshold.
- The sort runs on every `UiState` push (~1 s cadence) for as long as
  the listing screen is mounted. The same `n log n` argument applies;
  no caching is warranted.
- `to_lowercase()` allocates per comparison key. With small `n` this
  is a non-issue. If lists ever grow past several hundred entries,
  consider pre-computing a lowercase key once per render.

## Verification

From repo root:

```bash
cargo fmt --all -- --check
cargo test -p common
cargo test -p speleodb-compass-sidecar-ui project_listing
make test-ui                 # WASM-side via wasm-pack (requires Firefox)
```
