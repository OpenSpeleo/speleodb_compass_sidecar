import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  AuthScreen,
  AUTH_METHOD_ERROR,
  validEmailField,
  validOAuth,
} from "./auth-screen";
import { controller } from "../lib/controller";
import { API_BASE_URL } from "../lib/constants";

vi.mock("../lib/controller", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/controller")>()),
  controller: { authenticate: vi.fn().mockResolvedValue(undefined) },
}));
describe("authentication UI parity", () => {
  it("begins with empty fields and silent validation", () => {
    const { container } = render(<AuthScreen />);
    expect(container.querySelector(".invalid")).toBeNull();
    expect(screen.getByLabelText("SpeleoDB instance")).toHaveValue("");
    expect(screen.getByLabelText("SpeleoDB instance")).toHaveAttribute(
      "placeholder",
      API_BASE_URL,
    );
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
  });
  it("retains touched-field conflicts when an input is cleared", () => {
    render(<AuthScreen />);
    fireEvent.change(screen.getByLabelText("Email"), {
      target: { value: "me@example.com" },
    });
    expect(
      screen.getByText("Password is required when using email"),
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "" } });
    fireEvent.change(screen.getByLabelText("OAUTH Token"), {
      target: { value: "a".repeat(40) },
    });
    expect(
      screen.getByText("Cannot use both email/password AND OAuth token"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Cannot use both OAuth token AND email/password"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Reset Form" }));
    expect(
      screen.queryByText("Cannot use both OAuth token AND email/password"),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("OAUTH Token")).toHaveValue("");
  });
  it("shows the exact auth-method error on an empty submission", () => {
    render(<AuthScreen />);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(screen.getByText(AUTH_METHOD_ERROR)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByText("Connection failed")).not.toBeInTheDocument();
  });
  it("uses the normalized instance with unchanged displayed input", async () => {
    render(<AuthScreen />);
    const instance = screen.getByLabelText("SpeleoDB instance");
    fireEvent.input(instance, {
      target: { value: "https://example.com/path///" },
    });
    fireEvent.blur(instance);
    expect(instance).toHaveValue("https://example.com/path///");
    fireEvent.change(screen.getByLabelText("OAUTH Token"), {
      target: { value: "a".repeat(40) },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() =>
      expect(controller.authenticate).toHaveBeenCalledWith(
        null,
        null,
        "a".repeat(40),
        "https://example.com/path",
      ),
    );
  });
  it("shows credentials-specific copy and retains a disabled connecting state", async () => {
    let reject!: (error: Error) => void;
    vi.mocked(controller.authenticate).mockImplementationOnce(
      () =>
        new Promise((_resolve, rejectPromise) => {
          reject = rejectPromise;
        }),
    );
    render(<AuthScreen />);
    fireEvent.change(screen.getByLabelText("OAUTH Token"), {
      target: { value: "a".repeat(40) },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(
      screen.getByRole("button", { name: "Connecting..." }),
    ).toBeDisabled();
    reject(new Error("Unauthorized: 401"));
    await waitFor(() =>
      expect(screen.getByText("Invalid credentials")).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
  });
  it("preserves exact field validation without stricter email rules", () => {
    expect(validEmailField("")).toBe(true);
    expect(validEmailField("@a.co")).toBe(false);
    expect(validEmailField("a@a.co")).toBe(true);
    expect(validEmailField("a@.")).toBe(false);
    expect(validOAuth("a".repeat(40) + "\n")).toBe(false);
  });
});
