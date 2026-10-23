import type { Page } from "@playwright/test";

/** Opens the app against the in-memory mock. Flags: seeded, recovered, leftover, missing, readonly, symlink. */
export async function launch(page: Page, flags = "") {
  await page.goto(flags ? `/?mock=${flags}` : "/");
  await page.waitForSelector("main");
}

export async function openFile(page: Page, name: string) {
  await page.getByRole("button", { name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`) }).first().click();
  await page.getByRole("group", { name: "File header" }).getByRole("heading", { name }).waitFor();
}

/** Simulates another program changing the file on disk (mock only). */
export async function externalChange(page: Page, name: string, mode: "non-overlapping" | "overlapping" | "invalid") {
  await page.evaluate(
    ([n, m]) => {
      const c = (window as unknown as { __configdeckMock: { idOf(n: string): string; simulateExternalChange(id: string, m: string): void } }).__configdeckMock;
      c.simulateExternalChange(c.idOf(n), m);
    },
    [name, mode],
  );
}

export const saveStatus = (page: Page) => page.getByTestId("save-status");

export async function noHorizontalScroll(page: Page) {
  return page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth && document.body.scrollWidth <= window.innerWidth);
}
