import { createRoot } from "react-dom/client";
import { App } from "./app";
import { controller } from "./lib/controller";
import { uiStateStore } from "./lib/state";

function report(error: unknown, context: string) {
  const message =
    error instanceof Error ? error.stack || error.message : String(error);
  console.error(error);
  void controller.reportFrontendError(message, context).catch(() => {
    /* Reporting must not recurse. */
  });
}
window.addEventListener("error", (event) =>
  report(event.error ?? event.message, "javascript"),
);
window.addEventListener("unhandledrejection", (event) =>
  report(event.reason, "unhandledrejection"),
);
window.addEventListener("pagehide", () => uiStateStore.stop(), { once: true });

// Render directly into body to preserve the layout and CSS selector ancestry.
createRoot(document.body, {
  onUncaughtError: (error) => report(error, "react"),
  onCaughtError: (error) => report(error, "react"),
  onRecoverableError: (error) => report(error, "react"),
}).render(<App />);
void uiStateStore.start().catch((error) => report(error, "initialization"));
