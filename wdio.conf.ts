import { appBinary, launcher, makeSandbox } from "./tests/wdio/helpers.ts";
import { spawn, type ChildProcess } from "node:child_process";
import { homedir } from "node:os";
import { join } from "node:path";

// Real-app E2E: tauri-driver (official Tauri v2 WebDriver proxy) -> WebKitWebDriver -> the built app.
// One sandbox per run (fixture copies + HOME/XDG dirs); created at load, workers inherit the env.
// Each spec file = one session = one app launch, so specs stage their fixtures in their own subdir.
let driver: ChildProcess;
process.env.CONFIGDECK_SANDBOX ??= makeSandbox().dir;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./tests/wdio/*.spec.ts"],
  maxInstances: 1,
  hostname: "127.0.0.1",
  port: 4444,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { timeout: 60_000 },
  waitforTimeout: 10_000,
  logLevel: "warn",
  // tauri-driver spawns `application args...`; the wrapper sets the isolated env then execs the real app.
  capabilities: [
    {
      browserName: "wry",
      "wdio:enforceWebDriverClassic": true, // tauri-driver rejects the BiDi webSocketUrl capability
      "tauri:options": { application: launcher, args: [process.env.CONFIGDECK_SANDBOX, appBinary()] },
    } as WebdriverIO.Capabilities,
  ],

  onPrepare: () => {
    driver = spawn(process.env.TAURI_DRIVER ?? join(homedir(), ".cargo", "bin", "tauri-driver"), [], {
      stdio: [null, process.stdout, process.stderr],
    });
    return new Promise((r) => setTimeout(r, 1500)); // ponytail: fixed wait, poll :4444 if flaky
  },
  onComplete: () => void driver?.kill(),
};
