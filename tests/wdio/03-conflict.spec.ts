import { $, browser, expect } from "@wdio/globals";
import { writeFileSync } from "node:fs";
import { addFileByMouse, byRole, expectOpen, read, stageFixture } from "./helpers.ts";

const status = () => $('[data-testid="save-status"]');

/** Edit Debug in the app, then change `path` on disk behind its back, and open the Conflict dialog. */
async function conflictWith(name: string, diskEdit: (original: string) => string) {
  const path = stageFixture(name);
  const original = read(path);
  await addFileByMouse(path);
  await expectOpen(name);
  await (await byRole("switch", "Debug")).click();
  await expect(status()).toHaveText(expect.stringContaining("Ready · 1 pending change"));

  writeFileSync(path, diskEdit(original));

  // The watcher flags the file; saving stays paused until the conflict is resolved.
  const banner = $('[data-testid="conflict-banner"]');
  await banner.waitForDisplayed({ timeoutMsg: "conflict banner never appeared after the outside change" });
  await banner.$("button=Compare").click();
  const dialog = $('[role="dialog"]');
  await dialog.waitForDisplayed();
  await expect(dialog).toHaveText(expect.stringContaining("This file changed outside ConfigDeck"));
  return { path, original, dialog, banner };
}

describe("A14 conflict on save", () => {
  it("non-overlapping disk change: Reapply is enabled; both changes survive the save", async () => {
    const { path, original, dialog, banner } = await conflictWith("feature-flags.json", (o) =>
      o.replace('"features": ["search", "analytics"]', '"features": ["search", "analytics", "extra"]'),
    );
    const reapply = dialog.$("button=Reapply my changes to the new version");
    await expect(reapply).toBeEnabled();
    await reapply.click();
    await banner.waitForDisplayed({ reverse: true });
    await expect(status()).toHaveText(expect.stringContaining("Ready · 1 pending change"));

    await $("button=Save changes").click();
    await expect(status()).toHaveText(expect.stringMatching(/Saved \d\d:\d\d/));
    const saved = read(path);
    expect(saved).toContain('"debug": true');
    expect(saved).toContain('"analytics", "extra"');
    expect(saved).toBe(original.replace('"debug": false', '"debug": true').replace('"analytics"]', '"analytics", "extra"]'));
  });

  it("overlapping disk change (same key): Reapply is disabled", async () => {
    const { path, dialog } = await conflictWith("feature-flags.json", (o) => o.replace('"debug": false', '"debug": null'));
    await expect(dialog.$("button=Reapply my changes to the new version")).toBeDisabled();
    await expect(dialog).toHaveText(expect.stringContaining("both changed"));
    // Nothing was overwritten.
    await browser.pause(300);
    expect(read(path)).toContain('"debug": null');
  });
});
