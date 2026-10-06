import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { InitialImportOutcome, LocalProjectStatus } from "../lib/types";
import { controller } from "../lib/controller";
import { ProjectDetails } from "./project-details";
import { MainLayout } from "./main-layout";
import { projectFixture, stateFixture } from "./test-fixtures";

vi.mock("../lib/controller", () => ({
  controller: {
    openProject: vi.fn().mockResolvedValue(undefined),
    clearActiveProject: vi.fn().mockResolvedValue(undefined),
    saveProject: vi.fn().mockResolvedValue("Saved"),
    discardChanges: vi.fn().mockResolvedValue(undefined),
    pickCompassProjectFile: vi.fn().mockResolvedValue("/tmp/Cave.mak"),
    reimportCompassProject: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("./import-selector", () => ({
  InitialImportModal: ({
    onClose,
    onComplete,
  }: {
    onClose: () => void;
    onComplete: (outcome: InitialImportOutcome, count: number) => void;
  }) => (
    <div data-testid="initial-import">
      <button onClick={onClose}>Cancel import</button>
      <button
        onClick={() =>
          onComplete({ LocalOnly: { error: { NetworkRequest: "offline" } } }, 2)
        }
      >
        Complete local import
      </button>
    </div>
  ),
}));
const props = (localStatus: LocalProjectStatus = "UpToDate") => ({
  project: projectFixture({ local_status: localStatus }),
  userEmail: "me@example.com",
  compassOpen: false,
  projectDownloading: false,
});

describe("project detail interaction parity", () => {
  it("renders each project status and its current action availability", () => {
    const cases: [LocalProjectStatus, string][] = [
      ["Unknown", "Unknown"],
      ["RemoteOnly", "Available for Download"],
      ["EmptyLocal", "Empty Project"],
      ["Dirty", "Unsaved Local Changes"],
      ["UpToDate", "Up to Date"],
      ["OutOfDate", "Update Available"],
      ["DirtyAndOutOfDate", "Local Changes & Update Available"],
    ];
    const { rerender } = render(<ProjectDetails {...props()} />);
    for (const [state, label] of cases) {
      rerender(<ProjectDetails {...props(state)} />);
      expect(screen.getByText(label)).toBeInTheDocument();
      expect(
        screen.queryByRole("button", { name: "Save Project" }) !== null,
      ).toBe(state === "Dirty" || state === "DirtyAndOutOfDate");
    }
  });
  it("returns only the read-only modal on mount and shows detail after dismissal", () => {
    const parameters = props();
    parameters.project.info.permission = "READ_ONLY";
    render(<ProjectDetails {...parameters} />);
    expect(screen.getByText("Read-Only Access")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Open in Compass" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByText("⚠️ Read-Only Mode")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Open in Compass" }),
    ).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: "Problem?" }),
    ).not.toBeInTheDocument();
  });
  it("treats another user’s lock as read-only", () => {
    const parameters = props();
    parameters.project.info.active_mutex = {
      user: "other@example.com",
      creation_date: "",
      modified_date: "",
    };
    render(<ProjectDetails {...parameters} />);
    expect(screen.getByText("Read-Only Access")).toBeInTheDocument();
  });
  it("keeps save available but disables open/back while Compass is open", () => {
    render(<ProjectDetails {...props("Dirty")} compassOpen />);
    expect(
      screen.getByRole("button", { name: "Open in Compass" }),
    ).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: "← Back to Projects" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save Project" })).toBeEnabled();
    expect(screen.getByText("Compass is open")).toBeInTheDocument();
  });
  it("requires a commit message and shows the saving overlay before success", async () => {
    let resolve!: (value: "Saved") => void;
    vi.mocked(controller.saveProject).mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    render(<ProjectDetails {...props("Dirty")} />);
    fireEvent.click(screen.getByRole("button", { name: "Save Project" }));
    expect(
      screen.getByText("Please, enter a commit message."),
    ).toBeInTheDocument();
    fireEvent.change(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
      { target: { value: " Survey changes " } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Save Project" }));
    expect(screen.getByText("Saving Project")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Saving..." })).toBeDisabled();
    expect(controller.saveProject).toHaveBeenCalledWith(
      props().project.info.id,
      " Survey changes ",
    );
    await act(async () => resolve("Saved"));
    expect(screen.getByText("Project Saved Successfully")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
    ).toHaveValue("");
    expect(screen.queryByText("Saving Project")).not.toBeInTheDocument();
  });
  it("shows no changes and preserves exact upload errors", async () => {
    vi.mocked(controller.saveProject)
      .mockResolvedValueOnce("NoChanges")
      .mockRejectedValueOnce(new Error("offline"));
    render(<ProjectDetails {...props("Dirty")} />);
    const message = screen.getByPlaceholderText(
      "Describe your changes (max 255 characters)",
    );
    fireEvent.change(message, { target: { value: "First save" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Project" }));
    await screen.findByText("No Changes Detected");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    fireEvent.change(message, { target: { value: "Retry save" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Project" }));
    await screen.findByText("Error: Failed to zip project: offline");
    expect(message).toHaveValue("Retry save");
  });
  it("confirms discard and invokes the existing command without a project argument", async () => {
    render(<ProjectDetails {...props("Dirty")} />);
    fireEvent.click(screen.getByRole("button", { name: "Discard Changes" }));
    const modal = screen.getByText("Discard Changes?").closest(".modal")!;
    fireEvent.click(
      within(modal as HTMLElement).getByRole("button", {
        name: "Discard Changes",
      }),
    );
    await waitFor(() =>
      expect(controller.discardChanges).toHaveBeenCalledWith(),
    );
  });
  it("restores the empty project prompt and its focus after cancelling initial import", () => {
    render(<ProjectDetails {...props("EmptyLocal")} />);
    const trigger = document.getElementById("initial-import-empty-trigger")!;
    expect(trigger).toHaveFocus();
    fireEvent.click(trigger);
    expect(screen.getByTestId("initial-import")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel import" }));
    expect(
      document.getElementById("initial-import-empty-trigger"),
    ).toHaveFocus();
  });
  it("preserves locally imported selections and supplies a retry commit message", () => {
    const { rerender } = render(<ProjectDetails {...props("EmptyLocal")} />);
    fireEvent.click(document.getElementById("initial-import-empty-trigger")!);
    fireEvent.click(
      screen.getByRole("button", { name: "Complete local import" }),
    );
    rerender(<ProjectDetails {...props("Dirty")} />);
    expect(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
    ).toHaveValue("Imported local project");
    expect(screen.getByRole("status")).toHaveTextContent(
      "Your 2 imported sections are saved on this computer.",
    );
  });
  it("performs overwrite warning, native picker, message and reimport in order", async () => {
    render(<ProjectDetails {...props()} />);
    fireEvent.click(screen.getByRole("button", { name: "Problem?" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Re-import from Disk" }),
    );
    expect(screen.getByText("Import Project From Disk?")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Proceed" }));
    await screen.findByText("Selected file: /tmp/Cave.mak");
    fireEvent.click(screen.getByRole("button", { name: "Import Project" }));
    expect(
      screen.getByText("Please enter an import message before continuing."),
    ).toBeInTheDocument();
    fireEvent.change(
      screen.getByPlaceholderText("Describe this import (max 255 characters)"),
      { target: { value: "New survey" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Import Project" }));
    await waitFor(() =>
      expect(controller.reimportCompassProject).toHaveBeenCalledWith(
        props().project.info.id,
        "/tmp/Cave.mak",
        "New survey",
      ),
    );
    await waitFor(() =>
      expect(screen.queryByText("Import Message")).not.toBeInTheDocument(),
    );
  });
  it("returns to idle when the reimport picker is cancelled", async () => {
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValueOnce(null);
    render(<ProjectDetails {...props()} />);
    fireEvent.click(screen.getByRole("button", { name: "Problem?" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Re-import from Disk" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Proceed" }));
    await waitFor(() =>
      expect(controller.pickCompassProjectFile).toHaveBeenCalledOnce(),
    );
    expect(screen.queryByText("Import Message")).not.toBeInTheDocument();
  });
  it("retains local form state during polling but resets it when the selected project changes", () => {
    const first = projectFixture({ local_status: "Dirty" });
    const second = projectFixture({
      local_status: "Dirty",
      info: { ...first.info, id: "second", name: "Second" },
    });
    const state = {
      ...stateFixture([first, second]),
      selected_project_id: first.info.id,
    };
    const { rerender } = render(<MainLayout uiState={state} />);
    fireEvent.change(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
      { target: { value: "Draft" } },
    );
    rerender(
      <MainLayout
        uiState={{ ...state, project_status: [{ ...first }, second] }}
      />,
    );
    expect(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
    ).toHaveValue("Draft");
    rerender(
      <MainLayout
        uiState={{ ...state, selected_project_id: second.info.id }}
      />,
    );
    expect(
      screen.getByPlaceholderText("Describe your changes (max 255 characters)"),
    ).toHaveValue("");
  });
});
