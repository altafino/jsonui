import { expect, test, type Page } from "@playwright/test";
import { externalChange, launch, noHorizontalScroll, openFile } from "./helpers";

// Visual verification set (spec §17). Test runs write to test-results/;
// UPDATE_SCREENSHOTS=1 (make screenshots) refreshes the committed docs/screenshots/.
const OUT = process.env.UPDATE_SCREENSHOTS ? "docs/screenshots" : "test-results/screenshots";
const WIDTHS = [1024, 1366, 1920] as const;
const HEIGHT: Record<number, number> = { 1024: 768, 1366: 768, 1920: 1080 };
const THEMES = ["dark", "light"] as const;

async function shoot(page: Page, name: string, w: number, theme: string) {
  await page.waitForTimeout(250);
  await page.screenshot({ path: `${OUT}/${name}-${w}-${theme}.png` });
}

for (const theme of THEMES) {
  for (const w of WIDTHS) {
    test.describe(`${theme} ${w}`, () => {
      test.use({ viewport: { width: w, height: HEIGHT[w] }, colorScheme: theme });

      test("workspace with a pending change", async ({ page }) => {
        await launch(page, "seeded");
        await openFile(page, "feature-flags.json").catch(() => page.getByRole("button", { name: /^feature-flags\.json/ }).click());
        await page.getByRole("switch", { name: "Debug" }).click();
        await expect(page.getByTestId("save-status")).toContainText("1 pending change");
        expect(await noHorizontalScroll(page)).toBe(true);
        await shoot(page, "workspace", w, theme);
      });

      test("onboarding, review, nested arrays, conflict, invalid source", async ({ page }) => {
        await launch(page);
        await shoot(page, "onboarding", w, theme);

        await launch(page, "seeded");
        await page.getByRole("button", { name: /^my-app · Shared/ }).click();
        await page.getByRole("button", { name: /^Hooks/ }).first().waitFor();
        await page.getByRole("button", { name: /^Pre Tool Use|^PreToolUse|^Pre tool use/ }).first().click();
        await page.getByText("Bash · enabled · 30").waitFor();
        expect(await noHorizontalScroll(page)).toBe(true);
        await shoot(page, "nested-arrays", w, theme);

        await page.getByRole("button", { name: /^feature-flags\.json/ }).click();
        await page.getByRole("switch", { name: "Debug" }).click();
        await page.getByRole("button", { name: "Review changes" }).click();
        await page.getByRole("dialog").getByTestId("change-row").waitFor();
        await shoot(page, "review", w, theme);
        await page.keyboard.press("Escape");

        await externalChange(page, "feature-flags.json", "non-overlapping");
        await page.getByTestId("conflict-banner").getByRole("button", { name: "Compare" }).click();
        await page.getByRole("dialog").getByRole("list", { name: "Changed on disk" }).waitFor();
        await shoot(page, "conflict", w, theme);
        await page.keyboard.press("Escape");
        await page.getByTestId("conflict-banner").getByRole("button", { name: "Discard my draft & reload" }).click();
        await page.getByTestId("conflict-banner").waitFor({ state: "hidden" });

        await page.getByRole("button", { name: /^broken-secrets\.json/ }).click();
        await page.getByTestId("source-view").waitFor();
        expect(await noHorizontalScroll(page)).toBe(true);
        await shoot(page, "invalid-source", w, theme);
      });
    });
  }
}
