import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/ui-contract.json";
import { formatBackendError, normalizeError } from "./errors";
import { resolveSelection } from "./import-selection";
import { loadingMessage } from "../components/loading-screen";
import {
  updateDismissalKey,
  updateNotificationMessage,
} from "../components/update-notification";
import { initialImportResultMessage } from "../components/project-details-model";
import type {
  BackendError,
  InitialImportOutcome,
  LoadingState,
  UiState,
  UpdateNotification,
} from "./types";

// common/tests/ui_contract.rs independently produces and checks these fixtures.
// These assertions exercise the frontend consumers of that exact wire format.
describe("Rust serialization contract", () => {
  it("retains snake_case state fields and every platform/local status", () => {
    const states = fixture.ui_states as UiState[];
    expect(states.map((state) => state.platform)).toEqual([
      "Windows",
      "MacOS",
      "Linux",
    ]);
    for (const state of states) {
      expect(Object.keys(state).sort()).toEqual([
        "compass_open",
        "loading_state",
        "platform",
        "project_downloading",
        "project_status",
        "selected_project_id",
        "update_notification",
        "user_email",
      ]);
      expect(
        state.project_status.map((project) => project.local_status),
      ).toEqual([
        "Unknown",
        "RemoteOnly",
        "EmptyLocal",
        "Dirty",
        "UpToDate",
        "OutOfDate",
        "DirtyAndOutOfDate",
      ]);
      expect(
        state.project_status.every(
          (project) => project.info.id === state.selected_project_id,
        ),
      ).toBe(true);
      expect(state.compass_open).toBe(true);
      expect(state.project_downloading).toBe(true);
    }
    expect(fixture.empty_state).toMatchObject({
      user_email: null,
      selected_project_id: null,
      update_notification: null,
      project_status: [],
      compass_open: false,
      project_downloading: false,
    });
  });
  it("distinguishes omitted optional fields from explicit nulls and omits commit trees", () => {
    const [minimal, complete] = fixture.projects;
    expect(minimal).not.toHaveProperty("latitude");
    expect(minimal).not.toHaveProperty("longitude");
    expect(minimal).toMatchObject({
      active_mutex: null,
      fork_from: null,
      latest_commit: null,
      type: "COMPASS",
    });
    expect(complete).toMatchObject({ latitude: 45.5, longitude: -73.5 });
    expect(complete!.latest_commit).toHaveProperty(
      "commit_date",
      "2026-02-01T12:00:00-05:00",
    );
    expect(complete!.latest_commit).not.toHaveProperty("tree");
    expect(fixture.commit_without_date).not.toHaveProperty("commit_date");
    expect(fixture.commit_without_date).not.toHaveProperty("tree");
    expect(fixture.project_types).toEqual(["COMPASS", "IGNORED"]);
  });
  it("consumes every loading variant including an externally tagged failure", () => {
    const actual = (fixture.loading_states as LoadingState[]).map(
      loadingMessage,
    );
    expect(actual).toEqual([
      "Initializing...",
      "Loading user preferences...",
      "Authenticating user...",
      "Loading projects...",
      "Starting application...",
      "Starting application...",
      "Error: Network request error: offline",
    ]);
  });
  for (const [index, entry] of fixture.notifications.entries())
    it(`consumes updater phase ${index} with Rust dismissal identity`, () => {
      const notification = entry.payload as UpdateNotification;
      expect(updateDismissalKey(notification)).toBe(entry.dismissal_key);
      expect(updateNotificationMessage(notification.phase)).toEqual(
        expect.any(String),
      );
      expect(
        updateNotificationMessage(notification.phase).length,
      ).toBeGreaterThan(0);
    });
  it("resolves the serialized preview and consumes every outcome without unwrapping errors incorrectly", () => {
    const selection = resolveSelection(fixture.preview.sections, new Set([1]));
    expect([...selection.included]).toEqual([0, 1]);
    expect(fixture.preview.full_import_reason).toBeNull();
    const outcomes = fixture.import_outcomes as InitialImportOutcome[];
    expect(outcomes.map((outcome) => Object.keys(outcome)[0])).toEqual([
      "Synced",
      "Synced",
      "LocalOnly",
      "UploadedNeedsRefresh",
    ]);
    for (const outcome of outcomes)
      expect(initialImportResultMessage(outcome, 2).length).toBeGreaterThan(0);
    expect(initialImportResultMessage(outcomes[2]!, 2)).toContain(
      "Network request error: offline",
    );
    expect(initialImportResultMessage(outcomes[3]!, 2)).toContain(
      "Unauthorized: expired",
    );
  });
});

describe("all Rust Error Display variants", () => {
  for (const [index, entry] of fixture.errors.entries())
    it(`matches Rust Display for fixture ${index}`, () => {
      const error = entry.payload as BackendError;
      expect(formatBackendError(error)).toBe(entry.display);
      const normalized = normalizeError(error);
      if (typeof error === "string") {
        // Unit variants arrive as strings, so the existing controller forwards them literally.
        expect(normalized.message).toBe(error);
      } else if ("ImportSourceChanged" in error) {
        expect(normalized.kind).toBe("ImportSourceChanged");
        expect(normalized.message).toBe(error.ImportSourceChanged);
      } else if (
        "ProjectImport" in error &&
        error.ProjectImport.is_permission_error
      ) {
        expect(normalized.message).toBe(
          "Cannot import 'A.DAT' because macOS denied file access. Move/copy the survey folder to an accessible location (for example `~/Documents`) and try again.",
        );
      } else {
        expect(normalized.kind).toBe("Command");
        expect(normalized.message).toBe(entry.display);
      }
    });
  it("covers every variant and both permission values", () => {
    const names = fixture.errors.map((entry) =>
      typeof entry.payload === "string"
        ? entry.payload
        : Object.keys(entry.payload)[0],
    );
    expect(new Set(names).size).toBe(32);
    expect(names.filter((name) => name === "ProjectImport")).toHaveLength(2);
  });
});
