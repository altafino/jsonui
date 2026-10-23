import { $, expect } from "@wdio/globals";
import { addFileByMouse, expectOpen, stageFixture } from "./helpers.ts";

describe("A01 add a JSON file and see its form", () => {
  it("adds feature-flags.json through the picker and renders its fields", async () => {
    const path = stageFixture("feature-flags.json");
    await addFileByMouse(path);
    await expectOpen("feature-flags.json");

    await expect($('[role="switch"][aria-label="Debug"]')).toBeDisplayed();
    await expect($('[role="switch"][aria-label="Enabled"]')).toBeDisplayed();
    await expect($('input[aria-label="Application"]')).toHaveValue("example-service");
    await expect($('[data-testid="save-status"]')).toHaveText(expect.stringContaining("No unsaved changes"));
  });
});
