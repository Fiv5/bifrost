import { test, expect } from "@playwright/test";
import {
  apiBase,
  openPage,
  resetAccessControl,
  setSelectValue,
  waitForToast,
} from "./helpers/admin-helpers";

test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ request }) => {
  await resetAccessControl(request);
});

test("Settings 访问控制支持模式切换、白名单、临时白名单和 LAN 开关", async ({
  page,
}) => {
  await openPage(page, "settings");
  await page.getByRole("tab", { name: /Access Control/ }).click();
  await expect(page.getByRole("tab", { name: /Access Control/, exact: false })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.locator("body")).toContainText("Access Settings");

  await setSelectValue(page, page.getByTestId("settings-access-mode-select"), "Whitelist");
  await expect(page.locator("body")).toContainText("Only allow whitelisted IPs/CIDRs");

  await page.getByTestId("settings-access-allow-lan").click();

  await page.getByTestId("settings-whitelist-input").fill("10.0.0.1");
  await page.getByTestId("settings-whitelist-add-button").click();
  await waitForToast(page, "Added 10.0.0.1 to whitelist");
  await expect(page.getByTestId("settings-whitelist-table")).toContainText("10.0.0.1");

  await page.getByTestId("settings-temp-whitelist-input").fill("10.0.0.2");
  await page.getByTestId("settings-temp-whitelist-add-button").click();
  await waitForToast(page, "Added 10.0.0.2 to temporary whitelist");
  await expect(page.getByTestId("settings-temp-whitelist-table")).toContainText("10.0.0.2");
});

test("Settings 性能配置在第二个页面主动刷新后可见", async ({
  page,
  context,
  request,
}) => {
  const perfRes = await request.get(`${apiBase}/config/performance`);
  const perf = (await perfRes.json()) as { traffic: { max_records: number } };
  const original = perf.traffic.max_records;

  try {
    await openPage(page, "settings");
    await page.getByRole("tab", { name: /Performance/ }).click();
    await expect(page.locator("body")).toContainText("Max Records");

    const page2 = await context.newPage();
    await openPage(page2, "settings");
    await page2.getByRole("tab", { name: /Performance/ }).click();
    await expect(page2.locator("body")).toContainText("Max Records");

    const handle = page.locator(".ant-slider-handle").first();
    await expect(handle).toBeVisible();
    await handle.focus();
    await page.keyboard.press("ArrowRight");
    await waitForToast(page, "Max records updated");
    await expect
      .poll(async () => {
        const res = await request.get(`${apiBase}/config/performance`);
        const body = (await res.json()) as { traffic: { max_records: number } };
        return body.traffic.max_records;
      })
      .not.toBe(original);

    const refreshedRes = await request.get(`${apiBase}/config/performance`);
    const refreshed = (await refreshedRes.json()) as { traffic: { max_records: number } };
    await page2.reload();
    await page2.getByRole("tab", { name: /Performance/ }).click();
    await expect(page2.locator("body")).toContainText(
      refreshed.traffic.max_records.toLocaleString(),
    );
    await page2.close();
  } finally {
    await request.put(`${apiBase}/config/performance`, {
      data: { max_records: original },
    });
  }
});

test("Settings TLS 与证书页支持开关、模式和只读展示", async ({
  page,
  request,
}) => {
  const tlsRes = await request.get(`${apiBase}/config/tls`);
  const originalTls = await tlsRes.json();

  try {
    await openPage(page, "settings");
    await page.getByRole("tab", { name: /Proxy/ }).click();
    await expect(page.locator("body")).toContainText("HTTPS Interception");

    await page.getByTestId("settings-tls-enable-switch").click();
    await expect
      .poll(async () => {
        const res = await request.get(`${apiBase}/config/tls`);
        const body = await res.json();
        return body.enable_tls_interception;
      })
      .toBe(true);

    await page.getByTestId("settings-tls-include-input").fill("*.ui-e2e.local");
    await page.getByTestId("settings-tls-include-add-button").click();
    await expect(page.locator("body")).toContainText("*.ui-e2e.local");

    await page.getByTestId("settings-tls-exclude-input").fill("*.ui-e2e-skip.local");
    await page.getByTestId("settings-tls-exclude-add-button").click();
    await expect(page.locator("body")).toContainText("*.ui-e2e-skip.local");

    await page.getByRole("tab", { name: /Certificate/ }).click();
    await expect(page.locator("body")).toContainText("Certificate Status");
    await expect(page.getByTestId("settings-certificate-download")).toBeVisible();
    await expect(page.getByTestId("settings-certificate-qrcode")).toBeVisible();
  } finally {
    await request.put(`${apiBase}/config/tls`, { data: originalTls });
  }
});
