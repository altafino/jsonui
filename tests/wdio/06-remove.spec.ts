import { $, expect } from "@wdio/globals";
import { addFileByMouse, expectOpen, read, stageFixture } from "./helpers.ts";

const REMOVED = ["debug", "retryCount", "displayName"];

describe("remove one field, then several at once, then save", () => {
  it("drops exactly those keys on disk and leaves every other line byte-identical", async () => {
    const path = stageFixture("feature-flags.json");
    const original = read(path);

    await addFileByMouse(path);
    await expectOpen("feature-flags.json");

    // One field through its always-visible button.
    await $('button[aria-label="Remove Debug"]').click();
    await $('button[aria-label="Remove Debug"]').waitForExist({ reverse: true });

    // Two more through selection mode, as one step.
    await $("button=Select").click();
    await $('input[aria-label="Select Retry count"]').click();
    await $('input[aria-label="Select Display name"]').click();
    await expect($('[role="region"][aria-label="Selection"]')).toHaveText(expect.stringContaining("2 selected"));
    await $("button=Remove selected").click();
    await $('[role="region"][aria-label="Selection"]').waitForExist({ reverse: true });
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("Ready · 3 pending changes"));

    await $("button=Review changes").click();
    const dialog = $('[role="dialog"]');
    await dialog.waitForDisplayed();
    await dialog.$("button=Save changes").click();
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringMatching(/Saved \d\d:\d\d · backup created/));

    const saved = read(path);
    const expected = JSON.parse(original) as Record<string, unknown>;
    for (const k of REMOVED) delete expected[k];
    expect(JSON.parse(saved)).toEqual(expected);

    // Untouched lines are byte-identical: the file is the original minus the three key lines.
    const gone = (l: string) => REMOVED.some((k) => l.trimStart().startsWith(`"${k}":`));
    expect(saved).toBe(original.split("\n").filter((l) => !gone(l)).join("\n"));
  });
});
