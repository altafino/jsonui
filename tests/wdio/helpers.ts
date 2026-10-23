import { browser, $ } from "@wdio/globals";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");

export interface Sandbox {
  dir: string;
  /** Directory the tests copy fixtures into and the app is asked to open. */
  files: string;
}

/** Fresh per-session temp dir: fixture copies plus HOME/XDG dirs for the app's own state. */
export function makeSandbox(): Sandbox {
  const dir = mkdtempSync(join(tmpdir(), "configdeck-wdio-"));
  for (const d of ["home", "xdg-config", "xdg-data", "xdg-cache", "files"]) mkdirSync(join(dir, d));
  writeFileSync(join(dir, "picks.txt"), "");
  return { dir, files: join(dir, "files") };
}

export function appBinary(): string {
  const env = process.env.CONFIGDECK_BIN;
  if (env) return resolve(env);
  for (const p of ["release", "debug"]) {
    const bin = join(root, "src-tauri/target", p, "configdeck");
    if (existsSync(bin)) return bin;
  }
  throw new Error("ConfigDeck binary not found: run `pnpm build:app` first (or set CONFIGDECK_BIN)");
}

export const launcher = join(root, "tests/wdio/launch-app.sh");

export const sandbox = (): Sandbox => {
  const dir = process.env.CONFIGDECK_SANDBOX!;
  return { dir, files: join(dir, "files") };
};

/** Copies fixtures/<name> into the sandbox and returns the copy's path. */
export function stageFixture(name: string): string {
  const dst = join(mkdtempSync(join(sandbox().files, "s-")), name);
  copyFileSync(join(root, "fixtures", name), dst);
  return dst;
}

/** The next native picker call returns `path` (empty string = cancel). */
export function queuePick(path: string) {
  writeFileSync(join(sandbox().dir, "picks.txt"), path + "\n");
}

export const read = (p: string) => readFileSync(p, "utf8");

export const saveStatus = () => $('[data-testid="save-status"]');

/** Mouse path from the empty-state welcome screen: "Add JSON file" -> scripted picker. */
export async function addFileByMouse(path: string) {
  queuePick(path);
  // Never create two element queries at once: WebKitWebDriver can hang on concurrent commands.
  const welcomeShown = async () => await $("button=Add JSON file").isExisting();
  await browser.waitUntil(async () => (await welcomeShown()) || (await $("button=Add file").isExisting()), { timeoutMsg: "app UI never rendered" });
  if (await welcomeShown()) {
    const welcome = $("button=Add JSON file");
    await welcome.waitForClickable();
    await welcome.click();
  } else {
    // Files already registered: the sidebar "Add file" dropdown.
    await $("button=Add file").click();
    await $('[role="menuitem"]=Add JSON file…').click();
  }
}

/** Element with ARIA `role` whose accessible name (aria-label or aria-labelledby text) equals `name`. */
export async function byRole(role: string, name: string) {
  const find = (r: string, n: string) =>
    [...document.querySelectorAll<HTMLElement>(r === "textbox" ? `[role="textbox"], input:not([type]), input[type="text"], textarea` : `[role="${r}"]`)].find((e) => {
      const ids = e.getAttribute("aria-labelledby");
      const label = ids ? ids.split(" ").map((i) => document.getElementById(i)?.textContent ?? "").join(" ") : (e.getAttribute("aria-label") ?? "");
      return label.trim() === n;
    }) ?? null;
  const el = await browser.execute(find, role, name);
  if (!el) throw new Error(`no ${role} named ${name}`);
  return $(el);
}

/** Waits for the file header to show `name`. */
export async function expectOpen(name: string) {
  const header = $('[role="group"][aria-label="File header"]');
  await browser.waitUntil(async () => (await header.isExisting()) && (await header.getText()).includes(name), {
    timeoutMsg: `file header never showed ${name}`,
  });
}
