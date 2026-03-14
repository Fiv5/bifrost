import { test, expect } from "@playwright/test";
import {
  apiBase,
  clearReplay,
  clearTraffic,
  openPage,
  startMockHttpServer,
  uniqueName,
  waitForTrafficRow,
} from "./helpers/admin-helpers";

test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ request }) => {
  await clearTraffic(request);
  await clearReplay(request);
});

test("Replay 页面保存请求、创建分组、移动并执行，然后查看历史记录", async ({
  page,
  request,
}) => {
  const requestName = uniqueName("replay-request");
  const groupName = uniqueName("replay-group");
  const server = await startMockHttpServer();

  try {
    await openPage(page, "replay");
    await expect(page.getByTestId("replay-request-panel")).toBeVisible();

    await page.getByTestId("replay-url-input").fill(`http://127.0.0.1:${server.port}/replay-check`);
    await page.getByTestId("replay-save-button").click();
    const saveDialog = page.getByRole("dialog", { name: "Save Request" });
    await saveDialog.getByTestId("replay-save-name-input").fill(requestName);
    await saveDialog.getByRole("button", { name: "Save" }).click();

    await page.getByTestId("replay-new-group-button").click();
    await page.getByTestId("replay-group-name-input").fill(groupName);
    await page.getByRole("button", { name: "Create" }).click();
    await expect(page.getByText(groupName, { exact: true }).first()).toBeVisible();

    const requestsRes = await request.get(`${apiBase}/replay/requests?saved=true&limit=100`);
    expect(requestsRes.ok()).toBeTruthy();
    const requestsPayload = (await requestsRes.json()) as {
      requests: Array<{ id: string; name?: string; url: string }>;
    };
    const replayRequest = requestsPayload.requests.find(
      (item) => item.name === requestName || item.url.endsWith("/replay-check"),
    );
    expect(replayRequest?.id).toBeTruthy();

    const groupsRes = await request.get(`${apiBase}/replay/groups`);
    expect(groupsRes.ok()).toBeTruthy();
    const groupsPayload = (await groupsRes.json()) as {
      groups: Array<{ id: string; name: string }>;
    };
    const replayGroup = groupsPayload.groups.find((item) => item.name === groupName);
    expect(replayGroup?.id).toBeTruthy();

    const moveRes = await request.put(
      `${apiBase}/replay/requests/${encodeURIComponent(replayRequest!.id)}/move`,
      { data: { group_id: replayGroup!.id } },
    );
    expect(moveRes.ok()).toBeTruthy();

    const requestNode = page
      .getByTestId("replay-request-node")
      .filter({ hasText: requestName })
      .first();
    await expect(requestNode).toBeVisible();

    await requestNode.click();
    await page.getByTestId("replay-send-button").click();
    await expect.poll(() => server.requests.length).toBeGreaterThan(0);

    await page.getByTestId("replay-mode-history").click();
    await expect(page.getByText("/replay-check")).toBeVisible();

    await openPage(page, "traffic");
    await waitForTrafficRow(page, "/replay-check");
  } finally {
    await server.close();
  }
});
