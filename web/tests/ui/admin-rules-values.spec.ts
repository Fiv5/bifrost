import { test, expect } from "@playwright/test";
import {
  apiBase,
  clearRules,
  clearTraffic,
  clearValues,
  openPage,
  sendProxyRequest,
  startMockHttpServer,
  uniqueName,
  waitForTrafficRow,
} from "./helpers/admin-helpers";

test.describe.configure({ mode: "serial" });

async function changeSort(page: import("@playwright/test").Page, testId: string, label: string) {
  await page.getByTestId(testId).click();
  await page.locator(".ant-select-dropdown").getByText(label, { exact: true }).click();
}

test.beforeEach(async ({ request }) => {
  await clearTraffic(request);
  await clearRules(request);
  await clearValues(request);
});

test("Values 页面完成 CRUD、支持多种排序，并通过 push 自动同步外部写入", async ({
  page,
  context,
  request,
}) => {
  const valueName = uniqueName("a-ui-value");
  const renamedValueName = `${valueName}-renamed`;
  const pushedValueName = uniqueName("z-push-value");

  await openPage(page, "values");
  await expect(page.getByTestId("values-list")).toBeVisible();

  const syncPage = await context.newPage();
  await openPage(syncPage, "values");
  await expect(syncPage.getByTestId("values-list")).toBeVisible();

  await page.getByTestId("value-new-button").click();
  await page
    .getByRole("dialog")
    .getByPlaceholder("Value name (e.g., api_key, auth_token)")
    .fill(valueName);
  await page.getByRole("button", { name: "Create" }).click();

  const valueItem = page.getByTestId("value-item").filter({ hasText: valueName }).first();
  await expect(valueItem).toBeVisible();
  await valueItem.click();

  await valueItem.getByTestId("value-item-menu").click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("dialog").getByPlaceholder("New name").fill(renamedValueName);
  await page.getByRole("button", { name: "Rename" }).click();
  const renamedValueItem = page
    .getByTestId("value-item")
    .filter({ hasText: renamedValueName })
    .first();
  await expect(renamedValueItem).toBeVisible();

  await page.getByTestId("value-new-button").click();
  await page
    .getByRole("dialog")
    .getByPlaceholder("Value name (e.g., api_key, auth_token)")
    .fill(pushedValueName);
  await page.getByRole("button", { name: "Create" }).click();

  await expect(
    page.getByTestId("value-item").filter({ hasText: pushedValueName }).first(),
  ).toBeVisible();
  await expect(page.getByTestId("value-item").first()).toHaveAttribute(
    "data-value-name",
    pushedValueName,
  );
  await expect(
    syncPage.getByTestId("value-item").filter({ hasText: pushedValueName }).first(),
  ).toBeVisible();
  await expect(syncPage.getByTestId("value-item").first()).toHaveAttribute(
    "data-value-name",
    pushedValueName,
  );

  await changeSort(page, "value-sort-select", "Name");
  await expect(page.getByTestId("value-item").first()).toHaveAttribute(
    "data-value-name",
    renamedValueName,
  );

  await page.waitForTimeout(1100);
  const updateValueRes = await request.put(
    `${apiBase}/values/${encodeURIComponent(renamedValueName)}`,
    { data: { value: "updated-by-api" } },
  );
  if (!updateValueRes.ok()) {
    throw new Error(await updateValueRes.text());
  }
  await page.getByTestId("value-refresh-button").click();

  await changeSort(page, "value-sort-select", "Updated");
  await expect(page.getByTestId("value-item").first()).toHaveAttribute(
    "data-value-name",
    renamedValueName,
  );

  await renamedValueItem.getByTestId("value-item-menu").click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page.getByRole("dialog", { name: "Delete Value" }).getByRole("button", { name: "Delete" }).click();
  await expect(
    page.getByTestId("value-item").filter({ hasText: renamedValueName }),
  ).toHaveCount(0);
  await expect(
    syncPage.getByTestId("value-item").filter({ hasText: renamedValueName }),
  ).toHaveCount(0);

  await syncPage.close();
});

test("Rules 页面创建、应用、禁用、删除规则，并支持切换排序", async ({
  page,
  request,
}) => {
  const ruleName = uniqueName("a-ui-rule");
  const latestRuleName = uniqueName("z-ui-rule");
  const server = await startMockHttpServer();

  try {
    await openPage(page, "rules");
    await expect(page.getByTestId("rules-list")).toBeVisible();

    await page.getByTestId("rule-new-button").click();
    await page.getByRole("dialog").getByPlaceholder("Rule name").fill(ruleName);
    await page.getByRole("button", { name: "Create" }).click();

    const ruleItem = page.getByTestId("rule-item").filter({ hasText: ruleName }).first();
    await expect(ruleItem).toBeVisible();
    await ruleItem.click();

    await page.getByTestId("rule-new-button").click();
    await page.getByRole("dialog").getByPlaceholder("Rule name").fill(latestRuleName);
    await page.getByRole("button", { name: "Create" }).click();
    await expect(page.getByTestId("rule-item").first()).toHaveAttribute(
      "data-rule-name",
      latestRuleName,
    );

    await changeSort(page, "rule-sort-select", "Name");
    await expect(page.getByTestId("rule-item").first()).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );

    await page.waitForTimeout(1100);
    const updateRuleRes = await request.put(`${apiBase}/rules/${encodeURIComponent(ruleName)}`, {
      data: { content: "127.0.0.1 reqHeaders://X-UI-Rule=alpha" },
    });
    if (!updateRuleRes.ok()) {
      throw new Error(await updateRuleRes.text());
    }
    await page.getByTestId("rule-refresh-button").click();

    await changeSort(page, "rule-sort-select", "Updated");
    await expect(page.getByTestId("rule-item").first()).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );

    await sendProxyRequest(`http://127.0.0.1:${server.port}/rules-check`);
    await expect.poll(() => server.requests.length).toBeGreaterThan(0);
    expect(server.requests.at(-1)?.headers["x-ui-rule"]).toBe("alpha");

    await openPage(page, "traffic");
    const row = await waitForTrafficRow(page, "/rules-check");
    await row.click();
    await expect(page.getByTestId("traffic-detail-header")).toContainText("/rules-check");

    await openPage(page, "rules");
    await expect(ruleItem).toBeVisible();
    await ruleItem.locator(".ant-switch").click();

    const requestsBefore = server.requests.length;
    await sendProxyRequest(`http://127.0.0.1:${server.port}/rules-disabled`);
    await expect.poll(() => server.requests.length).toBeGreaterThan(requestsBefore);
    expect(server.requests.at(-1)?.headers["x-ui-rule"]).toBeUndefined();

    await ruleItem.click({ button: "right" });
    await page.getByRole("menuitem", { name: "Delete" }).click();
    await page.getByRole("dialog", { name: "Delete Rule" }).getByRole("button", { name: "Delete" }).click();
    await expect(
      page.getByTestId("rule-item").filter({ hasText: ruleName }),
    ).toHaveCount(0);
  } finally {
    await server.close();
  }
});
