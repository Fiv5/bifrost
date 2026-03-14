import { test, expect } from "@playwright/test";
import {
  clearRules,
  clearScripts,
  clearTraffic,
  openPage,
  setMonacoEditor,
  uniqueName,
  waitForToast,
} from "./helpers/admin-helpers";

test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ request }) => {
  await clearTraffic(request);
  await clearRules(request);
  await clearScripts(request);
});

test("Scripts 页面完成创建、测试、push 同步，并让请求脚本真实作用于代理流量", async ({
  page,
  context,
}) => {
  const requestScriptName = uniqueName("request-script");
  const responseScriptName = uniqueName("response-script");
  const decodeScriptName = uniqueName("decode-script");
  const pushedScriptName = uniqueName("push-script");
  await openPage(page, "scripts");
  await expect(page.getByTestId("scripts-list-panel")).toBeVisible();

  const syncPage = await context.newPage();
  await openPage(syncPage, "scripts");
  await expect(syncPage.getByTestId("scripts-list-panel")).toBeVisible();

  await page.getByTestId("scripts-new-request-button").click();
  await setMonacoEditor(
    page,
    page.getByTestId("scripts-editor"),
    'request.headers["x-script-ui"] = "applied";',
  );
  await page.getByTestId("scripts-save-button").click();
  const saveDialog = page.getByRole("dialog", { name: "Save New Script" });
  await saveDialog
    .getByPlaceholder("Enter script name (e.g., api/add-auth-header)")
    .fill(requestScriptName);
  await saveDialog.getByRole("button", { name: "Save" }).click();
  await waitForToast(page, "Script created");

  const requestNode = page
    .locator('[data-testid="script-tree-node"]')
    .filter({ hasText: requestScriptName.split("/").pop() || requestScriptName })
    .first();
  await expect(requestNode).toBeVisible();
  await expect(
    syncPage
      .locator('[data-testid="script-tree-node"]')
      .filter({ hasText: requestScriptName.split("/").pop() || requestScriptName })
      .first(),
  ).toBeVisible();

  await requestNode.click();
  await page.getByTestId("scripts-test-button").click();
  await expect(page.getByTestId("scripts-test-result-panel")).toBeVisible();

  await page.getByTestId("scripts-new-response-button").click();
  await setMonacoEditor(
    page,
    page.getByTestId("scripts-editor"),
    'response.headers["x-response-script"] = "enabled";',
  );
  await page.getByTestId("scripts-save-button").click();
  await saveDialog
    .getByPlaceholder("Enter script name (e.g., api/add-auth-header)")
    .fill(responseScriptName);
  await saveDialog.getByRole("button", { name: "Save" }).click();
  await waitForToast(page, "Script created");

  await page.getByTestId("scripts-new-decode-button").click();
  await setMonacoEditor(
    page,
    page.getByTestId("scripts-editor"),
    'ctx.output = { data: "decoded-ui", code: "ok", msg: "from-ui" };',
  );
  await page.getByTestId("scripts-save-button").click();
  await saveDialog
    .getByPlaceholder("Enter script name (e.g., api/add-auth-header)")
    .fill(decodeScriptName);
  await saveDialog.getByRole("button", { name: "Save" }).click();
  await waitForToast(page, "Script created");

  const decodeNode = page
    .locator('[data-testid="script-tree-node"]')
    .filter({ hasText: decodeScriptName.split("/").pop() || decodeScriptName })
    .first();
  await decodeNode.click();
  await page.getByTestId("scripts-test-button").click();
  await expect(page.getByTestId("scripts-test-result-panel")).toBeVisible();

  await page.getByTestId("scripts-new-request-button").click();
  await setMonacoEditor(
    page,
    page.getByTestId("scripts-editor"),
    'request.headers["x-push-sync"] = "ok";',
  );
  await page.getByTestId("scripts-save-button").click();
  await saveDialog
    .getByPlaceholder("Enter script name (e.g., api/add-auth-header)")
    .fill(pushedScriptName);
  await saveDialog.getByRole("button", { name: "Save" }).click();
  await waitForToast(page, "Script created");

  await expect(
    page.locator('[data-testid="script-tree-node"]').filter({ hasText: pushedScriptName }).first(),
  ).toBeVisible();
  await expect(
    syncPage.locator('[data-testid="script-tree-node"]').filter({ hasText: pushedScriptName }).first(),
  ).toBeVisible();

  await page.locator('[data-testid="script-tree-node"]').filter({ hasText: requestScriptName }).first().click();
  await page.getByTestId("scripts-delete-button").click();
  await page.getByRole("dialog", { name: "Delete Script" }).getByRole("button", { name: "Delete" }).click();
  await waitForToast(page, "Script deleted");
  await expect(
    page.locator('[data-testid="script-tree-node"]').filter({ hasText: requestScriptName }),
  ).toHaveCount(0);
  await expect(
    syncPage.locator('[data-testid="script-tree-node"]').filter({ hasText: requestScriptName }),
  ).toHaveCount(0);

  await syncPage.close();
});
