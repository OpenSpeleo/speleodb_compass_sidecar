import type { UpdateNotification, UpdateNotificationPhase } from "../lib/types";
import { controller } from "../lib/controller";

export function updateNotificationMessage(
  phase: UpdateNotificationPhase,
): string {
  if (phase === "Checking") return "Checking for updates...";
  if ("Downloading" in phase) {
    const { version, progress_percent: progress } = phase.Downloading;
    return progress === null
      ? `Downloading update ${version}...`
      : `Downloading update ${version} (${progress}%)`;
  }
  if ("Installing" in phase)
    return `Installing update ${phase.Installing.version}...`;
  if ("Relaunching" in phase) return "Update installed. Relaunching...";
  if ("UpToDate" in phase) return `${phase.UpToDate.app_name} is up to date.`;
  return `Update failed: ${phase.Failed.message}`;
}
export function updateNotificationIsWorking(
  phase: UpdateNotificationPhase,
): boolean {
  return (
    phase === "Checking" ||
    "Downloading" in phase ||
    "Installing" in phase ||
    "Relaunching" in phase
  );
}
export function updateNotificationIsError(
  phase: UpdateNotificationPhase,
): boolean {
  return typeof phase === "object" && "Failed" in phase;
}
export function updateDismissalKey(notification: UpdateNotification): string {
  const phase = notification.phase;
  const part =
    phase === "Checking"
      ? "checking"
      : "Downloading" in phase
        ? "downloading"
        : "Installing" in phase
          ? "installing"
          : "Relaunching" in phase
            ? "relaunching"
            : "UpToDate" in phase
              ? "up-to-date"
              : "failed";
  return `${notification.id}:${part}`;
}

export function UpdateNotificationToast({
  notification,
}: {
  notification: UpdateNotification | null;
}) {
  if (notification === null) return null;
  const { phase } = notification;
  const failed = updateNotificationIsError(phase);
  const logFailure = (message: string) => (error: unknown) =>
    console.error(message, error);
  // Accessibility: the live region contains only the label and message. Its
  // non-atomic updates announce changed message text without repeating "Updates"
  // on every progress tick. Dismissal and action buttons remain outside it, where
  // users reach them through ordinary Tab navigation.
  return (
    <aside
      className={`update-notification${failed ? " update-notification--error" : ""}`}
      aria-label="Update status"
    >
      <div className="update-notification__content">
        <div className="update-notification__status" aria-hidden="true">
          {updateNotificationIsWorking(phase) ? (
            <span className="update-notification__spinner" />
          ) : (
            <span className="update-notification__dot" />
          )}
        </div>
        <div
          className="update-notification__text"
          role="status"
          aria-live="polite"
          aria-atomic="false"
        >
          <div className="update-notification__label">Updates</div>
          <div className="update-notification__message">
            {updateNotificationMessage(phase)}
          </div>
        </div>
        <button
          type="button"
          className="update-notification__dismiss"
          aria-label="Dismiss update notification"
          onClick={() => {
            void controller
              .dismissUpdateNotification(updateDismissalKey(notification))
              .catch(logFailure("Failed to dismiss update notification:"));
          }}
        >
          ×
        </button>
      </div>
      {failed && (
        <div className="update-notification__actions">
          <button
            type="button"
            onClick={() => {
              void controller
                .checkForUpdatesNow()
                .catch(logFailure("Failed to retry update check:"));
            }}
          >
            Retry
          </button>
          <button
            type="button"
            onClick={() => {
              void controller
                .openLatestRelease()
                .catch(logFailure("Failed to open latest release page:"));
            }}
          >
            Download Latest
          </button>
        </div>
      )}
    </aside>
  );
}
