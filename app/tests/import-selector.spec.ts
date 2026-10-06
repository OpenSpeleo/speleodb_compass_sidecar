import { expect, test, type Page } from "@playwright/test";

async function expectFooterReachable(page: Page) {
  const geometry = await page.locator("dialog").evaluate((dialog) => {
    const footer = dialog.querySelector("footer")!;
    const button = footer.querySelector(".import-selector__primary")!;
    const bounds = dialog.getBoundingClientRect();
    const actions = footer.getBoundingClientRect();
    const primary = button.getBoundingClientRect();
    return {
      bounds: { top: bounds.top, bottom: bounds.bottom },
      actions: { top: actions.top, bottom: actions.bottom },
      primary: { left: primary.left, right: primary.right },
      viewport: { width: innerWidth, height: innerHeight },
      scrollable: dialog.scrollHeight > dialog.clientHeight,
    };
  });
  expect(geometry.actions.top).toBeGreaterThanOrEqual(
    Math.max(0, geometry.bounds.top) - 1,
  );
  expect(geometry.actions.bottom).toBeLessThanOrEqual(
    Math.min(geometry.viewport.height, geometry.bounds.bottom) + 1,
  );
  expect(geometry.primary.left).toBeGreaterThanOrEqual(0);
  expect(geometry.primary.right).toBeLessThanOrEqual(geometry.viewport.width);
  return geometry;
}

test("native dialog focuses its title and Escape dismisses it", async ({
  page,
}) => {
  await page.goto("/tests/import.html");
  await expect(page.locator("dialog")).toBeVisible();
  await expect(page.locator("#import-selector-title")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByText("Dialog closed")).toBeVisible();
  await expect(page.locator("dialog")).toHaveCount(0);
});

test("busy native dialog blocks Escape and disables confirmation", async ({
  page,
}) => {
  await page.goto("/tests/import.html?busy=1");
  await expect(page.locator("#import-selector-title")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.locator("dialog")).toBeVisible();
  await expect(page.getByRole("button", { name: "Importing…" })).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Cancel", exact: true }),
  ).toBeDisabled();
});

test("selection includes dependencies and filtering leaves global bulk actions intact", async ({
  page,
}) => {
  await page.goto("/tests/import.html");
  await expect(page.getByRole("checkbox")).toHaveCount(20);
  await page.getByRole("searchbox").fill("Passage 019");
  await expect(page.getByRole("checkbox")).toHaveCount(1);
  await page
    .getByRole("button", { name: "Clear selection", exact: true })
    .click();
  await expect(page.getByText("0 of 20 sections")).toBeVisible();
  await page.getByRole("checkbox").check();
  await expect(page.getByText("19 included automatically")).toBeVisible();
  await page
    .getByRole("button", { name: "Import 20 sections", exact: true })
    .click();
  await expect(page.getByTestId("confirmed")).toHaveText("[19]");
  await page.getByRole("button", { name: "Select all", exact: true }).click();
  await expect(page.getByText("20 of 20 sections")).toBeVisible();
  await page.getByRole("searchbox").fill("");
  await expect(page.locator("#import-section-0")).toBeChecked();
  await expect(page.locator("#import-section-0")).toBeDisabled();
});

test("500 connected sections render explanations only when expanded", async ({
  page,
}) => {
  await page.goto("/tests/import.html?dense=500");
  await expect(page.getByRole("checkbox")).toHaveCount(500);
  await expect(page.locator("li")).toHaveCount(0);
  await page.locator("summary").first().click();
  await expect(page.locator("li").first()).toBeVisible();
  await expect(
    page.getByText("Passage 001.DAT: Connects at station A1"),
  ).toBeVisible();
  await page.locator("summary").first().click();
  await expect(page.locator("li")).toHaveCount(0);
});

for (const viewport of [
  { width: 800, height: 900 },
  { width: 560, height: 620 },
]) {
  test(`long warnings and errors keep actions visible at ${viewport.width}×${viewport.height}`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await page.goto("/tests/import.html?warning=long&error=long");
    await expect(
      page.getByRole("button", {
        name: "Import complete project",
        exact: true,
      }),
    ).toBeVisible();
    const geometry = await expectFooterReachable(page);
    expect(geometry.scrollable).toBe(true);
    await page.locator("dialog").evaluate((dialog) => {
      dialog.scrollTop = dialog.scrollHeight;
    });
    await expectFooterReachable(page);
    await expect(
      page.getByRole("button", { name: "Clear selection", exact: true }),
    ).toHaveCount(0);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByText("Dialog closed")).toBeVisible();
  });
}
