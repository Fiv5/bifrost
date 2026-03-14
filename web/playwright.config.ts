import { defineConfig } from "@playwright/test";
import { fileURLToPath } from "node:url";

const webPort = Number(process.env.WEB_PORT ?? 3000);
const webRoot = fileURLToPath(new URL(".", import.meta.url));
const backendPort = Number(process.env.BIFROST_UI_TEST_PORT ?? process.env.BACKEND_PORT ?? 9910);

export default defineConfig({
  testDir: "./tests/ui",
  timeout: 120000,
  expect: {
    timeout: 15000,
  },
  use: {
    baseURL: `http://127.0.0.1:${webPort}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command: `BACKEND_PORT=${backendPort} pnpm dev -- --host 127.0.0.1 --port ${webPort} --backend-port ${backendPort}`,
    url: `http://127.0.0.1:${webPort}/_bifrost/`,
    reuseExistingServer: true,
    cwd: webRoot,
    timeout: 120000,
  },
  globalSetup: "./tests/ui/global-setup.ts",
  globalTeardown: "./tests/ui/global-teardown.ts",
});
