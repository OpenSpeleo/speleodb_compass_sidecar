import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { controller } from "../lib/controller";
import {
  CreateProjectModal,
  validateCreateProject,
  validCoordinate,
} from "./create-project-modal";

vi.mock("../lib/controller", () => ({
  controller: { createProject: vi.fn().mockResolvedValue(undefined) },
}));
const valid = {
  name: "Cave",
  description: "Survey",
  country: "US",
  latitude: "",
  longitude: "",
};
describe("project creation parity", () => {
  it("validates required fields in their existing order", () => {
    expect(validateCreateProject({ ...valid, name: " " })).toBe(
      "Project name is required",
    );
    expect(validateCreateProject({ ...valid, name: "a".repeat(256) })).toBe(
      "Project name must be less than 255 characters",
    );
    expect(validateCreateProject({ ...valid, description: " " })).toBe(
      "Description is required",
    );
    expect(validateCreateProject({ ...valid, country: "" })).toBe(
      "Please select a country",
    );
    expect(validateCreateProject(valid)).toBeNull();
  });
  it("counts project names as UTF-8 bytes", () => {
    expect(validateCreateProject({ ...valid, name: "🦇".repeat(64) })).toBe(
      "Project name must be less than 255 characters",
    );
    expect(
      validateCreateProject({ ...valid, name: "🦇".repeat(63) }),
    ).toBeNull();
  });
  it("parses full coordinate strings without new range constraints", () => {
    for (const value of [
      "45.1",
      "-93.2",
      "1e-3",
      ".5",
      "+Infinity",
      "NaN",
      "1000",
    ])
      expect(validCoordinate(value)).toBe(true);
    for (const value of [
      "12oops",
      " 12",
      "12 ",
      "12\n",
      "12\r",
      "",
      "1,2",
      ".",
      "0x12",
    ])
      expect(validCoordinate(value)).toBe(false);
    expect(validateCreateProject({ ...valid, latitude: "invalid" })).toBe(
      "Latitude must be a valid number",
    );
    expect(validateCreateProject({ ...valid, longitude: "invalid" })).toBe(
      "Longitude must be a valid number",
    );
  });
  it("sends optional coordinate strings and leaves successful navigation to backend state", async () => {
    const onClose = vi.fn();
    render(<CreateProjectModal onClose={onClose} />);
    fireEvent.change(screen.getByPlaceholderText("My Awesome Cave Project"), {
      target: { value: "Cave" },
    });
    fireEvent.change(screen.getByPlaceholderText("Describe the project..."), {
      target: { value: "Survey" },
    });
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "US" } });
    fireEvent.change(screen.getByPlaceholderText("e.g. 45.1234"), {
      target: { value: "45.1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create Project" }));
    await waitFor(() =>
      expect(controller.createProject).toHaveBeenCalledWith(
        "Cave",
        "Survey",
        "US",
        "45.1",
        null,
      ),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Create Project" }),
      ).toBeEnabled(),
    );
    expect(onClose).not.toHaveBeenCalled();
  });
});
