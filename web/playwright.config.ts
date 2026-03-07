import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/ui",
  timeout: 60000,
  expect: {
    timeout: 15000,
  },
  use: {
    baseURL: "http://127.0.0.1:3000",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: {
    command: "pnpm dev -- --host 127.0.0.1 --port 3000",
    url: "http://127.0.0.1:3000/_bifrost/",
    reuseExistingServer: true,
    cwd: "./",
    timeout: 120000,
  },
  globalSetup: "./tests/ui/global-setup.ts",
  globalTeardown: "./tests/ui/global-teardown.ts",
});
