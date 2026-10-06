import { StrictMode } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { controller } from "../lib/controller";
import { FrontendError } from "../lib/errors";
import type { ImportPreview, InitialImportOutcome } from "../lib/types";
import {
  ImportDialog,
  ImportRequests,
  InitialImportModal,
  SectionSelector,
} from "./import-selector";

vi.mock("../lib/controller", () => ({
  controller: {
    pickCompassProjectFile: vi.fn(),
    previewCompassImport: vi.fn(),
    cancelCompassImport: vi.fn(),
    confirmCompassImport: vi.fn(),
  },
}));
const preview = (id = "preview"): ImportPreview => ({
  preview_id: id,
  source_name: "Cave.mak",
  source_directory: "Surveys/Cave",
  full_import_reason: null,
  sections: [0, 1, 2].map((id) => ({
    id,
    name: `${String.fromCharCode(65 + id)}.DAT`,
    relative_path: `region/${id}.DAT`,
    dependencies:
      id === 1 ? [{ section_id: 0, reason: "Connects at station A1" }] : [],
  })),
});
const props = () => ({
  preview: preview(),
  onConfirm: vi.fn(),
  onCancel: vi.fn(),
  onChooseFile: vi.fn(),
  onRefresh: vi.fn(),
});
function checkbox(id: number): HTMLInputElement {
  return document.getElementById(`import-section-${id}`) as HTMLInputElement;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
beforeEach(() => {
  vi.resetAllMocks();
  for (const method of ["showModal", "close"] as const) {
    if (typeof HTMLDialogElement.prototype[method] !== "function") {
      Object.defineProperty(HTMLDialogElement.prototype, method, {
        configurable: true,
        writable: true,
        value() {},
      });
    }
  }
  vi.spyOn(HTMLDialogElement.prototype, "showModal").mockImplementation(
    function (this: HTMLDialogElement) {
      this.open = true;
    },
  );
  vi.spyOn(HTMLDialogElement.prototype, "close").mockImplementation(function (
    this: HTMLDialogElement,
  ) {
    this.open = false;
  });
  vi.mocked(controller.cancelCompassImport).mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("selection dialog parity", () => {
  it("defaults to all, locks dependencies and submits explicit roots only", () => {
    const properties = props();
    render(<SectionSelector {...properties} />);
    expect(checkbox(0).checked).toBe(true);
    expect(checkbox(0).disabled).toBe(true);
    expect(screen.getByText("Required by B.DAT")).toBeTruthy();
    expect(screen.queryByText("B.DAT: Connects at station A1")).toBeNull();
    fireEvent.click(screen.getByText("Clear selection"));
    expect(checkbox(0).checked).toBe(false);
    expect(
      (screen.getByText("Import 0 sections") as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(checkbox(1));
    expect(checkbox(0).checked).toBe(true);
    expect(checkbox(0).disabled).toBe(true);
    expect(screen.getByText("1 included automatically")).toBeTruthy();
    fireEvent.click(screen.getByText("Import 2 sections"));
    expect(properties.onConfirm).toHaveBeenCalledWith([1]);
    fireEvent.click(checkbox(1));
    expect(checkbox(0).checked).toBe(false);
  });
  it("renders connection details only when expanded", async () => {
    render(<SectionSelector {...props()} />);
    const details = document.querySelector("details")!;
    details.open = true;
    fireEvent(details, new Event("toggle"));
    expect(
      await screen.findByText("B.DAT: Connects at station A1"),
    ).toBeTruthy();
  });
  it("keeps filtered sections in bulk selection, and offers search recovery", () => {
    render(<SectionSelector {...props()} />);
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "C.DAT" },
    });
    expect(screen.getAllByRole("checkbox")).toHaveLength(1);
    fireEvent.click(screen.getByText("Clear selection"));
    expect(screen.getByText("0 of 3 sections")).toBeTruthy();
    fireEvent.click(screen.getByText("Select all"));
    expect(screen.getByText("3 of 3 sections")).toBeTruthy();
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "missing" },
    });
    expect(screen.getByText("No matching sections")).toBeTruthy();
    fireEvent.click(screen.getByText("Clear search"));
    expect(screen.getAllByRole("checkbox")).toHaveLength(3);
  });
  it("blocks stale previews and full-only changes", () => {
    const properties = props();
    const { rerender } = render(
      <SectionSelector {...properties} needsRefresh error="A.DAT changed" />,
    );
    expect(screen.getByText("The source files changed")).toBeTruthy();
    expect(
      (screen.getByText("Import 3 sections") as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByText("Refresh preview"));
    expect(properties.onRefresh).toHaveBeenCalledOnce();
    rerender(
      <SectionSelector
        {...properties}
        preview={{
          ...properties.preview,
          full_import_reason: "Nested project files",
        }}
      />,
    );
    expect(screen.queryByText("Clear selection")).toBeNull();
    expect(checkbox(2).disabled).toBe(true);
    expect(
      (screen.getByText("Import complete project") as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });
  it("focuses the heading and prevents native Escape cancellation when busy", () => {
    const properties = props();
    const { rerender } = render(
      <ImportDialog onCancel={properties.onCancel}>
        <SectionSelector {...properties} />
      </ImportDialog>,
    );
    expect(document.activeElement?.id).toBe("import-selector-title");
    const dialog = document.querySelector("dialog")!;
    expect(dialog.open).toBe(true);
    fireEvent(dialog, new Event("cancel", { cancelable: true }));
    expect(properties.onCancel).toHaveBeenCalledOnce();
    rerender(
      <ImportDialog busy onCancel={properties.onCancel}>
        <SectionSelector {...properties} busy />
      </ImportDialog>,
    );
    fireEvent(dialog, new Event("cancel", { cancelable: true }));
    expect(properties.onCancel).toHaveBeenCalledOnce();
    expect((screen.getByText("Importing…") as HTMLButtonElement).disabled).toBe(
      true,
    );
    expect(document.querySelector(".modal")).toBeNull();
  });
  it("does not mount hidden dependency trees for 500 sections", () => {
    const properties = props();
    properties.preview.sections = Array.from({ length: 500 }, (_, id) => ({
      id,
      name: `Passage ${id}.DAT`,
      relative_path: `Passage ${id}.DAT`,
      dependencies: id
        ? [{ section_id: id - 1, reason: "Shared station" }]
        : [],
    }));
    render(<SectionSelector {...properties} />);
    expect(screen.getAllByRole("checkbox")).toHaveLength(500);
    expect(document.querySelectorAll("li")).toHaveLength(0);
    expect(screen.getByText("500 of 500 sections")).toBeTruthy();
    expect(document.querySelector("footer")).toBeTruthy();
  });
});

describe("native picker and asynchronous import lifecycle", () => {
  it("closes a cancelled first picker without showing a dialog, including StrictMode replay", async () => {
    const onClose = vi.fn();
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue(null);
    render(
      <StrictMode>
        <InitialImportModal
          projectId="project"
          onClose={onClose}
          onComplete={vi.fn()}
        />
      </StrictMode>,
    );
    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(controller.pickCompassProjectFile).toHaveBeenCalledOnce();
    expect(document.querySelector("dialog")).toBeNull();
  });
  it("ignores late picker results after unmount", async () => {
    const picker = deferred<string | null>();
    vi.mocked(controller.pickCompassProjectFile).mockReturnValue(
      picker.promise,
    );
    const { unmount } = render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    await waitFor(() =>
      expect(controller.pickCompassProjectFile).toHaveBeenCalledOnce(),
    );
    unmount();
    await act(async () => {
      picker.resolve("Cave.mak");
    });
    expect(controller.previewCompassImport).not.toHaveBeenCalled();
  });
  it("releases late analysis previews after cancellation", async () => {
    const analysis = deferred<ImportPreview>();
    const onClose = vi.fn();
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport).mockReturnValue(
      analysis.promise,
    );
    render(
      <InitialImportModal
        projectId="project"
        onClose={onClose}
        onComplete={vi.fn()}
      />,
    );
    await screen.findByText("Reading your project");
    fireEvent.click(screen.getByText("Cancel"));
    expect(onClose).toHaveBeenCalledOnce();
    await act(async () => {
      analysis.resolve(preview());
    });
    expect(controller.cancelCompassImport).toHaveBeenCalledWith("preview");
    expect(screen.queryByText("Import 3 sections")).toBeNull();
  });
  it("shows analysis failures and retries the same path", async () => {
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport)
      .mockRejectedValueOnce(new FrontendError("Command", "Read failed"))
      .mockResolvedValueOnce(preview());
    render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    expect(
      await screen.findByText("This project needs a closer look"),
    ).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toBe("Read failed");
    fireEvent.click(screen.getByText("Try again"));
    await screen.findByText("Import 3 sections");
    expect(controller.previewCompassImport).toHaveBeenLastCalledWith(
      "project",
      "Cave.mak",
    );
    expect(controller.pickCompassProjectFile).toHaveBeenCalledOnce();
  });
  it("keeps a reviewed selection after a picker error and cancels it on unmount", async () => {
    vi.mocked(controller.pickCompassProjectFile)
      .mockResolvedValueOnce("Cave.mak")
      .mockRejectedValueOnce(new FrontendError("Command", "Picker failed"));
    vi.mocked(controller.previewCompassImport).mockResolvedValue(preview());
    const { unmount } = render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Change file"));
    await screen.findByText("Picker failed");
    expect(screen.getAllByRole("checkbox")).toHaveLength(3);
    unmount();
    expect(controller.cancelCompassImport).toHaveBeenCalledWith("preview");
  });
  it("keeps ordinary confirmation failures retryable without refreshing", async () => {
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport).mockResolvedValue(preview());
    vi.mocked(controller.confirmCompassImport).mockRejectedValue(
      new FrontendError("Command", "Request failed"),
    );
    render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Import 3 sections"));
    await screen.findByText("Request failed");
    expect(
      (screen.getByText("Import 3 sections") as HTMLButtonElement).disabled,
    ).toBe(false);
    expect(screen.queryByText("The source files changed")).toBeNull();
  });
  it("restores the prior preview when Change file is cancelled", async () => {
    vi.mocked(controller.pickCompassProjectFile)
      .mockResolvedValueOnce("Cave.mak")
      .mockResolvedValueOnce(null);
    vi.mocked(controller.previewCompassImport).mockResolvedValue(preview());
    render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Change file"));
    await waitFor(() =>
      expect(controller.pickCompassProjectFile).toHaveBeenCalledTimes(2),
    );
    await waitFor(() =>
      expect(
        (screen.getByText("Import 3 sections") as HTMLButtonElement).disabled,
      ).toBe(false),
    );
    expect(controller.cancelCompassImport).not.toHaveBeenCalled();
  });
  it("prevents duplicate submission, keeps cancellation blocked and reports selected closure count", async () => {
    const pending = deferred<InitialImportOutcome>();
    const onComplete = vi.fn();
    const onClose = vi.fn();
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport).mockResolvedValue(preview());
    vi.mocked(controller.confirmCompassImport).mockReturnValue(pending.promise);
    render(
      <InitialImportModal
        projectId="project"
        onClose={onClose}
        onComplete={onComplete}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Clear selection"));
    fireEvent.click(checkbox(1));
    const submit = screen.getByText("Import 2 sections");
    fireEvent.click(submit);
    fireEvent.click(submit);
    expect(controller.confirmCompassImport).toHaveBeenCalledExactlyOnceWith(
      "preview",
      [1],
    );
    fireEvent(
      document.querySelector("dialog")!,
      new Event("cancel", { cancelable: true }),
    );
    expect(onClose).not.toHaveBeenCalled();
    const outcome: InitialImportOutcome = {
      LocalOnly: { error: { NetworkRequest: "offline" } },
    };
    await act(async () => {
      pending.resolve(outcome);
    });
    expect(onComplete).toHaveBeenCalledWith(outcome, 2);
  });
  it("requires refresh after source change, resets selection and releases previous preview", async () => {
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport)
      .mockResolvedValueOnce(preview())
      .mockResolvedValueOnce(preview("fresh"));
    vi.mocked(controller.confirmCompassImport).mockRejectedValue(
      new FrontendError("ImportSourceChanged", "A.DAT changed"),
    );
    render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Clear selection"));
    fireEvent.click(checkbox(2));
    fireEvent.click(screen.getByText("Import 1 section"));
    await screen.findByText("The source files changed");
    expect(
      (screen.getByText("Import 1 section") as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByText("Refresh preview"));
    await screen.findByText("Import 3 sections");
    expect(controller.cancelCompassImport).toHaveBeenCalledWith("preview");
    expect(controller.previewCompassImport).toHaveBeenCalledTimes(2);
  });
  it("releases preview on unmount but waits until a pending submit settles", async () => {
    const pending = deferred<InitialImportOutcome>();
    vi.mocked(controller.pickCompassProjectFile).mockResolvedValue("Cave.mak");
    vi.mocked(controller.previewCompassImport).mockResolvedValue(preview());
    vi.mocked(controller.confirmCompassImport).mockReturnValue(pending.promise);
    const onComplete = vi.fn();
    const { unmount } = render(
      <InitialImportModal
        projectId="project"
        onClose={vi.fn()}
        onComplete={onComplete}
      />,
    );
    await screen.findByText("Import 3 sections");
    fireEvent.click(screen.getByText("Import 3 sections"));
    unmount();
    expect(controller.cancelCompassImport).not.toHaveBeenCalled();
    await act(async () => {
      pending.resolve({ Synced: { save_result: "Saved" } });
    });
    expect(controller.cancelCompassImport).toHaveBeenCalledWith("preview");
    expect(onComplete).not.toHaveBeenCalled();
  });
});

describe("synchronous request guards", () => {
  it("invalidates replaced and unmounted generations", () => {
    const requests = new ImportRequests();
    const first = requests.begin();
    expect(requests.accepts(first)).toBe(true);
    const second = requests.begin();
    expect(requests.accepts(first)).toBe(false);
    expect(requests.accepts(second)).toBe(true);
    requests.mounted = false;
    expect(requests.accepts(second)).toBe(false);
  });
  it("serializes reads and submissions before rendering", () => {
    const requests = new ImportRequests();
    expect(requests.startRead()).not.toBeNull();
    expect(requests.startRead()).toBeNull();
    expect(requests.startImport()).toBeNull();
    requests.reading = false;
    expect(requests.startImport()).not.toBeNull();
    expect(requests.startImport()).toBeNull();
    expect(requests.startRead()).toBeNull();
  });
});
