import type { LoadingState } from "../lib/types";
import { formatBackendError } from "../lib/errors";

export function loadingMessage(state: LoadingState): string {
  if (typeof state === "object")
    return `Error: ${formatBackendError(state.Failed)}`;
  switch (state) {
    case "NotStarted":
      return "Initializing...";
    case "LoadingPrefs":
      return "Loading user preferences...";
    case "Authenticating":
      return "Authenticating user...";
    case "LoadingProjects":
      return "Loading projects...";
    default:
      return "Starting application...";
  }
}

export function LoadingScreen({
  loadingState,
}: {
  loadingState: LoadingState;
}) {
  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        zIndex: 9999,
        backdropFilter: "blur(2px)",
      }}
    >
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <img
          src="public/speleodb_long.png"
          className="logo speleodb"
          alt="SpeleoDB logo"
          style={{ width: "80%", height: "auto" }}
        />
        <div className="container" style={{ width: "100%", height: "100%" }}>
          <div
            style={{
              padding: "32px 48px",
              borderRadius: "12px",
              display: "flex",
              flexDirection: "column",
              justifyContent: "center",
              alignItems: "center",
            }}
          >
            <div className="spinner" />
            <p
              style={{
                paddingTop: "16px",
                color: "rgb(255,255,255,0.7)",
                textShadow: "2px 2px 2px rgb(0,0,0,0.8)",
                fontSize: "18px",
                fontWeight: 500,
                margin: 0,
              }}
            >
              {loadingMessage(loadingState)}
            </p>
          </div>
        </div>
      </div>
      <style>
        {
          "@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }"
        }
      </style>
    </div>
  );
}
