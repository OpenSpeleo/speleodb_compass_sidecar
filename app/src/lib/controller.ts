// Network calls stay in the native Tauri backend, avoiding CORS and WebView restrictions.
import { invoke } from "@tauri-apps/api/core";
import { FrontendError, normalizeError } from "./errors";
import type {
  AboutInfo,
  ImportPreview,
  InitialImportOutcome,
  ProjectSaveResult,
} from "./types";

/** Validate an OAuth token: exactly 40 hexadecimal characters. */
export function validateOauth(value: string): boolean {
  return value.length === 40 && /^[0-9a-fA-F]{40}$/.test(value);
}
/** Require one @, a dot in the domain, and a nonempty password. */
export function validateEmailPassword(
  email: string,
  password: string,
): boolean {
  const parts = email.split("@");
  return (
    password.length > 0 &&
    parts.length === 2 &&
    parts[0].length > 0 &&
    parts[1].includes(".")
  );
}
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}
export const controller = {
  ensureInitialized: () => call<void>("ensure_initialized"),
  aboutInfo: () => call<AboutInfo>("about_info"),
  reportFrontendError: (message: string, context: string) =>
    call<void>("report_frontend_error", { message, context }),
  checkForUpdatesNow: () => call<void>("check_for_updates_now"),
  dismissUpdateNotification: (dismissalKey: string) =>
    call<void>("dismiss_update_notification", { dismissalKey }),
  openLatestRelease: () => call<void>("open_latest_release"),
  signOut: () => call<void>("sign_out"),
  authenticate: async (
    email: string | null,
    password: string | null,
    oauth: string | null,
    instance: string,
  ): Promise<void> => {
    // Authentication methods are mutually exclusive: OAuth OR email/password.
    const oauthOk = oauth !== null && validateOauth(oauth);
    const passwordOk =
      email !== null &&
      password !== null &&
      validateEmailPassword(email, password);
    if (oauthOk === passwordOk)
      throw new FrontendError(
        "Command",
        "Must provide exactly one auth method: either email+password or a 40-char OAUTH token",
      );
    // The backend saves user preferences after successful authentication.
    await call<void>("auth_request", {
      email,
      password,
      oauth,
      instance: new URL(instance).href,
    });
  },
  openProject: (projectId: string) => call<void>("open_project", { projectId }),
  saveProject: (projectId: string, commitMessage: string) =>
    call<ProjectSaveResult>("save_project", { projectId, commitMessage }),
  discardChanges: () => call<void>("discard_changes"),
  setActiveProject: (projectId: string) =>
    call<void>("set_active_project", { projectId }),
  clearActiveProject: () => call<void>("clear_active_project"),
  releaseProjectMutex: (projectId: string) =>
    call<void>("release_project_mutex", { projectId }),
  pickCompassProjectFile: () =>
    call<string | null>("pick_compass_project_file"),
  previewCompassImport: (projectId: string, makPath: string) =>
    call<ImportPreview>("preview_compass_import", { projectId, makPath }),
  confirmCompassImport: (previewId: string, selectedSectionIds: number[]) =>
    call<InitialImportOutcome>("confirm_compass_import", {
      previewId,
      selectedSectionIds,
    }),
  cancelCompassImport: (previewId: string) =>
    call<void>("cancel_compass_import", { previewId }),
  reimportCompassProject: (
    projectId: string,
    makPath: string,
    commitMessage: string,
  ) =>
    call<void>("reimport_compass_project", {
      projectId,
      makPath,
      commitMessage,
    }),
  createProject: (
    name: string,
    description: string,
    country: string,
    latitude: string | null,
    longitude: string | null,
  ) =>
    call<void>("create_project", {
      name,
      description,
      country,
      latitude,
      longitude,
    }),
};
