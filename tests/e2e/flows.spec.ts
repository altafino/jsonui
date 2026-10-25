import { expect, test } from "@playwright/test";
import { externalChange, launch, noHorizontalScroll, openFile, saveStatus } from "./helpers";

test("first launch: sample, toggle, review, save, undo save", async ({ page }) => {
  await launch(page);
  await expect(page.getByText("Edit JSON, YAML and TOML settings as forms. Every save makes a backup first.")).toBeVisible();
  await page.getByRole("button", { name: /Try with a sample/ }).click();

  const header = page.getByRole("group", { name: "File header" });
  await expect(header.getByRole("heading", { name: "feature-flags.json" })).toBeVisible();
  await expect(header.getByText("Sample", { exact: true })).toBeVisible();
  await expect(page.getByText("Safe to edit. This is a copy in ConfigDeck's data folder.")).toBeVisible();

  const debug = page.getByRole("switch", { name: "Debug" });
  await expect(debug).not.toBeChecked();
  await debug.click();
  await expect(debug).toBeChecked();
  await expect(saveStatus(page)).toContainText("Ready · 1 pending change");

  await page.getByRole("button", { name: "Review changes" }).click();
  const dialog = page.getByRole("dialog", { name: "Review changes" });
  await expect(dialog.getByTestId("change-row")).toContainText("Debug");
  await dialog.getByRole("tab", { name: "Text diff" }).click();
  await expect(dialog.getByTestId("text-diff")).toContainText('"debug": true');
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(saveStatus(page)).toContainText(/Saved \d\d:\d\d · backup created/);
  await page.getByRole("button", { name: "Undo save" }).click();
  await expect(saveStatus(page)).toContainText("Restored");
  await expect(page.getByRole("switch", { name: "Debug" })).not.toBeChecked();
});

test("invalid file shows the source view with a masked secret", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "broken-secrets.json");
  const view = page.getByTestId("source-view");
  await expect(view.getByRole("alert")).toContainText("Expected a comma");
  await expect(view).not.toContainText("sk-test-0000000000000000");
  await expect(view).toContainText("••••");
  await expect(page.getByRole("button", { name: "Save changes" })).toHaveCount(0);
  await expect(saveStatus(page)).toContainText("No unsaved changes");
  await view.getByRole("button", { name: /Reveal masked value/ }).click();
  await expect(view).toContainText("sk-test-0000000000000000");
});

test("outside change with a draft: conflict, then reapply without overlap", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json");
  await page.getByRole("switch", { name: "Debug" }).click();
  await externalChange(page, "feature-flags.json", "non-overlapping");

  const banner = page.getByTestId("conflict-banner");
  await expect(banner).toContainText("This file changed outside ConfigDeck");
  await expect(page.getByRole("button", { name: "Save changes" })).toBeDisabled();
  await banner.getByRole("button", { name: "Compare" }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("list", { name: "Changed on disk" })).toBeVisible();
  await expect(dialog.getByRole("list", { name: "Your unsaved changes" })).toContainText("Debug");
  await dialog.getByRole("button", { name: "Reapply my changes to the new version" }).click();
  await expect(banner).toBeHidden();
  await expect(saveStatus(page)).toContainText("Ready · 1 pending change");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(saveStatus(page)).toContainText(/Saved/);
});

test("overlapping outside change blocks reapply", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json");
  await page.getByRole("switch", { name: "Debug" }).click();
  await externalChange(page, "feature-flags.json", "overlapping");
  await page.getByTestId("conflict-banner").getByRole("button", { name: "Compare" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("both changed").first()).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Reapply my changes to the new version" })).toBeDisabled();
});

test("keyboard-only edit and save (A27)", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json");
  await page.keyboard.press("Control+f");
  await expect(page.getByRole("searchbox", { name: "Filter this form" })).toBeFocused();
  await page.keyboard.type("debug");

  // Tab to the switch; focus must be visibly ringed.
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press("Tab");
    if (await page.evaluate(() => document.activeElement?.getAttribute("role") === "switch")) break;
  }
  const sw = page.getByRole("switch", { name: "Debug" });
  await expect(sw).toBeFocused();
  expect(await sw.evaluate((el) => getComputedStyle(el).outlineStyle)).not.toBe("none");
  await page.keyboard.press("Space");
  await expect(sw).toBeChecked();

  // Document undo/redo work while focus is on a non-text control.
  await page.keyboard.press("Control+z");
  await expect(saveStatus(page)).toContainText("No unsaved changes");
  await page.keyboard.press("Control+Shift+z");
  await expect(saveStatus(page)).toContainText("Ready · 1 pending change");

  await page.keyboard.press("Control+s");
  await expect(saveStatus(page)).toContainText(/Saved \d\d:\d\d/);

  // F6 cycles landmarks.
  await page.keyboard.press("F6");
  const landmark = await page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset.landmark);
  expect(landmark).toBeTruthy();
});

test("nested array of objects: summary rows, move with the keyboard", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "my-app · Shared");
  await page.getByRole("button", { name: /^Hooks/ }).first().waitFor();
  await page.getByRole("button", { name: /^Pre Tool Use|^PreToolUse|^Pre tool use/ }).first().click();
  await expect(page.getByText("Bash · enabled · 30")).toBeVisible();
  const handle = page.getByRole("button", { name: /Reorder item 1 of 2/ });
  await handle.focus();
  await page.keyboard.press("Alt+ArrowDown");
  await expect(page.getByRole("button", { name: /Reorder item 2 of 2/ })).toBeFocused();
});

test("Shift+F10 opens the row context menu; Copy JSON path is offered", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json");
  await page.getByRole("textbox", { name: "Application" }).focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem", { name: "Copy JSON path" })).toBeVisible();
  await page.keyboard.press("Escape");
});

test("1024 px: rail sidebar, no horizontal scroll, Ctrl+B opens the overlay", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json").catch(async () => {
    // In rail mode names are tooltips; open via accessible name instead.
    await page.getByRole("button", { name: /^feature-flags\.json/ }).click();
  });
  await expect(page.getByTestId("sidebar")).toHaveAttribute("data-mode", "rail");
  expect(await noHorizontalScroll(page)).toBe(true);
  await page.keyboard.press("Control+b");
  await expect(page.getByTestId("sidebar-overlay")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("sidebar-overlay")).toBeHidden();
});

test("multi-select remove, one undo, protected setting has no checkbox", async ({ page }) => {
  await launch(page, "seeded");
  await openFile(page, "feature-flags.json");
  await expect(page.getByText("Required", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Remove Application" })).toHaveCount(0);

  await page.getByRole("button", { name: "Select", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "Select Application" })).toHaveCount(0);
  await page.getByRole("checkbox", { name: "Select Debug" }).check();
  await page.getByRole("checkbox", { name: "Select Retry count" }).check();
  await page.getByRole("checkbox", { name: "Select item 1" }).first().check();
  await page.screenshot({ path: "test-results/selection-mode.png" });
  await expect(page.getByText("3 selected")).toBeVisible();
  await page.getByRole("button", { name: "Remove selected" }).click();

  await expect(page.getByRole("switch", { name: "Debug" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Remove Retry count" })).toHaveCount(0);
  await expect(page.getByText("Removed 3 items")).toBeAttached();
  await expect(saveStatus(page)).toContainText(/pending change/);

  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.getByRole("switch", { name: "Debug" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Remove Retry count" })).toBeVisible();
  await expect(saveStatus(page)).not.toContainText("pending change");
});
