# Desktop Menu

SpeleoDB Compass Sidecar owns the native Tauri application menu so it can show
app-specific account and update actions. That menu must always include a native
application submenu with Quit and a native Edit submenu with the standard
clipboard actions.

The application submenu uses Tauri's predefined Quit item so platform quit
shortcuts such as Cmd+Q on macOS keep working even though the app owns a custom
menu.

The Edit submenu is the app-wide source of desktop clipboard accelerators:
Cut, Copy, Paste, and Select All. It is present both before and after
authentication so fields such as the OAuth token input keep normal Cmd/Ctrl+V
paste behavior.

Menu construction lives in the Tauri backend. When adding a new app menu state,
reuse the shared menu builder instead of constructing a one-off menu, otherwise
platform edit shortcuts can silently disappear.
