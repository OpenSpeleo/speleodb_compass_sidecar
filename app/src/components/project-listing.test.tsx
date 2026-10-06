import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  ProjectListing,
  compareModifiedDateDesc,
  compareProjectName,
  sortProjects,
} from "./project-listing";
import { ProjectListingItem } from "./project-listing-item";
import { projectFixture, stateFixture } from "./test-fixtures";
import { controller } from "../lib/controller";

vi.mock("../lib/controller", () => ({
  controller: { setActiveProject: vi.fn(), createProject: vi.fn() },
}));
const project = (name: string, date: string) => {
  const value = projectFixture();
  value.info = { ...value.info, name, modified_date: date, id: name };
  return value;
};

describe("project list parity", () => {
  it("compares names case-insensitively ascending", () => {
    expect(compareProjectName("alpha", "Beta")).toBeLessThan(0);
    expect(compareProjectName("BETA", "alpha")).toBeGreaterThan(0);
    expect(compareProjectName("Alpha", "alpha")).toBe(0);
    expect(compareProjectName("\u{10000}", "\ue000")).toBeGreaterThan(0);
  });
  it("compares modified dates descending", () =>
    expect(
      compareModifiedDateDesc("2026-04-27T10:00:00Z", "2026-04-20T10:00:00Z"),
    ).toBeLessThan(0));
  it("sorts full projects by name", () =>
    expect(
      sortProjects("Name", [
        project("Charlie", "1"),
        project("alpha", "3"),
        project("Bravo", "2"),
      ]).map((p) => p.info.name),
    ).toEqual(["alpha", "Bravo", "Charlie"]));
  it("sorts full projects most recent first", () =>
    expect(
      sortProjects("Modified", [
        project("Old", "2026-04-01"),
        project("Newest", "2026-04-27"),
        project("Middle", "2026-04-15"),
      ]).map((p) => p.info.name),
    ).toEqual(["Newest", "Middle", "Old"]));
  it("retains incoming order for case-equal names", () =>
    expect(
      sortProjects("Name", [
        project("alpha", "1"),
        project("Alpha", "2"),
        project("ALPHA", "3"),
      ]).map((p) => p.info.modified_date),
    ).toEqual(["1", "2", "3"]));
  it("switches sort modes and opens the create form", () => {
    const { container } = render(
      <ProjectListing
        uiState={stateFixture([
          project("Zed", "2026-04-27"),
          project("Alpha", "2026-04-01"),
        ])}
      />,
    );
    expect(
      [...container.querySelectorAll(".project-card h3")].map(
        (node) => node.textContent,
      ),
    ).toEqual(["Alpha", "Zed"]);
    fireEvent.click(screen.getByRole("button", { name: "Most Recent" }));
    expect(
      [...container.querySelectorAll(".project-card h3")].map(
        (node) => node.textContent,
      ),
    ).toEqual(["Zed", "Alpha"]);
    fireEvent.click(screen.getByRole("button", { name: "Create New Project" }));
    expect(
      screen.getByPlaceholderText("My Awesome Cave Project"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(
      screen.queryByPlaceholderText("My Awesome Cave Project"),
    ).not.toBeInTheDocument();
  });
  it("truncates Unicode scalars and preserves navigation error text", async () => {
    const value = project("🦇".repeat(26), "2026-04-01");
    vi.mocked(controller.setActiveProject).mockRejectedValueOnce(
      new Error("offline"),
    );
    render(<ProjectListingItem project={value} userEmail="me@example.com" />);
    fireEvent.click(screen.getByText("🦇".repeat(25)));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Could not open this project: offline",
      ),
    );
    expect(controller.setActiveProject).toHaveBeenCalledWith(value.info.id);
  });
});
