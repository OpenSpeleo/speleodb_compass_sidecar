// Select the API environment independently of Vite minification.
if (process.argv.includes("--release"))
  process.env.SIDECAR_UI_PROFILE = "release";
// Tauri sets the platform for every hook and DEBUG only for debug builds.
// Standalone builds retain their staging default; native builds match Rust.
const profile =
  process.env.SIDECAR_UI_PROFILE ??
  (process.env.TAURI_ENV_PLATFORM && process.env.TAURI_ENV_DEBUG !== "true"
    ? "release"
    : "debug");
if (profile !== "debug" && profile !== "release")
  throw new Error(`Unknown SIDECAR_UI_PROFILE: ${profile}`);
process.env.SIDECAR_UI_PROFILE = profile;
const { build } = await import("vite");
await build();
export {};
