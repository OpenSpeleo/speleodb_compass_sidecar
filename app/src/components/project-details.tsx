/**
 * Display a project, open Compass (or its folder on non-Windows platforms),
 * commit changes with a message, and return to the project listing.
 * Read-only filesystem enforcement remains a backend concern to investigate;
 * this component preserves the existing permission and save controls.
 */
import { useEffect, useState, type CSSProperties } from "react";
import type { InitialImportOutcome, ProjectStatus } from "../lib/types";
import { controller } from "../lib/controller";
import { rustTrim } from "../lib/text";
import { Icon } from "../icons";
import { Modal } from "./modal";
import { InitialImportModal } from "./import-selector";
import { projectStatusPresentation } from "./project-status";
import {
  activeProcessingOverlay,
  displayCommitTime,
  initialImportResultMessage,
  isDirty,
  nextReimportStateAfterFilePick,
  saveCompletionState,
  shouldDisableProjectActionButtons,
  supportsInitialImport,
  validateImportCommitMessage,
  type ReimportFlow,
} from "./project-details-model";

interface ProjectDetailsProps {
  project: ProjectStatus;
  userEmail: string;
  compassOpen: boolean;
  projectDownloading: boolean;
}
const errorText = (error: unknown) =>
  error instanceof Error ? error.message : String(error);
const modalStyle: CSSProperties = {
  position: "fixed",
  top: 0,
  left: 0,
  width: "100vw",
  height: "100vh",
  backgroundColor: "rgba(0, 0, 0, 0.5)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 1000,
};
const spinnerStyle: CSSProperties = {
  border: "4px solid #e5e7eb",
  borderTopColor: "#3b82f6",
  borderRadius: "50%",
  width: "48px",
  height: "48px",
  animation: "spin 1s linear infinite",
  margin: "0 auto",
};
const buttonStyle: CSSProperties = {
  color: "white",
  border: "none",
  padding: "8px 16px",
  borderRadius: "4px",
  cursor: "pointer",
};

function ProcessingOverlay({
  title,
  message,
}: {
  title: string;
  message: string;
}) {
  return (
    <div className="modal" style={{ ...modalStyle, zIndex: 2000 }}>
      <div
        className="modal-card"
        style={{
          backgroundColor: "white",
          borderRadius: "12px",
          padding: "24px",
          maxWidth: "500px",
          width: "90%",
          boxShadow: "0 10px 25px rgba(0, 0, 0, 0.2)",
          borderTop: "4px solid #2563eb",
          textAlign: "center",
        }}
      >
        <h3
          style={{ margin: "0 0 12px 0", fontSize: "20px", color: "#1f2937" }}
        >
          {title}
        </h3>
        <p style={{ color: "#4b5563", lineHeight: 1.6, marginBottom: "20px" }}>
          {message}
        </p>
        <div style={spinnerStyle} />
      </div>
    </div>
  );
}

