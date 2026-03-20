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

test("Rules 页面支持持久化排序，且解析顺序符合列表顺序", async ({
  page,
  request,
}) => {
  const ruleName = uniqueName("alpha-rule");
  const latestRuleName = uniqueName("beta-rule");
  const server = await startMockHttpServer();

  try {
    const createRuleRes = await request.post(`${apiBase}/rules`, {
      data: {
        name: ruleName,
        content: "127.0.0.1 reqHeaders://X-UI-Rule=alpha",
      },
    });
    if (!createRuleRes.ok()) {
      throw new Error(await createRuleRes.text());
    }
    const createLatestRuleRes = await request.post(`${apiBase}/rules`, {
      data: {
        name: latestRuleName,
        content: "127.0.0.1 reqHeaders://X-UI-Rule=beta",
      },
    });
    if (!createLatestRuleRes.ok()) {
      throw new Error(await createLatestRuleRes.text());
    }

    await openPage(page, "rules");
    await expect(page.getByTestId("rules-list")).toBeVisible();
    await page.evaluate(() => {
      document.querySelectorAll(".ant-modal-mask, .ant-modal-wrap").forEach((element) => {
        const node = element as HTMLElement;
        node.style.display = "none";
        node.style.pointerEvents = "none";
      });
    });

    const ruleItem = page.getByTestId("rule-item").filter({ hasText: ruleName }).first();
    const latestRuleItem = page
      .getByTestId("rule-item")
      .filter({ hasText: latestRuleName })
      .first();
    await expect(ruleItem).toBeVisible();
    await expect(latestRuleItem).toBeVisible();

    const updateRuleRes = await request.put(`${apiBase}/rules/${encodeURIComponent(ruleName)}`, {
      data: { content: "127.0.0.1 reqHeaders://X-UI-Rule=alpha" },
    });
    if (!updateRuleRes.ok()) {
      throw new Error(await updateRuleRes.text());
    }
    await page.getByTestId("rule-refresh-button").click();

    await expect(page.getByTestId("rule-item").nth(0)).toHaveAttribute(
      "data-rule-name",
      latestRuleName,
    );
    await expect(page.getByTestId("rule-item").nth(1)).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );

    await sendProxyRequest(`http://127.0.0.1:${server.port}/rules-check`);
    await expect.poll(() => server.requests.length).toBeGreaterThan(0);
    expect(server.requests.at(-1)?.headers["x-ui-rule"]).toBe("beta");

    await ruleItem.dragTo(latestRuleItem, {
      targetPosition: { x: 20, y: 4 },
    });

    await expect(page.getByTestId("rule-item").nth(0)).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );
    await expect(page.getByTestId("rule-item").nth(1)).toHaveAttribute(
      "data-rule-name",
      latestRuleName,
    );

    const requestsAfterReorder = server.requests.length;
    await sendProxyRequest(`http://127.0.0.1:${server.port}/rules-reordered`);
    await expect.poll(() => server.requests.length).toBeGreaterThan(requestsAfterReorder);
    expect(server.requests.at(-1)?.headers["x-ui-rule"]).toBe("alpha");

    await openPage(page, "traffic");
    const row = await waitForTrafficRow(page, "/rules-reordered");
    await row.click();
    await expect(page.getByTestId("traffic-detail-header")).toContainText("/rules-reordered");

    await openPage(page, "rules");
    await expect(ruleItem).toBeVisible();
    await ruleItem.locator(".ant-switch").click();

    await page.getByTestId("rule-sort-select").click();
    await page.locator(".ant-select-dropdown").getByText("Name", { exact: true }).click();
    await expect(page.getByTestId("rule-item").first()).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );

    await page.getByTestId("rule-sort-select").click();
    await page.locator(".ant-select-dropdown").getByText("Manual", { exact: true }).click();
    await expect(page.getByTestId("rule-item").first()).toHaveAttribute(
      "data-rule-name",
      ruleName,
    );

    const requestsAfterManualRestore = server.requests.length;
    await sendProxyRequest(`http://127.0.0.1:${server.port}/rules-manual-restored`);
    await expect.poll(() => server.requests.length).toBeGreaterThan(requestsAfterManualRestore);
    expect(server.requests.at(-1)?.headers["x-ui-rule"]).toBe("beta");

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
