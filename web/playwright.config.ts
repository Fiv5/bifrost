import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const webPort = Number(process.env.WEB_PORT ?? 3000);
const webRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  testDir: "./tests/ui",
  timeout: 60000,
  expect: {
    timeout: 15000,
  },
  use: {
    baseURL: `http://localhost:${webPort}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command: `pnpm dev -- --host 127.0.0.1 --port ${webPort}`,
    url: `http://localhost:${webPort}/_bifrost/`,
    reuseExistingServer: true,
    cwd: webRoot,
    timeout: 120000,
  },
  globalSetup: "./tests/ui/global-setup.ts",
  globalTeardown: "./tests/ui/global-teardown.ts",
});
