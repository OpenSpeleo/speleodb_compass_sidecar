import { describe, expect, it } from "vitest";
import {
  activeProcessingOverlay,
  commitDateParseCandidates,
  displayCommitTime,
  initialImportResultMessage,
  nextReimportStateAfterFilePick,
  normalizeCommitRelativeTime,
  saveCompletionState,
  shouldDisableProjectActionButtons,
  supportsInitialImport,
  validateImportCommitMessage,
} from "./project-details-model";
import type { LocalProjectStatus } from "../lib/types";

describe("project details behavior", () => {
  it("rejects empty or too-long import messages", () => {
    for (const value of ["", " \n\t ", "a".repeat(256)])
      expect(validateImportCommitMessage(value)).toBe(false);
  });
  it("accepts nonempty trimmed content and counts Unicode scalars", () => {
    expect(validateImportCommitMessage(" Imported files ")).toBe(true);
    expect(validateImportCommitMessage("🦇".repeat(255))).toBe(true);
    expect(validateImportCommitMessage("🦇".repeat(256))).toBe(false);
  });
  it("transitions after cancellation and file selection", () => {
    expect(nextReimportStateAfterFilePick(null)).toEqual({ phase: "Idle" });
    expect(nextReimportStateAfterFilePick("C:\\Survey\\Cave.mak")).toEqual({
      phase: "EnterCommitMessage",
      makPath: "C:\\Survey\\Cave.mak",
    });
  });
  it("disables project actions while Compass is open", () =>
    expect(shouldDisableProjectActionButtons(true, false)).toBe(true));
  it("disables project actions while busy", () =>
    expect(shouldDisableProjectActionButtons(false, true)).toBe(true));
  it("enables actions while idle and Compass closed", () =>
    expect(shouldDisableProjectActionButtons(false, false)).toBe(false));
  it("sanitizes future relative commit dates", () => {
    expect(normalizeCommitRelativeTime(" in 2 minutes ")).toBe("just now");
    expect(normalizeCommitRelativeTime("IN 5 HOURS")).toBe("just now");
  });
  it("keeps past relative commit dates", () =>
    expect(normalizeCommitRelativeTime(" 3 days ago ")).toBe("3 days ago"));
  it("retains explicit timezones", () => {
    for (const value of [
      "2026-04-20T12:00:00Z",
      "2026-04-20T12:00:00-04:00",
      "2026-04-20T12:00:00+0200",
    ])
      expect(commitDateParseCandidates(value)).toEqual([value]);
  });
  it("adds server timezone candidates after the original", () =>
    expect(commitDateParseCandidates(" 2026-04-20T12:00:00 ")).toEqual([
      "2026-04-20T12:00:00",
      "2026-04-20T12:00:00-05:00",
      "2026-04-20T12:00:00-04:00",
    ]));
  it("prefers the import processing overlay", () =>
    expect(activeProcessingOverlay(true, true)).toBe("Importing"));
  it("shows saving only while uploading", () => {
    expect(activeProcessingOverlay(false, true)).toBe("Saving");
    expect(activeProcessingOverlay(false, false)).toBe(null);
  });
  it("clears the message and shows success after a saved project", () =>
    expect(saveCompletionState("Saved")).toEqual({
      clearCommitMessage: true,
      clearCommitMessageError: true,
      showUploadSuccess: true,
      showNoChangesModal: false,
    }));
  it("clears the message and shows no-changes after an identical project", () =>
    expect(saveCompletionState("NoChanges")).toEqual({
      clearCommitMessage: true,
      clearCommitMessageError: true,
      showUploadSuccess: false,
      showNoChangesModal: true,
    }));
  it("offers section selection only for initial empty imports", () => {
    const statuses: LocalProjectStatus[] = [
      "Unknown",
      "RemoteOnly",
      "EmptyLocal",
      "Dirty",
      "UpToDate",
      "OutOfDate",
      "DirtyAndOutOfDate",
    ];
    for (const status of statuses)
      expect(supportsInitialImport(status)).toBe(status === "EmptyLocal");
  });
  it("does not make initial import availability depend on a creation commit", () =>
    expect(supportsInitialImport("EmptyLocal")).toBe(true));
  it("distinguishes uploaded, local-only and refresh-needed outcomes", () => {
    expect(
      initialImportResultMessage({ Synced: { save_result: "Saved" } }, 1),
    ).toBe("Imported 1 section and saved to SpeleoDB.");
    expect(
      initialImportResultMessage({ Synced: { save_result: "NoChanges" } }, 2),
    ).toBe("Imported 2 sections and saved to SpeleoDB.");
    expect(
      initialImportResultMessage(
        { LocalOnly: { error: { NetworkRequest: "offline" } } },
        3,
      ),
    ).toContain("saved on this computer. The upload did not finish.");
    expect(
      initialImportResultMessage(
        { UploadedNeedsRefresh: { error: { FileWrite: "disk full" } } },
        3,
      ),
    ).toContain(
      "uploaded to SpeleoDB, but local synchronization did not finish.",
    );
  });
  it("formats dates with the host locale and falls back for invalid dates", () => {
    const commit = {
      id: "abc",
      message: "Saved",
      author_name: "A",
      dt_since: "in 1 minute",
      commit_date: "2026-04-20T12:00:00Z",
    };
    expect(displayCommitTime(commit)).toBe(
      new Date(commit.commit_date).toLocaleString("default"),
    );
    expect(displayCommitTime({ ...commit, commit_date: "invalid" })).toBe(
      "just now",
    );
    expect(commitDateParseCandidates(" ")).toEqual([]);
  });
});
