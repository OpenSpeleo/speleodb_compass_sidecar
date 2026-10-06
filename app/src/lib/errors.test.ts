import { describe, expect, it } from "vitest";
import type { BackendError } from "./types";
import {
  formatBackendError,
  FrontendError,
  normalizeError,
  UNKNOWN_COMMAND_ERROR,
} from "./errors";

const importError = (permission: boolean): BackendError => ({
  ProjectImport: {
    src_path: "/tmp/Region_1.DAT",
    dst_path: "/tmp/project/Region_1.DAT",
    details: permission
      ? "Operation not permitted (os error 1)"
      : "No such file or directory (os error 2)",
    is_permission_error: permission,
  },
});

describe("legacy frontend error contract", () => {
  it("project_import_permission_error_is_humanized", () => {
    expect(normalizeError(importError(true))).toMatchObject({
      kind: "Command",
      message:
        "Cannot import 'Region_1.DAT' because macOS denied file access. Move/copy the survey folder to an accessible location (for example `~/Documents`) and try again.",
    });
  });
  it("non_permission_import_error_uses_backend_message", () => {
    expect(normalizeError(importError(false)).message).toBe(
      "Error importing project file from: /tmp/Region_1.DAT to /tmp/project/Region_1.DAT: No such file or directory (os error 2)",
    );
  });
  it("format_backend_error_non_import_errors_use_display", () => {
    expect(normalizeError({ ProjectNotFound: "/tmp/missing" }).message).toBe(
      "Project not found: /tmp/missing",
    );
  });
  it("js_string_command_error_is_forwarded", () => {
    expect(normalizeError("simple command failure")).toMatchObject({
      kind: "Command",
      message: "simple command failure",
    });
    // Unit variants received as raw command strings retain the backend representation.
    expect(normalizeError("NoAuthToken").message).toBe("NoAuthToken");
  });
  it("unknown_js_error_payload_falls_back_to_generic_message", () => {
    for (const value of [
      42,
      null,
      undefined,
      [],
      {},
      { UnknownVariant: "unknown" },
      { constructor: "prototype key" },
      { toString: "prototype key" },
      { Api: { status: "401", message: "bad" } },
      { Api: { status: -1, message: "bad" } },
      { Api: { status: 401.5, message: "bad" } },
      { Api: { status: 65536, message: "bad" } },
      { ProjectImport: { src_path: "/tmp/a" } },
    ])
      expect(normalizeError(value)).toMatchObject({
        kind: "Command",
        message: UNKNOWN_COMMAND_ERROR,
      });
  });
  it("source_changed_error_remains_typed_for_preview_recovery", () => {
    const error = normalizeError({ ImportSourceChanged: "A.DAT changed" });
    expect(error).toBeInstanceOf(FrontendError);
    expect(error).toMatchObject({
      kind: "ImportSourceChanged",
      message: "A.DAT changed",
    });
    expect(String(error)).toBe("A.DAT changed");
    expect(normalizeError(error)).toBe(error);
  });
});

describe("Rust Display parity for event payloads", () => {
  const cases: [BackendError, string][] = [
    ["NoAuthToken", "No auth token set"],
    [
      { ProjectAlreadyExists: "/tmp/project" },
      "Project directory already exists at /tmp/project",
    ],
    [{ ProjectNotFound: "/tmp/project" }, "Project not found: /tmp/project"],
    [
      { CreateDirectory: "/tmp/project" },
      "Couldn't create storage directory for project",
    ],
    [
      { Deserialization: "invalid JSON" },
      "Error deserializing data: invalid JSON",
    ],
    [{ Serialization: "invalid TOML" }, "Error serializing TOML"],
    ["NoUserPreferences", "No user preferences found"],
    [{ ApiInfoRead: "/tmp/prefs" }, "Error reading user preferece file"],
    [{ ApiInfoWrite: "/tmp/prefs" }, "Error writing user preference file"],
    [
      importError(true),
      "Error importing project file from: /tmp/Region_1.DAT to /tmp/project/Region_1.DAT: Operation not permitted (os error 1)",
    ],
    [{ ProjectWrite: "/tmp/project" }, "Error writing project file"],
    ["FilePermissionSet", "Error setting file permissions"],
    ["NoProjectSelected", "No project selected"],
    [
      { ProjectFileNotFound: "/tmp/A.DAT" },
      "Referenced file not found: /tmp/A.DAT",
    ],
    [
      { EmptyProjectDirectory: "uuid" },
      "Empty project directory for project ID: uuid",
    ],
    [{ NetworkRequest: "offline" }, "Network request error: offline"],
    [{ Unauthorized: "expired" }, "Unauthorized: expired"],
    [{ NotFound: "missing" }, "Resource not found: missing"],
    [{ Unprocessable: "invalid" }, "Unprocessable entity: invalid"],
    [{ Conflict: "locked" }, "Conflict: locked"],
    [
      { Api: { status: 500, message: "server error" } },
      "API error 500: server error",
    ],
    [{ FileRead: "denied" }, "File read failed: denied"],
    [{ FileWrite: "denied" }, "File write failed: denied"],
    [{ NoProjectData: "uuid" }, "No project data found for project uuid"],
    [
      { ProjectMutexLocked: "uuid" },
      "Project mutex already locked for project uuid",
    ],
    [{ ZipFile: "invalid" }, "Zip File Error: invalid"],
    [{ OsCommand: "failed" }, "Os Command Error: failed"],
    ["CompassNotFound", "Compass Not Found"],
    [{ CompassExecutable: "failed" }, "Compass Executable Error: failed"],
    [{ CompassProject: "invalid" }, "Compass Project Error: invalid"],
    [
      { ImportSourceChanged: "A.DAT changed" },
      "Source files changed. Refresh the preview before importing: A.DAT changed",
    ],
    ["NoAppHandle", "No app handle available"],
  ];
  it.each(cases)(
    "formats %j using the unchanged Rust Display implementation",
    (payload, message) => expect(formatBackendError(payload)).toBe(message),
  );
});
