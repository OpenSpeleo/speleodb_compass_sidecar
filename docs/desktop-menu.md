# Desktop Menu

SpeleoDB Compass Sidecar owns one native Tauri application menu named
`SpeleoDB Compass Sidecar`. Account, editing, help, and application-lifecycle
actions are flat groups inside that menu rather than separate top-level menus.
This keeps the small action set discoverable without spending most of the menu
bar on one- or two-item menus.

## Layout

Before authentication:

1. About
2. Check For Updates Now
3. Cut, Copy, Paste, and Select All
4. Quit

After authentication, Sign Out is inserted as its own group between the update
actions and the clipboard actions. Separators appear only between non-empty
groups, so rebuilding the unauthenticated menu cannot leave a leading, trailing,
or doubled separator.

The menu label and ordering are identical on supported desktop platforms. macOS
displays it in the global menu bar; Windows displays it in the application
window menu bar.

## Native roles

There is deliberately no top-level Edit menu, but the unified menu retains
Tauri's predefined Cut, Copy, Paste, and Select All roles. Those roles provide
the native Cmd/Ctrl+X/C/V/A accelerators used by WebView inputs, including the
OAuth token and project forms. Replacing them with plain text menu items, or
removing them, can silently break clipboard shortcuts.

Quit is also a predefined Tauri role. This preserves platform quit behavior,
including Cmd+Q on macOS, while continuing to route application exit through the
existing mutex-release cleanup.

## Ownership and state transitions

Menu layout and construction live in `app/src-tauri/src/state.rs`. A pure layout
helper defines the title and ordered actions for each authentication state; the
Tauri builder consumes that layout to create exactly one submenu. The custom
About, Check For Updates Now, and Sign Out identifiers are shared with the event
dispatcher in `app/src-tauri/src/lib.rs`.

The menu is rebuilt only after initialization or authentication reaches a stable
state. Sign Out is the only authentication-dependent action. Menu API failures
are logged rather than allowed to terminate the application.

The layout contains a fixed number of entries and changes only on authentication
transitions, so it adds no polling or per-render frontend work.

## Verification

Unit tests assert the exact authenticated and unauthenticated layouts and
separator invariants. Native smoke testing on macOS and Windows must also
confirm:

- only one top-level menu is visible;
- signing in adds Sign Out and signing out removes it;
- Cmd/Ctrl clipboard shortcuts work in editable fields;
- About, update checking, and Quit retain their existing behavior.

On macOS, the startup path sets AppKit's `NSDisabledCharacterPaletteMenuItem`,
`NSDisabledDictationMenuItem`, and `NSWritingToolsMenuItemDisabled` user
defaults before Tauri creates the menu. The same values remain declared in
`app/src-tauri/Info.plist` for packaged-app metadata.

Because neither mechanism reliably suppresses every macOS release's additions,
the backend also cleans the application submenu on the main thread after every
menu installation. It keeps separators and the exact selectors owned by the
Sidecar, removes every other injected action—including Writing Tools, AutoFill,
Dictation, and Emoji & Symbols—and then removes orphaned separators. Matching
selectors instead of localized titles keeps this behavior independent of the
system language.

The cleanup does not replace or disable the native clipboard roles.

The AppKit cleanup has a compile-time platform boundary. The module declaration,
both call sites, and its Objective-C/AppKit dependencies use
`cfg(target_os = "macos")`; consequently, none of the cleanup code or native
dependencies is compiled or linked on Windows. Windows uses only the shared
Tauri menu layout and native menu roles described above.

CI runs the Rust test suite natively on both `macos-latest` and
`windows-latest`. The macOS run compiles and executes the AppKit-specific tests,
while the Windows run proves that the application builds and tests without the
macOS module.
