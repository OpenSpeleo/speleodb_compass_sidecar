import { test, expect } from "@playwright/test";
import { installIpcFixture, projectFixture, stateFixture } from "./ipc-fixture";

test("startup registers state listener before initialization, then navigates and returns", async ({
  page,
}) => {
  await installIpcFixture(page, stateFixture());
  await page.goto("/");
  await expect(page.getByText("Crystal Cave", { exact: true })).toBeVisible();
  await page.getByText("Crystal Cave", { exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Open in Compass" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "← Back to Projects" }).click();
  await expect(
    page.getByRole("button", { name: "Open in Compass" }),
  ).toHaveCount(0);
  const calls = await page.evaluate(() =>
    (
      window as unknown as {
        __SIDECAR_FIXTURE__: { calls: { command: string }[] };
      }
    ).__SIDECAR_FIXTURE__.calls.map((call) => call.command),
  );
  expect(calls.indexOf("plugin:event|listen")).toBeLessThan(
    calls.indexOf("ensure_initialized"),
  );
  expect(
    calls.filter((command) => command === "ensure_initialized"),
  ).toHaveLength(1);
});
test("authentication preserves conflicting cleared-field validation and reset", async ({
  page,
}) => {
  await installIpcFixture(
    page,
    stateFixture({ loading_state: "Unauthenticated" }),
  );
  await page.goto("/");
  await page.getByLabel("Email", { exact: true }).fill("u@example.com");
  await expect(
    page.getByText("Password is required when using email"),
  ).toBeVisible();
  await page.getByLabel("Email", { exact: true }).fill("");
  await page.getByLabel("OAUTH Token").fill("0".repeat(40));
  await expect(
    page.getByText("Cannot use both OAuth token AND email/password"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Reset Form" }).click();
  await expect(page.locator(".field-error")).toHaveCount(0);
});
test("dirty project saves a supplied message and exposes unchanged completion copy", async ({
  page,
}) => {
  const project = projectFixture("Dirty");
  await installIpcFixture(
    page,
    stateFixture({
      project_status: [project],
      selected_project_id: project.info.id,
    }),
  );
  await page.goto("/");
  await page
    .getByPlaceholder("Describe your changes (max 255 characters)")
    .fill("New survey");
  await page.getByRole("button", { name: "Save Project", exact: true }).click();
  await expect(page.getByText("Project Saved Successfully")).toBeVisible();
});
test("About remains a separately served native-window document", async ({
  page,
}) => {
  await installIpcFixture(page, stateFixture());
  await page.goto("/about.html");
  await expect(page.locator("#version")).toHaveText("v26.9.23");
  await expect(
    page.getByRole("link", { name: "React", exact: true }),
  ).toHaveAttribute("href", "https://react.dev");
});
