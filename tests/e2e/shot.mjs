// Ad-hoc screenshot helper: node tests/e2e/shot.mjs <url> <out.png> <width> <height> <theme> [click-text ...]
import { chromium } from "@playwright/test";

const [url, out, w = "1366", h = "768", theme = "dark", ...clicks] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: +w, height: +h }, colorScheme: theme });
const logs = [];
page.on("console", (m) => ["error", "warning"].includes(m.type()) && logs.push(m.text()));
page.on("pageerror", (e) => logs.push("PAGEERROR " + e.message));
await page.goto(url);
await page.waitForTimeout(600);
for (const c of clicks) {
  if (c.startsWith("key:")) await page.keyboard.press(c.slice(4));
  else if (c.startsWith("w:")) await page.waitForTimeout(+c.slice(2));
  else await page.getByText(c, { exact: true }).first().click();
  await page.waitForTimeout(400);
}
await page.screenshot({ path: out });
console.log(logs.join("\n"));
await browser.close();
