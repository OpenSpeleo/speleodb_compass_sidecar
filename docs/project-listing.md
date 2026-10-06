# Project Listing UX

## Feature intent

The project listing is the user's primary surface for navigating SpeleoDB
content from the sidecar. It must:

- Surface the user's projects in a predictable, **user-controlled** order so
  scrolling and selection are habitual rather than a hunt.
- Leave the listing visually breathable inside the 800×900 fixed Tauri window so
  individual rows are easy to scan.

This document captures **why** the layout and sort design look the way they do;
the "what" is in the linked source files.

## Engineering scope

React project-listing components own sorting and rendering; shared frontend
helpers preserve the previous comparison and truncation rules. `UiState` remains
a Rust-owned IPC snapshot. The backend supplies modified-date ordering, and the
UI applies the user's chosen stable sort. `app/styles.css` continues to own the
container gutter.

## Layout: container-relative widths

### Why this exists

Earlier the project listing's `<section>` used `min-width: 96vw`, the main
layout's `<header>` used `width: 96vw`, and the commit textarea used
`max-width: 94vw`. With viewport-relative widths, children opted out of their
parent container's flow entirely — the visible left/right gutter on the 800 px
window collapsed to ~16 px per side and the UI felt cramped and edge-glued.

### How it works now

The gutter is centralised on `.container` (the class on every screen's root
`<main>` — auth, main layout, loading screen):

```css
.container {
  margin: 0 auto;
  padding: 1rem 1.5rem;
  box-sizing: border-box;
  /* …flex layout omitted… */
}
```

Every direct child opts into that gutter by sizing relatively (`width: 100%`)
instead of using viewport units. Concretely:

- `MainLayout`'s header: `width: 100%`.
- `ProjectListing`'s `<section>`: `width: 100%`.
- `ProjectDetails`' commit textarea: `width: 100%; box-sizing: border-box;`
  (border-box is required so the inline 8 px padding doesn't push the textarea
  past the parent's content edge.)

### Consequences

- Tuning the gutter is now a **one-line CSS edit** on `.container`. No
  per-component inline style needs to follow.
- Modal backdrops in the project-details, generic-modal and create-project
  components deliberately keep `width: 100vw` because they are full-bleed
  overlays positioned with `position: fixed` and must cover the whole window
  irrespective of the container.

## Sort modes

### Available modes

The default Name mode sorts lowercased names ascending. Modified mode sorts
fixed-width ISO-8601 modification strings descending. Comparators preserve the
previous Rust ordering rather than using locale-aware sorting or date parsing.
Both operate on a copy of the incoming project list.

### Why `Name` is the default

The backend pushes a fresh `UiState` roughly every second to refresh
local-status indicators (Compass running, file dirty, etc.). If the default sort
were `Modified`, any backend-side modification would shuffle the visible order,
making the listing feel unstable while the user is just trying to click on a
row.

Alphabetical order is invariant under those refreshes unless the user explicitly
opts in. `Modified` remains a one-click toggle for users who want recency.

### Stability is part of the contract

The frontend uses stable array sorting. Two rows that compare equal under the
active comparator (e.g. case-different names that collapse under lowercasing)
retain their incoming order from the backend, which is itself sorted by
`modified_date` descending. That gives a deterministic implicit secondary sort
without paying for a multi-key comparator.

Frontend sort regressions enforce this incoming-order stability.

### Why ISO-8601 lexical comparison is correct

`ProjectInfo.modified_date` is emitted by the backend as a fixed-width ISO-8601
timestamp (e.g. `2026-04-27T14:32:11Z`). For fixed-width ISO-8601 strings,
lexical order matches chronological order — so direct string comparison is
correct without any date-parsing dependency. Frontend tests pin this contract.

If the backend ever emits non-ISO-8601 or variable-width dates, the test will
keep passing (it only asserts ordering on ISO-8601 inputs) but the production
behavior will silently degrade. The mitigation is upstream: keep `state.rs`
emitting ISO-8601, full stop.

## Toggle UI

The toggle is a single row of two buttons above the list, prefixed by a small
"Sort by:" label. Active button is filled blue (matching the existing primary
CTA `#2563eb`); inactive is outlined against the dark background. The React
component preserves the same active/inactive styles.

The selected sort mode is React state, so the user's choice survives backend
`UiState` refreshes for the lifetime of the listing screen. It deliberately
resets to `Name` when the screen is unmounted/remounted (e.g. after navigating
into project details and back) — a cross-session sort preference is out of
scope.

## Testing and performance

Frontend unit tests cover case-insensitive name ordering, descending modified
strings and incoming-order stability for equal keys. Browser tests also cover
the default Name choice, sort button interaction, scroll position and reset on
unmount. Run `make lint`, `make test-ui` and `make build-ui`.

Sorting operates on a copy of the incoming list and costs O(n log n). Lowercase
keys can be computed once per sort; do not change comparator semantics to
locale-aware collation or date parsing. Backend pushes may update statuses every
second, but UI state and user choice remain stable while the listing is mounted.
No native polling or server ordering changes are part of the frontend migration.
