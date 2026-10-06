// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const build = vi.hoisted(() => vi.fn());
vi.mock("vite", async (importOriginal) => ({
  ...(await importOriginal<typeof import("vite")>()),
  build,
}));

const originalArgv = process.argv;
beforeEach(() => {
  vi.resetModules();
  build.mockReset();
  process.argv = ["bun", "scripts/build.ts"];
  vi.stubEnv("SIDECAR_UI_PROFILE", undefined);
  vi.stubEnv("TAURI_ENV_PLATFORM", undefined);
  vi.stubEnv("TAURI_ENV_DEBUG", undefined);
});
afterEach(() => {
  process.argv = originalArgv;
  vi.unstubAllEnvs();
});

async function buildOrigin() {
  await import("../scripts/build");
  expect(build).toHaveBeenCalledOnce();
  const { default: config } = await import("../vite.config");
  return JSON.parse(config.define!.__SIDECAR_API_BASE_URL__);
}

describe("frontend build API origin", () => {
  it("keeps standalone builds on staging", async () => {
    expect(await buildOrigin()).toBe("https://stage.speleodb.org");
  });

  for (const platform of ["darwin", "windows", "linux"]) {
    it(`uses production for a native ${platform} release hook`, async () => {
      // Tauri omits TAURI_ENV_DEBUG for release; it does not set it to false.
      vi.stubEnv("TAURI_ENV_PLATFORM", platform);
      expect(await buildOrigin()).toBe("https://www.speleodb.org");
    });
  }

  it("uses staging for native --debug builds", async () => {
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
    vi.stubEnv("TAURI_ENV_DEBUG", "true");
    expect(await buildOrigin()).toBe("https://stage.speleodb.org");
  });

  it("supports standalone release builds", async () => {
    process.argv.push("--release");
    expect(await buildOrigin()).toBe("https://www.speleodb.org");
  });

  it("preserves the explicit profile override", async () => {
    vi.stubEnv("TAURI_ENV_PLATFORM", "darwin");
    vi.stubEnv("SIDECAR_UI_PROFILE", "debug");
    expect(await buildOrigin()).toBe("https://stage.speleodb.org");
  });

  it("supports the release workflow's explicit profile", async () => {
    vi.stubEnv("SIDECAR_UI_PROFILE", "release");
    expect(await buildOrigin()).toBe("https://www.speleodb.org");
  });

  it("rejects invalid profiles before emitting any assets", async () => {
    vi.stubEnv("SIDECAR_UI_PROFILE", "invalid");
    await expect(import("../scripts/build")).rejects.toThrow(
      "Unknown SIDECAR_UI_PROFILE: invalid",
    );
    expect(build).not.toHaveBeenCalled();
  });
});