export function ProjectDetails({
  project,
  userEmail,
  compassOpen,
  projectDownloading,
}: ProjectDetailsProps) {
  const { info } = project;
  const dirty = isDirty(project.local_status);
  const lockedByUser =
    info.active_mutex !== null && info.active_mutex.user === userEmail;
  const readonly =
    (info.active_mutex !== null && info.active_mutex.user !== userEmail) ||
    info.permission === "READ_ONLY";
  const initialImportAvailable = supportsInitialImport(project.local_status);
  const [uploading, setUploading] = useState(false);
  // On mount, choose the read-only or empty-project prompt from the initial status.
  const [showReadonlyModal, setShowReadonlyModal] = useState(readonly);
  const [showUploadSuccess, setShowUploadSuccess] = useState(false);
  const [showNoChangesModal, setShowNoChangesModal] = useState(false);
  const [showEmptyProjectModal, setShowEmptyProjectModal] = useState(
    !readonly && initialImportAvailable,
  );
  const [showInitialImport, setShowInitialImport] = useState(false);
  const [initialImportResult, setInitialImportResult] = useState<{
    outcome: InitialImportOutcome;
    count: number;
  } | null>(null);
  const [showDiscardConfirmModal, setShowDiscardConfirmModal] = useState(false);
  const [reimportFlow, setReimportFlow] = useState<ReimportFlow>({
    phase: "Idle",
  });
  const [reimportMessage, setReimportMessage] = useState("");
  const [reimportMessageError, setReimportMessageError] = useState(false);
  const [reimporting, setReimporting] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [discardError, setDiscardError] = useState<string | null>(null);
  const [commitMessage, setCommitMessage] = useState("");
  const [commitMessageError, setCommitMessageError] = useState(false);
  const [showProblemMenu, setShowProblemMenu] = useState(false);
  const busy =
    uploading ||
    discarding ||
    reimporting ||
    projectDownloading ||
    showInitialImport;
  const disableActions = shouldDisableProjectActionButtons(compassOpen, busy);
  const showBackButton = !compassOpen && (readonly || !dirty);
  const hasProjectData = !["EmptyLocal", "RemoteOnly", "Unknown"].includes(
    project.local_status,
  );
  const status = projectStatusPresentation(project.local_status);
  const overlay = activeProcessingOverlay(reimporting, uploading);

  // The initial prompt remounts after cancellation. Native dialog focus
  // restoration cannot restore a button removed during the transition.
  useEffect(() => {
    if (!showInitialImport && showEmptyProjectModal)
      document.getElementById("initial-import-empty-trigger")?.focus();
  }, [showInitialImport, showEmptyProjectModal]);

  // Back navigation releases the mutex and clears the active project in the backend.
  async function back() {
    try {
      await controller.clearActiveProject();
    } catch (error) {
      setErrorMessage(`Could not return to projects: ${errorText(error)}`);
    }
  }
  // Open Compass on Windows or the project folder on other platforms.
  function openProject() {
    void controller.openProject(info.id).catch(() => {});
  }
  // Save Project: backend packaging, commit, upload, and resulting confirmation.
  async function save() {
    if (rustTrim(commitMessage) === "") {
      setCommitMessageError(true);
      return;
    }
    setUploading(true);
    setUploadError(null);
    try {
      const result = await controller.saveProject(info.id, commitMessage);
      setInitialImportResult(null);
      const completion = saveCompletionState(result);
      setCommitMessage("");
      setCommitMessageError(false);
      setShowUploadSuccess(completion.showUploadSuccess);
      setShowNoChangesModal(completion.showNoChangesModal);
    } catch (error) {
      setUploadError(`Failed to zip project: ${errorText(error)}`);
    } finally {
      setUploading(false);
    }
  }
  // Load from Disk: initial imports use the section-selection workflow.
  function importFromDisk() {
    if (busy || !initialImportAvailable) return;
    setErrorMessage(null);
    setInitialImportResult(null);
    setShowInitialImport(true);
  }
  // Existing data follows the overwrite warning, file picker, and commit-message flow.
  function beginReimport() {
    setReimportMessage("");
    setReimportMessageError(false);
    setReimportFlow({ phase: "ConfirmOverwrite" });
  }
  async function pickReimport() {
    setReimportFlow({ phase: "Idle" });
    try {
      const path = await controller.pickCompassProjectFile();
      setReimportMessage("");
      setReimportMessageError(false);
      setReimportFlow(nextReimportStateAfterFilePick(path));
    } catch (error) {
      setErrorMessage(
        `Failed to select Compass project file: ${errorText(error)}`,
      );
    }
  }
  async function confirmReimport() {
    if (reimportFlow.phase !== "EnterCommitMessage") return;
    if (!validateImportCommitMessage(reimportMessage)) {
      setReimportMessageError(true);
      return;
    }
    setReimporting(true);
    setErrorMessage(null);
    try {
      await controller.reimportCompassProject(
        info.id,
        reimportFlow.makPath,
        reimportMessage,
      );
      setReimportMessage("");
      setReimportMessageError(false);
      setReimportFlow({ phase: "Idle" });
    } catch (error) {
      setErrorMessage(`Failed to import project: ${errorText(error)}`);
    } finally {
      setReimporting(false);
    }
  }
  async function discard() {
    setShowDiscardConfirmModal(false);
    setDiscarding(true);
    setDiscardError(null);
    try {
      await controller.discardChanges();
    } catch (error) {
      setDiscardError(`Failed to discard changes: ${errorText(error)}`);
    } finally {
      setDiscarding(false);
    }
  }

  // Show only the success modal; the project detail surface returns when it closes.
  if (showReadonlyModal)
    return (
      <Modal
        title="Read-Only Access"
        message={`The project '${info.name}' was opened in READ-ONLY mode.\n\nModifications to this project cannot be saved because \n\n                            - the project is currently locked by another user, or\n                            - you do not have permission to edit the project\n\nContact a Project Administrator if you believe this is a mistake.`}
        modalType="Warning"
        showCloseButton
        onClose={() => setShowReadonlyModal(false)}
      />
    );

  return (
    <section style={{ width: "100%" }}>
      <div
        style={{
          width: "100%",
          marginBottom: "16px",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        {showBackButton ? (
          <button
            style={{
              ...buttonStyle,
              backgroundColor: "#10b981",
              fontWeight: 500,
            }}
            onClick={() => {
              void back();
            }}
            disabled={dirty || busy}
          >
            ← Back to Projects
          </button>
        ) : (
          <div />
        )}
        <button
          onClick={openProject}
          disabled={disableActions}
          className="project-primary-action-button"
        >
          Open in Compass
        </button>
      </div>
      {compassOpen && (
        <div
          style={{
            padding: "12px 16px",
            backgroundColor: "#dbeafe",
            borderLeft: "4px solid #3b82f6",
            borderRadius: "4px",
            marginBottom: "16px",
          }}
        >
          <strong style={{ color: "#1e40af" }}>Compass is open</strong>
          <p style={{ color: "#1e3a8a", marginTop: "4px", fontSize: "14px" }}>
            Please close Compass before navigating back to prevent losing
            unsaved work.
          </p>
        </div>
      )}
      {lockedByUser && (
        <div
          style={{
            padding: "12px 16px",
            backgroundColor: "#fef3c7",
            borderLeft: "4px solid #f59e0b",
            borderRadius: "4px",
            marginBottom: "16px",
          }}
        >
          <strong style={{ color: "#92400e" }}>Project Locked</strong>
          <p style={{ color: "#78350f", marginTop: "4px", fontSize: "14px" }}>
            This project is locked and unavailable to other users until you
            close it.
          </p>
        </div>
      )}
      <h2>
        <strong>Project: </strong>
        {info.name}
      </h2>
      <p style={{ color: "#6b7280", fontSize: "14px" }}>{`ID: ${info.id}`}</p>
      {/* Project status indicator */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: "4px",
          padding: "10px 14px",
          backgroundColor: "#f9fafb",
          borderRadius: "6px",
          borderLeft: `3px solid ${status.color}`,
          margin: "8px 0 12px 0",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <span
            style={{
              color: status.color,
              display: "flex",
              alignItems: "center",
            }}
          >
            <Icon name={status.icon} />
          </span>
          <span style={{ fontWeight: 600, color: status.color }}>
            {status.text}
          </span>
        </div>
        {info.latest_commit !== null && (
          <p
            style={{ margin: "2px 0 0 0", fontSize: "13px", color: "#6b7280" }}
          >{`Latest: "${info.latest_commit.message}" by ${info.latest_commit.author_name} (${displayCommitTime(info.latest_commit)})`}</p>
        )}
      </div>
      {!readonly && !hasProjectData && (
        <div
          style={{
            display: "flex",
            justifyContent: "center",
            marginTop: "12px",
            marginBottom: "4px",
          }}
        >
          <button
            onClick={initialImportAvailable ? importFromDisk : beginReimport}
            disabled={disableActions}
            className="project-primary-action-button"
          >
            {reimporting ? "Importing..." : "Import Compass Project From Disk"}
          </button>
        </div>
      )}
      {initialImportResult !== null && (
        <div
          className={`initial-import-result${"Synced" in initialImportResult.outcome ? "" : " initial-import-result--warning"}`}
          role="status"
          aria-live="polite"
        >
          {initialImportResultMessage(
            initialImportResult.outcome,
            initialImportResult.count,
          )}
        </div>
      )}
      {projectDownloading ? (
        <div
          style={{
            padding: "24px",
            textAlign: "center",
            backgroundColor: "#f3f4f6",
            borderRadius: "8px",
            margin: "20px 0",
          }}
        >
          <div style={{ ...spinnerStyle, margin: "0 auto 16px" }} />
          <p style={{ color: "#4b5563", fontSize: "16px" }}>
            Downloading and extracting project...
          </p>
        </div>
      ) : errorMessage !== null ? (
        <div
          style={{
            padding: "16px",
            backgroundColor: "#fee2e2",
            border: "1px solid #ef4444",
            borderRadius: "8px",
            margin: "20px 0",
          }}
        >
          <strong style={{ color: "#dc2626" }}>Error: </strong>
          <span style={{ color: "#991b1b" }}>{errorMessage}</span>
        </div>
      ) : null}
      {/* Read-only notice, or the commit section for writable projects with local changes. */}
      {readonly ? (
        <div
          style={{
            padding: "12px 16px",
            backgroundColor: "#fef3c7",
            borderLeft: "4px solid #f59e0b",
            borderRadius: "4px",
            margin: "20px 0",
          }}
        >
          <strong style={{ color: "#92400e" }}>⚠️ Read-Only Mode</strong>
          <p style={{ color: "#78350f", marginTop: "4px", fontSize: "14px" }}>
            This project is opened in read-only mode. Modifications cannot be
            saved.
          </p>
        </div>
      ) : dirty ? (
        <div
          style={{
            marginTop: "24px",
            paddingTop: "24px",
            borderTop: "1px solid #e5e7eb",
          }}
        >
          <h3 style={{ marginBottom: "12px" }}>
            Compass project has changes. Before you can go back, you need to
            describe and save your work.
          </h3>
          <div
            style={{
              marginBottom: "16px",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <textarea
              rows={4}
              value={commitMessage}
              onChange={(event) => {
                setCommitMessage(event.currentTarget.value);
                if (rustTrim(event.currentTarget.value) !== "")
                  setCommitMessageError(false);
              }}
              placeholder="Describe your changes (max 255 characters)"
              maxLength={255}
              style={{
                width: "100%",
                boxSizing: "border-box",
                padding: "8px",
                border: `1px solid ${commitMessageError ? "#ef4444" : "#d1d5db"}`,
                borderRadius: "4px",
                fontFamily: "inherit",
              }}
            />
            {commitMessageError && (
              <div>
                <p
                  style={{
                    color: "#ef4444",
                    fontSize: "12px",
                    marginTop: "4px",
                  }}
                >
                  Please, enter a commit message.
                </p>
              </div>
            )}
          </div>
          <div style={{ display: "flex", gap: "12px" }}>
            <button
              onClick={() => {
                void save();
              }}
              disabled={!dirty || busy}
              style={{ ...buttonStyle, backgroundColor: "#2563eb" }}
            >
              {uploading ? "Saving..." : "Save Project"}
            </button>
            <button
              onClick={() => setShowDiscardConfirmModal(true)}
              disabled={busy}
              style={{ ...buttonStyle, backgroundColor: "#dc2626" }}
            >
              {discarding ? "Discarding..." : "Discard Changes"}
            </button>
          </div>
          {(uploadError ?? discardError) !== null && (
            <div
              style={{
                marginTop: "12px",
                color: "#dc2626",
                fontSize: "14px",
                textAlign: "center",
              }}
            >{`Error: ${uploadError ?? discardError}`}</div>
          )}
        </div>
      ) : null}
      {/* Dismissing the empty-project prompt returns to the project listing. */}
      {showEmptyProjectModal && !showInitialImport && (
        <Modal
          title="Empty Project"
          message={
            "This project contains no Compass data yet.\n\nTo initialize the project, use the 'Import from Disk' button to upload your local project files."
          }
          modalType="Info"
          showCloseButton
          closeButtonText="Back to Projects"
          onClose={() => {
            setShowEmptyProjectModal(false);
            void back();
          }}
          primaryButtonText="Import Compass Project From Disk"
          primaryButtonId="initial-import-empty-trigger"
          onPrimaryAction={importFromDisk}
        />
      )}
      {showInitialImport && (
        <InitialImportModal
          projectId={info.id}
          onClose={() => {
            setShowInitialImport(false);
            setShowEmptyProjectModal(true);
          }}
          onComplete={(outcome, count) => {
            if (!("Synced" in outcome))
              setCommitMessage("Imported local project");
            setShowInitialImport(false);
            setShowEmptyProjectModal(false);
            setInitialImportResult({ outcome, count });
          }}
        />
      )}
      {/* Reimport warning and commit-message flow */}
      {reimportFlow.phase === "ConfirmOverwrite" && (
        <Modal
          title="Import Project From Disk?"
          message={
            "You are about to import a local Compass project into the current project.\n\nWarning: this may overwrite existing project data:\n- Existing unsaved data may be lost.\n- The imported files will become the current project content.\n- This action cannot be undone.\n\nDo you want to proceed?"
          }
          modalType="Warning"
          showCloseButton
          closeButtonText="Cancel"
          primaryButtonText="Proceed"
          onClose={() => setReimportFlow({ phase: "Idle" })}
          onPrimaryAction={() => {
            void pickReimport();
          }}
        />
      )}
      {reimportFlow.phase === "EnterCommitMessage" && (
        <div className="modal" style={modalStyle}>
          <div
            className="modal-card"
            style={{
              backgroundColor: "white",
              borderRadius: "12px",
              padding: "24px",
              maxWidth: "560px",
              width: "90%",
              boxShadow: "0 10px 25px rgba(0, 0, 0, 0.2)",
              borderTop: "4px solid #3b82f6",
            }}
          >
            <h3
              style={{
                margin: "0 0 12px 0",
                fontSize: "20px",
                color: "#1f2937",
              }}
            >
              Import Message
            </h3>
            <p
              style={{
                color: "#4b5563",
                marginBottom: "8px",
                whiteSpace: "pre-line",
              }}
            >
              Enter a commit message for this import.
            </p>
            <p
              style={{
                color: "#6b7280",
                marginBottom: "12px",
                fontSize: "12px",
                wordBreak: "break-word",
              }}
            >{`Selected file: ${reimportFlow.makPath}`}</p>
            <textarea
              rows={4}
              value={reimportMessage}
              onChange={(event) => {
                setReimportMessage(event.currentTarget.value);
                if (validateImportCommitMessage(event.currentTarget.value))
                  setReimportMessageError(false);
              }}
              placeholder="Describe this import (max 255 characters)"
              maxLength={255}
              disabled={reimporting}
              style={{
                width: "100%",
                padding: "8px",
                border: `1px solid ${reimportMessageError ? "#ef4444" : "#d1d5db"}`,
                borderRadius: "6px",
                boxSizing: "border-box",
                fontFamily: "inherit",
              }}
            />
            {reimportMessageError && (
              <p
                style={{ color: "#ef4444", fontSize: "12px", marginTop: "6px" }}
              >
                Please enter an import message before continuing.
              </p>
            )}
            <div
              style={{
                display: "flex",
                justifyContent: "flex-end",
                gap: "12px",
                marginTop: "16px",
              }}
            >
              <button
                onClick={() => {
                  setReimportMessage("");
                  setReimportMessageError(false);
                  setReimportFlow({ phase: "Idle" });
                }}
                disabled={reimporting}
                style={{
                  padding: "8px 16px",
                  border: "1px solid #d1d5db",
                  borderRadius: "6px",
                  backgroundColor: "white",
                  color: "#374151",
                  cursor: "pointer",
                }}
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  void confirmReimport();
                }}
                disabled={reimporting}
                style={{
                  padding: "8px 16px",
                  border: "none",
                  borderRadius: "6px",
                  backgroundColor: "#2563eb",
                  color: "white",
                  cursor: "pointer",
                  fontWeight: 500,
                }}
              >
                {reimporting ? "Importing..." : "Import Project"}
              </button>
            </div>
          </div>
        </div>
      )}
      {/* Discard Changes Confirmation Modal */}
      {showDiscardConfirmModal && (
        <Modal
          title="Discard Changes?"
          message={
            "This will permanently discard all local changes and replace your working copy with the latest version from the server.\n\nThis action cannot be undone."
          }
          modalType="Warning"
          showCloseButton
          primaryButtonText="Discard Changes"
          onClose={() => setShowDiscardConfirmModal(false)}
          onPrimaryAction={() => {
            void discard();
          }}
        />
      )}
      {/* Upload Success Modal */}
      {showUploadSuccess && (
        <Modal
          title="Project Saved Successfully"
          message="Your changes have been successfully committed and uploaded to the server."
          modalType="Success"
          showCloseButton
          onClose={() => setShowUploadSuccess(false)}
        />
      )}
      {/* No Changes Modal (304) */}
      {showNoChangesModal && (
        <Modal
          title="No Changes Detected"
          message={
            "The project on the server is already identical to your local version. \n\nNo changes were saved."
          }
          modalType="Warning"
          showCloseButton
          onClose={() => setShowNoChangesModal(false)}
        />
      )}
      {/* Processing overlay for long-running import/save operations */}
      {overlay === "Importing" ? (
        <ProcessingOverlay
          title="Importing Project"
          message="Processing Compass files and syncing the project. This can take a moment."
        />
      ) : overlay === "Saving" ? (
        <ProcessingOverlay
          title="Saving Project"
          message="Uploading your Compass project to SpeleoDB. This can take a moment."
        />
      ) : null}
      {/* "Problem?" menu for reimport when the project already has data */}
      {!readonly && hasProjectData && (
        <div style={{ position: "fixed", bottom: "16px", left: "16px" }}>
          <div style={{ position: "relative" }}>
            {showProblemMenu && (
              <div
                style={{
                  position: "absolute",
                  bottom: "36px",
                  left: 0,
                  background: "white",
                  border: "1px solid #e5e7eb",
                  borderRadius: "6px",
                  boxShadow: "0 4px 12px rgba(0, 0, 0, 0.1)",
                  minWidth: "200px",
                  zIndex: 100,
                }}
              >
                <button
                  onClick={() => {
                    setShowProblemMenu(false);
                    beginReimport();
                  }}
                  disabled={disableActions}
                  style={{
                    width: "100%",
                    padding: "10px 14px",
                    border: "none",
                    background: "none",
                    textAlign: "left",
                    cursor: "pointer",
                    fontSize: "13px",
                    color: "#374151",
                    borderRadius: "6px",
                  }}
                >
                  Re-import from Disk
                </button>
              </div>
            )}
            <button
              onClick={() => setShowProblemMenu((value) => !value)}
              style={{
                background: "none",
                border: "1px solid #d1d5db",
                borderRadius: "4px",
                padding: "4px 10px",
                fontSize: "12px",
                color: "#9ca3af",
                cursor: "pointer",
              }}
            >
              Problem?
            </button>
          </div>
        </div>
      )}
      {/* Spinner animation */}
      <style>
        {
          "@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }"
        }
      </style>
    </section>
  );
}
