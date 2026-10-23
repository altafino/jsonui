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

/** Mouse path: Add file menu -> "Add JSON file…" -> scripted picker. */
export async function addFileByMouse(path: string) {
  queuePick(path);
  await $("button=Add file").click();
  await $('[role="menuitem"]=Add JSON file…').click();
}

/** Waits for the file header to show `name`. */
export async function expectOpen(name: string) {
  const header = $('[role="group"][aria-label="File header"]');
  await browser.waitUntil(async () => (await header.isExisting()) && (await header.getText()).includes(name), {
    timeoutMsg: `file header never showed ${name}`,
  });
}
