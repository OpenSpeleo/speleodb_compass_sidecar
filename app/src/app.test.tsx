import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./app";
import { useUiState } from "./lib/state";
import { stateFixture } from "./components/test-fixtures";
import { loadingMessage } from "./components/loading-screen";

vi.mock("./lib/state", () => ({ useUiState: vi.fn() }));
describe("root screen parity", () => {
  it("routes loading, authentication and ready states while retaining the update toast", () => {
    const base = stateFixture();
    vi.mocked(useUiState).mockReturnValue({
      ...base,
      loading_state: "LoadingPrefs",
      update_notification: { id: 1, phase: "Checking" },
    });
    const { rerender } = render(<App />);
    expect(screen.getByText("Loading user preferences...")).toBeInTheDocument();
    expect(screen.getByLabelText("Update status")).toBeInTheDocument();
    vi.mocked(useUiState).mockReturnValue({
      ...base,
      loading_state: "Unauthenticated",
      update_notification: { id: 1, phase: "Checking" },
    });
    rerender(<App />);
    expect(screen.getByLabelText("OAUTH Token")).toBeInTheDocument();
    expect(screen.getByLabelText("Update status")).toBeInTheDocument();
    vi.mocked(useUiState).mockReturnValue(base);
    rerender(<App />);
    expect(
      screen.getByRole("heading", { name: "Projects" }),
    ).toBeInTheDocument();
  });
  it("retains every startup message and Rust error display", () => {
    expect(loadingMessage("NotStarted")).toBe("Initializing...");
    expect(loadingMessage("Authenticating")).toBe("Authenticating user...");
    expect(loadingMessage("LoadingProjects")).toBe("Loading projects...");
    expect(loadingMessage("Ready")).toBe("Starting application...");
    expect(loadingMessage({ Failed: { Unauthorized: "expired" } })).toBe(
      "Error: Unauthorized: expired",
    );
  });
});
