import { $, expect } from "@wdio/globals";
import { addFileByMouse, byRole, expectOpen, stageFixture } from "./helpers.ts";

describe("A01 add a JSON file and see its form", () => {
  it("adds feature-flags.json through the picker and renders its fields", async () => {
    await addFileByMouse(stageFixture("feature-flags.json"));
    await expectOpen("feature-flags.json");

    await expect(await byRole("switch", "Debug")).toBeDisplayed();
    await expect(await byRole("switch", "Enabled")).toBeDisplayed();
    await expect(await byRole("textbox", "Application")).toHaveValue("example-service");
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("No unsaved changes"));
  });
});
