import { $, expect } from "@wdio/globals";
import { addFileByMouse, byRole, expectOpen, read, stageFixture } from "./helpers.ts";

// YAML and TOML files edit like JSON: toggling a switch and saving changes
// only that token; comments, quoting and layout stay byte-identical.
async function toggleDebugAndSave(name: string, before: string, after: string) {
  const path = stageFixture(name);
  const original = read(path);
  expect(original).toContain(before);

  await addFileByMouse(path);
  await expectOpen(name);
  await expect(await byRole("textbox", "Name")).toHaveValue("api");

  const debug = await byRole("switch", "Debug");
  await debug.click();
  await expect(debug).toHaveAttribute("aria-checked", "true");

  await $("button=Review changes").click();
  const dialog = $('[role="dialog"]');
  await dialog.waitForDisplayed();
  await dialog.$("button=Save changes").click();
  await expect($('[data-testid="save-status"]')).toHaveText(expect.stringMatching(/Saved \d\d:\d\d · backup created/));

  expect(read(path)).toBe(original.replace(before, after));
}

describe("YAML and TOML files", () => {
  it("saves a YAML toggle without touching comments or quotes", async () => {
    await toggleDebugAndSave("service.yaml", "debug: false  # verbose logging", "debug: true  # verbose logging");
  });

  it("saves a TOML toggle without touching comments or tables", async () => {
    await toggleDebugAndSave("service.toml", "debug = false  # verbose logging", "debug = true  # verbose logging");
  });
});
