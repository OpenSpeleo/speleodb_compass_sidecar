# Desktop menu platform roles

When replacing Tauri's default native menu, preserve platform-provided roles in
addition to app-specific menu items.

Rules:

- Keep a predefined Quit item in the custom menu so Cmd+Q works on macOS.
- Keep predefined Edit items for Cut, Copy, Paste, and Select All so native text
  editing shortcuts keep working.
- Test menu section composition for every auth state that rebuilds the menu.
