import type {
  CommitInfo,
  InitialImportOutcome,
  LocalProjectStatus,
  ProjectSaveResult,
} from "../lib/types";
import { formatBackendError } from "../lib/errors";
import { SERVER_TIME_ZONE } from "../lib/constants";
import { rustTrim } from "../lib/text";

export type ReimportFlow =
  | { phase: "Idle" }
  | { phase: "ConfirmOverwrite" }
  | { phase: "EnterCommitMessage"; makPath: string };
export const nextReimportStateAfterFilePick = (
  makPath: string | null,
): ReimportFlow =>
  makPath === null
    ? { phase: "Idle" }
    : { phase: "EnterCommitMessage", makPath };
export const validateImportCommitMessage = (message: string) =>
  rustTrim(message) !== "" && Array.from(message).length <= 255;
export const shouldDisableProjectActionButtons = (
  compassOpen: boolean,
  busy: boolean,
) => compassOpen || busy;
// The backend classifies remote Compass data. An automated creation commit does
// not itself mean survey data exists, so only the local status controls this gate.
export const supportsInitialImport = (status: LocalProjectStatus) =>
  status === "EmptyLocal";
export const isDirty = (status: LocalProjectStatus) =>
  status === "Dirty" || status === "DirtyAndOutOfDate";
export function initialImportResultMessage(
  outcome: InitialImportOutcome,
  count: number,
): string {
  if ("Synced" in outcome)
    return `Imported ${count} ${count === 1 ? "section" : "sections"} and saved to SpeleoDB.`;
  if ("LocalOnly" in outcome)
    return `Your ${count} imported sections are saved on this computer. The upload did not finish. Retry the upload using Save Project below; your selection is preserved. ${formatBackendError(outcome.LocalOnly.error)}`;
  return `Your ${count} imported sections were uploaded to SpeleoDB, but local synchronization did not finish. Use Save Project below to finish synchronization. Your selection is preserved. ${formatBackendError(outcome.UploadedNeedsRefresh.error)}`;
}
export function normalizeCommitRelativeTime(relative: string): string {
  const trimmed = rustTrim(relative);
  return /^in /i.test(trimmed) ? "just now" : trimmed;
}
export function commitDateParseCandidates(value: string): string[] {
  const trimmed = rustTrim(value);
  if (trimmed === "") return [];
  if (/[Zz]$|[+-]\d{2}:?\d{2}$/.test(trimmed)) return [trimmed];
  return ["US/Eastern", "America/New_York"].includes(SERVER_TIME_ZONE)
    ? [trimmed, `${trimmed}-05:00`, `${trimmed}-04:00`]
    : [trimmed];
}
export function displayCommitTime(commit: CommitInfo): string {
  if (commit.commit_date !== undefined && commit.commit_date !== null) {
    for (const candidate of commitDateParseCandidates(commit.commit_date)) {
      const milliseconds = Date.parse(candidate);
      if (!Number.isFinite(milliseconds)) continue;
      const display = rustTrim(
        new Date(milliseconds).toLocaleString("default"),
      );
      if (display !== "") return display;
    }
  }
  return normalizeCommitRelativeTime(commit.dt_since);
}
export const activeProcessingOverlay = (
  reimporting: boolean,
  uploading: boolean,
) => (reimporting ? "Importing" : uploading ? "Saving" : null);
export function saveCompletionState(result: ProjectSaveResult) {
  return {
    clearCommitMessage: true,
    clearCommitMessageError: true,
    showUploadSuccess: result === "Saved",
    showNoChangesModal: result === "NoChanges",
  };
}
