import { $, browser, expect } from "@wdio/globals";
import { expectOpen, queuePick, read, stageFixture } from "./helpers.ts";

/** Presses Tab until the focused element satisfies `test` (evaluated in the page). Keyboard only. */
async function tabTo(test: (el: HTMLElement) => boolean, max = 40) {
  for (let i = 0; i < max; i++) {
    await browser.keys("Tab");
    if (await browser.execute(`return (${test.toString()})(document.activeElement)`)) return;
  }
  throw new Error("focus never reached the target with Tab");
}

describe("A27 keyboard-only: add -> edit -> save (no mouse)", () => {
  it("completes the journey with Tab, Enter, Space and Ctrl+S only", async () => {
    const path = stageFixture("feature-flags.json");
    const original = read(path);
    queuePick(path);

    // Welcome screen: focus "Add JSON file" and activate it.
    await $("button=Add JSON file").waitForDisplayed();
    await tabTo((el) => el.textContent?.trim() === "Add JSON file");
    await browser.keys("Enter");
    await expectOpen("feature-flags.json");

    // Reach the Debug switch by Tab, toggle with Space.
    const isDebug = (el: HTMLElement) => {
      if (el.getAttribute("role") !== "switch") return false;
      const id = el.getAttribute("aria-labelledby");
      return (id ? document.getElementById(id)?.textContent : el.getAttribute("aria-label"))?.trim() === "Debug";
    };
    await tabTo(isDebug, 60);
    await browser.keys("Space");
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("Ready · 1 pending change"));

    await browser.keys(["Control", "s"]);
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringMatching(/Saved \d\d:\d\d/));
    expect(read(path)).toBe(original.replace('"debug": false', '"debug": true'));
  });
});
