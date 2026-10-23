import { defineConfig } from "@playwright/test";

// Browser-level UI flows against the Vite dev server with mocked IPC (eng R11).
// Real-app journeys run with WebdriverIO (tests/wdio).
export default defineConfig({
  testDir: "tests/e2e",
  use: { baseURL: "http://localhost:1420" },
  webServer: { command: "pnpm dev", port: 1420, reuseExistingServer: true },
});
