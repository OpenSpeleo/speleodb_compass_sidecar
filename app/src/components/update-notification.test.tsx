import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { UpdateNotificationPhase } from "../lib/types";
import { controller } from "../lib/controller";
import {
  UpdateNotificationToast,
  updateDismissalKey,
  updateNotificationIsError,
  updateNotificationIsWorking,
  updateNotificationMessage,
} from "./update-notification";

vi.mock("../lib/controller", () => ({
  controller: {
    dismissUpdateNotification: vi.fn().mockResolvedValue(undefined),
    checkForUpdatesNow: vi.fn().mockResolvedValue(undefined),
    openLatestRelease: vi.fn().mockResolvedValue(undefined),
  },
}));
const phases: UpdateNotificationPhase[] = [
  "Checking",
  { Downloading: { version: "1.2.3", progress_percent: null } },
  { Installing: { version: "1.2.3" } },
  { Relaunching: { version: "1.2.3" } },
  { UpToDate: { app_name: "Sidecar" } },
  { Failed: { message: "offline" } },
];
describe("update notification parity", () => {
  it("matches the checking message", () =>
    expect(updateNotificationMessage("Checking")).toBe(
      "Checking for updates...",
    ));
  it("matches downloading without progress", () =>
    expect(updateNotificationMessage(phases[1]!)).toBe(
      "Downloading update 1.2.3...",
    ));
  it("matches downloading with progress", () =>
    expect(
      updateNotificationMessage({
        Downloading: { version: "1.2.3", progress_percent: 42 },
      }),
    ).toBe("Downloading update 1.2.3 (42%)"));
  it("matches installing", () =>
    expect(updateNotificationMessage(phases[2]!)).toBe(
      "Installing update 1.2.3...",
    ));
  it("matches relaunching", () =>
    expect(updateNotificationMessage(phases[3]!)).toBe(
      "Update installed. Relaunching...",
    ));
  it("matches up-to-date", () =>
    expect(updateNotificationMessage(phases[4]!)).toBe(
      "Sidecar is up to date.",
    ));
  it("matches failure", () =>
    expect(updateNotificationMessage(phases[5]!)).toBe(
      "Update failed: offline",
    ));
  it("marks only failure as an error", () =>
    expect(phases.map(updateNotificationIsError)).toEqual([
      false,
      false,
      false,
      false,
      false,
      true,
    ]));
  it("marks only in-flight phases as working", () =>
    expect(phases.map(updateNotificationIsWorking)).toEqual([
      true,
      true,
      true,
      true,
      false,
      false,
    ]));
  it("renders nothing when absent", () => {
    const { container } = render(
      <UpdateNotificationToast notification={null} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
  it("renders retry and download only for failure", () => {
    render(
      <UpdateNotificationToast notification={{ id: 7, phase: phases[5]! }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    fireEvent.click(screen.getByRole("button", { name: "Download Latest" }));
    expect(controller.checkForUpdatesNow).toHaveBeenCalledOnce();
    expect(controller.openLatestRelease).toHaveBeenCalledOnce();
  });
  it("omits action buttons during checking", () => {
    render(
      <UpdateNotificationToast notification={{ id: 7, phase: "Checking" }} />,
    );
    expect(
      screen.queryByRole("button", { name: "Retry" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Download Latest" }),
    ).not.toBeInTheDocument();
  });
  it("gives dismissal an accessible name and sends its phase key", () => {
    render(
      <UpdateNotificationToast notification={{ id: 7, phase: phases[1]! }} />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Dismiss update notification" }),
    );
    expect(controller.dismissUpdateNotification).toHaveBeenCalledWith(
      "7:downloading",
    );
  });
  it("scopes the polite non-atomic live region to its label and message", () => {
    render(
      <UpdateNotificationToast notification={{ id: 7, phase: phases[5]! }} />,
    );
    const live = screen.getByRole("status");
    expect(live).toHaveAttribute("aria-live", "polite");
    expect(live).toHaveAttribute("aria-atomic", "false");
    expect(within(live).getByText("Updates")).toBeInTheDocument();
    expect(within(live).queryByRole("button")).not.toBeInTheDocument();
  });
  it("keeps dismissal keys stable across progress and distinct between phases", () => {
    expect(
      updateDismissalKey({
        id: 7,
        phase: { Downloading: { version: "1", progress_percent: 1 } },
      }),
    ).toBe(
      updateDismissalKey({
        id: 7,
        phase: { Downloading: { version: "1", progress_percent: 99 } },
      }),
    );
    expect(updateDismissalKey({ id: 7, phase: phases[2]! })).toBe(
      "7:installing",
    );
    expect(updateDismissalKey({ id: 7, phase: phases[4]! })).toBe(
      "7:up-to-date",
    );
  });
});
