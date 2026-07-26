# Desktop menu platform roles

When replacing Tauri's default native menu, preserve platform-provided roles in
addition to app-specific menu items. The roles do not require dedicated
top-level menus: a compact application can keep them in one unified app menu.

Rules:

- Keep exactly one `SpeleoDB Compass Sidecar` top-level menu on macOS and
  Windows unless the product genuinely grows enough actions to need more.
- Keep a predefined Quit item in the custom menu so Cmd+Q works on macOS.
- Keep predefined Cut, Copy, Paste, and Select All items in the unified menu so
  native text-editing shortcuts keep working even without an Edit submenu.
- Disable macOS's automatic Writing Tools, Dictation, and Emoji & Symbols
  additions by setting the corresponding `NSUserDefaults` before Tauri creates
  the menu. Neither user defaults nor `Info.plist` declarations are sufficient
  across every supported macOS release.
- Keep the same keys in the bundle `Info.plist`, and do not remove native
  clipboard roles to suppress unrelated operating-system entries.
- After each menu installation, remove every native item whose selector is not
  owned by the Sidecar, then normalize separators. Selector allowlisting removes
  Writing Tools and AutoFill without depending on localized titles.
- Put the AppKit implementation, every call site, and all Objective-C
  dependencies behind `cfg(target_os = "macos")`. Do not provide cross-platform
  no-op shims: leaving the module undeclared makes Windows compilation isolation
  explicit and lets native Windows CI enforce it.
- Model authentication-dependent menu contents in one shared layout and test its
  exact ordering and separator invariants for every state.
