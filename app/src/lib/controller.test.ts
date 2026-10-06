import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { controller, validateEmailPassword, validateOauth } from "./controller";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockResolvedValue(undefined));

describe("OAuth validation (former controller tests)", () => {
  it.each([
    ["0123456789abcdef0123456789abcdef01234567", true],
    ["0123456789ABCDEF0123456789ABCDEF01234567", true],
    ["0123456789aBcDeF0123456789AbCdEf01234567", true],
    ["0123456789abcdef", false],
    ["0123456789abcdef0123456789abcdef012345678", false],
    ["", false],
    ["g".repeat(40), false],
    ["0".repeat(39) + " ", false],
    ["0".repeat(39) + "-", false],
    ["0".repeat(40), true],
    ["f".repeat(40), true],
    ["0".repeat(40) + "\n", false],
  ])("%s → %s", (value, valid) => expect(validateOauth(value)).toBe(valid));
});
describe("email/password validation (former controller tests)", () => {
  it.each([
    ["user@example.com", "secret", true],
    ["userexample.com", "secret", false],
    ["user@localhost", "secret", false],
    ["user@example.com", "", false],
    ["", "", false],
    ["", "password", false],
    ["user@@example.com", "password", false],
    ["us@er@example.com", "password", false],
    ["user@domain", "password", false],
    ["user.name@example.com", "pass", true],
    ["user+tag@example.co.uk", "pass", true],
    ["user_name@sub.example.com", "pass", true],
    ["user@", "password", false],
    ["@example.com", "password", false],
    ["user@example.com", "x", true],
    ["user@example.com", "a".repeat(1000), true],
    ["user@example.com", "p@$$w0rd!", true],
    ["user@example.com", "🔒secure", true],
  ])("%s accepts supplied password: %s", (email, password, valid) =>
    expect(validateEmailPassword(email, password)).toBe(valid),
  );
});
it("requires exactly one valid authentication method and never invokes on invalid input", async () => {
  await expect(
    controller.authenticate(null, null, null, "https://example.com"),
  ).rejects.toThrow("Must provide exactly one auth method");
  await expect(
    controller.authenticate(
      "u@e.com",
      "password",
      "0".repeat(40),
      "https://example.com",
    ),
  ).rejects.toThrow("Must provide exactly one auth method");
  expect(invoke).not.toHaveBeenCalled();
});
it("retains null credentials and normalized URL when authenticating", async () => {
  await controller.authenticate(
    null,
    null,
    "0".repeat(40),
    "https://www.speleoDB.org",
  );
  expect(invoke).toHaveBeenCalledWith("auth_request", {
    email: null,
    password: null,
    oauth: "0".repeat(40),
    instance: "https://www.speleodb.org/",
  });
});
it("preserves every command name and exact argument casing", async () => {
  const calls: [
    () => Promise<unknown>,
    string,
    Record<string, unknown> | undefined,
  ][] = [
    [controller.ensureInitialized, "ensure_initialized", undefined],
    [controller.aboutInfo, "about_info", undefined],
    [
      () => controller.reportFrontendError("message", "context"),
      "report_frontend_error",
      { message: "message", context: "context" },
    ],
    [controller.checkForUpdatesNow, "check_for_updates_now", undefined],
    [
      () => controller.dismissUpdateNotification("1:checking"),
      "dismiss_update_notification",
      { dismissalKey: "1:checking" },
    ],
    [controller.openLatestRelease, "open_latest_release", undefined],
    [controller.signOut, "sign_out", undefined],
    [() => controller.openProject("id"), "open_project", { projectId: "id" }],
    [
      () => controller.saveProject("id", "commit"),
      "save_project",
      { projectId: "id", commitMessage: "commit" },
    ],
    [controller.discardChanges, "discard_changes", undefined],
    [
      () => controller.setActiveProject("id"),
      "set_active_project",
      { projectId: "id" },
    ],
    [controller.clearActiveProject, "clear_active_project", undefined],
    [
      () => controller.releaseProjectMutex("id"),
      "release_project_mutex",
      { projectId: "id" },
    ],
    [controller.pickCompassProjectFile, "pick_compass_project_file", undefined],
    [
      () => controller.previewCompassImport("id", "a.mak"),
      "preview_compass_import",
      { projectId: "id", makPath: "a.mak" },
    ],
    [
      () => controller.confirmCompassImport("preview", [2, 7]),
      "confirm_compass_import",
      { previewId: "preview", selectedSectionIds: [2, 7] },
    ],
    [
      () => controller.cancelCompassImport("preview"),
      "cancel_compass_import",
      { previewId: "preview" },
    ],
    [
      () => controller.reimportCompassProject("id", "a.mak", "commit"),
      "reimport_compass_project",
      { projectId: "id", makPath: "a.mak", commitMessage: "commit" },
    ],
    [
      () => controller.createProject("name", "description", "US", "1.2", null),
      "create_project",
      {
        name: "name",
        description: "description",
        country: "US",
        latitude: "1.2",
        longitude: null,
      },
    ],
  ];
  for (const [call, name, args] of calls) {
    await call();
    expect(invoke).toHaveBeenLastCalledWith(name, args);
  }
});
it("passes command results through and normalizes structured rejections", async () => {
  vi.mocked(invoke).mockResolvedValueOnce("NoChanges");
  expect(await controller.saveProject("id", "message")).toBe("NoChanges");
  vi.mocked(invoke).mockRejectedValueOnce({
    ImportSourceChanged: "a.dat changed",
  });
  await expect(
    controller.confirmCompassImport("preview", [1]),
  ).rejects.toMatchObject({
    kind: "ImportSourceChanged",
    message: "a.dat changed",
  });
});
