import { $, browser, expect } from "@wdio/globals";
import { addFileByMouse, byRole, expectOpen, read, stageFixture } from "./helpers.ts";

describe("toggle -> review -> save -> backup -> restore", () => {
  it("changes only the toggled token on disk, then restores the original bytes", async () => {
    const path = stageFixture("feature-flags.json");
    const original = read(path);
    expect(original).toContain('"debug": false');

    await addFileByMouse(path);
    await expectOpen("feature-flags.json");

    const debug = await byRole("switch", "Debug");
    await debug.click();
    await expect(debug).toHaveAttribute("aria-checked", "true");
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("Ready · 1 pending change"));

    await $("button=Review changes").click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await expect(dialog).toHaveText(expect.stringContaining("Review changes"));
    await dialog.$("button=Save changes").click();

    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringMatching(/Saved \d\d:\d\d · backup created/));
    // Only the one token changed; formatting and every other byte are untouched.
    expect(read(path)).toBe(original.replace('"debug": false', '"debug": true'));

    await $('button[aria-label="History"]').click();
    const history = $('[role="dialog"]');
    await history.waitForDisplayed();
    await history.$('ul[aria-label="Backups"] button').click();
    const restore = history.$("button=Restore this backup");
    await restore.waitForEnabled();
    await restore.click();

    await browser.waitUntil(async () => read(path) === original, { timeoutMsg: "original bytes were not restored on disk" });
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("Restored"));
    await expect(await byRole("switch", "Debug")).toHaveAttribute("aria-checked", "false");
  });
});
