import { test, expect } from "@playwright/test";
import type { APIRequestContext } from "@playwright/test";
import { createServer, request as httpRequest } from "node:http";
import type { AddressInfo } from "node:net";
import { promisify } from "node:util";
import { execFile, spawn } from "node:child_process";
import WebSocket, { WebSocketServer } from "ws";
import { HttpProxyAgent } from "http-proxy-agent";

const execFileAsync = promisify(execFile);
const proxyUrl =
  process.env.PROXY_URL ||
  `http://127.0.0.1:${process.env.BIFROST_UI_TEST_PORT ?? process.env.BACKEND_PORT ?? 9910}`;
const proxyHost = new URL(proxyUrl);
const apiBase =
  process.env.ADMIN_API_BASE || `${proxyUrl.replace(/\/$/, "")}/_bifrost/api`;

const startMockServer = async () => {
  const server = createServer((req, res) => {
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.end(JSON.stringify({ path: req.url || "/" }));
  });

  await new Promise<void>((resolve) => server.listen(0, resolve));
  const port = (server.address() as AddressInfo).port;

  return {
    port,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((err?: Error) => (err ? reject(err) : resolve())),
      ),
  };
};

const startSseServer = async () => {
  const server = createServer((req, res) => {
    const url = req.url || "";
    if (!url.startsWith("/sse-test")) {
      res.statusCode = 404;
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    for (let i = 1; i <= 40; i += 1) {
      if (i === 20) {
        res.write(
          `id: ${i}\ndata: {"type":"target-long","a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7,"h":8,"i":9,"j":10,"k":11}\n\n`,
        );
        continue;
      }
      if (i === 1) {
        res.write(`id: ${i}\ndata: alpha\n\n`);
        continue;
      }
      if (i === 2) {
        res.write(`id: ${i}\ndata: beta\n\n`);
        continue;
      }
      res.write(`id: ${i}\ndata: event-${i}\n\n`);
    }
    res.end();
  });

  await new Promise<void>((resolve) => server.listen(0, resolve));
  const port = (server.address() as AddressInfo).port;
  return {
    port,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((err?: Error) => (err ? reject(err) : resolve())),
      ),
  };
};

const startWsServer = async () => {
  const wss = new WebSocketServer({ port: 0, host: "127.0.0.1" });
  wss.on("connection", (socket: WebSocket) => {
    socket.on("message", (data: WebSocket.RawData) => {
      socket.send(data);
    });
  });
  await new Promise<void>((resolve) => wss.on("listening", resolve));
  const port = (wss.address() as AddressInfo).port;
  return {
    port,
    waitForClient: async () => {
      await expect
        .poll(() => wss.clients.size, { timeout: 15000 })
        .toBeGreaterThan(0);
    },
    close: () =>
      new Promise<void>((resolve, reject) =>
        wss.close((err?: Error) => (err ? reject(err) : resolve())),
      ),
  };
};

const sendProxyRequest = async (url: string) => {
  await execFileAsync(
    "curl",
    ["-sS", "--fail", "-x", proxyUrl, url],
    { timeout: 10000 },
  );
};

const clearTraffic = async (request: APIRequestContext) => {
  await request.delete(`${apiBase}/traffic`);
};

const streamSseViaProxy = async (url: string) => {
  const target = new URL(url);
  await new Promise<void>((resolve, reject) => {
    const req = httpRequest(
      {
        host: proxyHost.hostname,
        port: proxyHost.port || 80,
        method: "GET",
        path: url,
        headers: {
          Host: target.host,
        },
      },
      (res) => {
        res.on("data", () => {});
        res.on("end", () => resolve());
      },
    );
    req.on("error", reject);
    req.end();
  });
};

const sendWsViaProxy = async (url: string) => {
  await new Promise<void>((resolve, reject) => {
    const agent = new HttpProxyAgent(proxyUrl);
    const ws = new WebSocket(url, { agent });
    ws.on("open", () => {
      ws.send("hello");
      ws.send(Buffer.from([1, 2, 3, 4, 5, 6]));
      ws.close();
    });
    ws.on("close", () => resolve());
    ws.on("error", (err: Error) => reject(err));
  });
};

test("加载流量列表并显示详情", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startMockServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const path = `/hello-${token}`;

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  await sendProxyRequest(`http://127.0.0.1:${server.port}${path}`);
  await page.reload();
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  const row = page.getByTestId("traffic-row").filter({ hasText: path }).first();
  await expect(row).toBeVisible();
  const firstRow = row;
  await expect(firstRow).toHaveAttribute(
    "data-response-size",
    expect.stringMatching(/^[1-9]\d*$/),
  );
  await firstRow.click();

  const header = page.getByTestId("traffic-detail-header");
  await expect(header).toContainText("GET");
  await expect(header).toHaveAttribute(
    "data-url",
    expect.stringContaining("/hello"),
  );

  await server.close();
});

test("清空流量时前端立即清理", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startMockServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const path = `/clear-${token}`;

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  await sendProxyRequest(`http://127.0.0.1:${server.port}${path}`);
  const row = page.getByTestId("traffic-row").filter({ hasText: path }).first();
  await expect(row).toBeVisible();

  let deleteSeen = false;
  await page.route("**/_bifrost/api/traffic", async (route, req) => {
    if (req.method() === "DELETE") {
      deleteSeen = true;
      await new Promise((r) => setTimeout(r, 2000));
    }
    await route.continue();
  });

  await page.getByTestId("toolbar-clear-dropdown").click();
  await page.getByRole("menuitem", { name: "Clear all" }).click();
  await page.getByRole("button", { name: "Clear" }).click();

  await expect(row).toHaveCount(0, { timeout: 500 });
  expect(deleteSeen).toBeTruthy();

  await server.close();
});

test("实时更新与页面刷新保持数据", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startMockServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const firstPath = `/first-${token}`;
  const secondPath = `/second-${token}`;

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  await sendProxyRequest(`http://127.0.0.1:${server.port}${firstPath}`);
  await expect(
    page.getByTestId("traffic-row").filter({ hasText: firstPath }).first(),
  ).toBeVisible();

  await sendProxyRequest(`http://127.0.0.1:${server.port}${secondPath}`);
  await expect(
    page.getByTestId("traffic-row").filter({ hasText: secondPath }).first(),
  ).toBeVisible();

  await page.reload();
  await expect(
    page.getByTestId("traffic-row").filter({ hasText: secondPath }).first(),
  ).toBeVisible();

  await server.close();
});

test("订阅更新提示与滚动状态一致", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startMockServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  for (let i = 0; i < 30; i += 1) {
    await sendProxyRequest(
      `http://127.0.0.1:${server.port}/batch-${token}-${i}`,
    );
  }

  await expect(page.getByTestId("traffic-row").first()).toBeVisible();

  const scrollContainer = page.getByTestId("traffic-table-scroll");
  await scrollContainer.evaluate((el) => {
    el.scrollTop = 0;
    el.dispatchEvent(new Event("scroll"));
  });

  await sendProxyRequest(`http://127.0.0.1:${server.port}/latest-${token}`);
  const scrollBottomButton = page.getByTestId("traffic-scroll-bottom");
  await expect(scrollBottomButton).toBeVisible();
  await scrollBottomButton.click({ force: true });
  await expect(
    page.getByTestId("traffic-row").filter({ hasText: `/latest-${token}` }).first(),
  ).toBeVisible();

  await server.close();
});

test("WebSocket 帧与 size 更新展示正确", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startWsServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const wsPath = `/ws-${token}`;

  await sendWsViaProxy(`ws://127.0.0.1:${server.port}${wsPath}`);

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  const wsRow = page
    .getByTestId("traffic-row")
    .filter({ hasText: wsPath })
    .last();
  const frameCountAttr = await wsRow.getAttribute("data-frame-count");
  const frameCount = Number(frameCountAttr ?? "0");
  expect(frameCount).toBeGreaterThanOrEqual(2);
  await wsRow.click();
  await page.getByTestId("response-tab-messages").click();

  await expect(page.getByTestId("ws-frames-pane")).toBeVisible();
  await expect(page.getByTestId("ws-frames-summary")).toContainText("frames");
  const frameRows = page.getByTestId("ws-frame-row");
  await expect(frameRows.first()).toBeVisible();
  expect(await frameRows.count()).toBeGreaterThanOrEqual(2);
  await expect(page.getByTestId("ws-frames-table")).toContainText("6 B");

  await server.close();
});

test("WebSocket 外部站点回显与 size 增长", async ({ page, request }) => {
  await clearTraffic(request);
  await sendWsViaProxy("wss://echo.websocket.org/");

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  const wsRow = page
    .getByTestId("traffic-row")
    .filter({ hasText: "echo.websocket.org" })
    .first();
  await expect(wsRow).toBeVisible();

  await expect
    .poll(async () => {
      const frameCountAttr = await wsRow.getAttribute("data-frame-count");
      return Number(frameCountAttr ?? "0");
    })
    .toBeGreaterThanOrEqual(2);

  await wsRow.click();
  await page.getByTestId("response-tab-messages").click();

  await expect(page.getByTestId("ws-frames-pane")).toBeVisible();
  await expect(page.getByTestId("ws-frames-table")).toContainText("hello");
  await expect(page.getByTestId("ws-frames-table")).toContainText("6 B");
});

test("WebSocket 详情 Messages 面板打开后可实时收到新消息", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startWsServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const wsPath = `/ws-live-${token}`;
  const wsUrl = `ws://127.0.0.1:${server.port}${wsPath}`;
  const agent = new HttpProxyAgent(proxyUrl);
  const ws = new WebSocket(wsUrl, { agent });

  try {
    await new Promise<void>((resolve, reject) => {
      ws.once("open", resolve);
      ws.once("error", reject);
    });
    await server.waitForClient();

    ws.send(`seed-${token}`);

    await page.goto("/_bifrost/traffic");
    await expect(page.getByTestId("traffic-table")).toBeVisible();

    const wsRow = page.getByTestId("traffic-row").filter({ hasText: wsPath }).last();
    await expect(wsRow).toBeVisible();
    await wsRow.click();
    await page.getByTestId("response-tab-messages").click();

    await expect(page.getByTestId("ws-frames-pane")).toBeVisible();
    await expect(page.getByTestId("ws-frames-table")).toContainText(`seed-${token}`);
    await page.waitForTimeout(500);

    const livePayload = `after-open-${token}`;
    for (let i = 0; i < 5; i += 1) {
      ws.send(`${livePayload}-${i}`);
      await page.waitForTimeout(200);
    }

    await expect(page.getByTestId("ws-frames-table")).toContainText(livePayload);
  } finally {
    if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
      await new Promise<void>((resolve) => {
        ws.once("close", resolve);
        ws.close();
      });
    }
    await server.close();
  }
});

test("WebSocket 列表未到底部时不应强制滚动到底部", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startWsServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const wsPath = `/ws-scroll-${token}`;
  const wsUrl = `ws://127.0.0.1:${server.port}${wsPath}`;

  const agent = new HttpProxyAgent(proxyUrl);
  const ws = new WebSocket(wsUrl, { agent });

  await new Promise<void>((resolve, reject) => {
    ws.once("open", resolve);
    ws.once("error", reject);
  });

  for (let i = 0; i < 120; i += 1) {
    ws.send(`seed-${token}-${i}`);
  }

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  const wsRow = page
    .getByTestId("traffic-row")
    .filter({ hasText: wsPath })
    .last();
  await expect(wsRow).toBeVisible();
  await wsRow.click();
  await page.getByTestId("response-tab-messages").click();

  await expect(page.getByTestId("ws-frames-pane")).toBeVisible();

  const summary = page.getByTestId("ws-frames-summary");
  const summaryHandle = await summary.elementHandle();
  if (summaryHandle) {
    await page.waitForFunction(
      (el: SVGElement | HTMLElement) => {
        const text = el.textContent || "";
        const match = text.match(/(\d+)\s+of\s+(\d+)\s+frames/);
        if (!match) return false;
        return Number(match[1]) >= 50;
      },
      summaryHandle,
    );
  }

  const scrollContainer = page.getByTestId("ws-frames-table");
  await scrollContainer.evaluate((el) => {
    el.scrollTop = 0;
    el.dispatchEvent(new Event("scroll"));
  });
  const scrollTopBefore = await scrollContainer.evaluate((el) => el.scrollTop);

  for (let i = 120; i < 140; i += 1) {
    ws.send(`append-${token}-${i}`);
  }

  if (summaryHandle) {
    await page.waitForFunction(
      (el: SVGElement | HTMLElement) => {
        const text = el.textContent || "";
        const match = text.match(/(\d+)\s+of\s+(\d+)\s+frames/);
        if (!match) return false;
        return Number(match[1]) >= 70;
      },
      summaryHandle,
    );
  }

  const scrollTopAfter = await scrollContainer.evaluate((el) => el.scrollTop);
  expect(scrollTopAfter).toBe(scrollTopBefore);

  await new Promise<void>((resolve) => {
    ws.once("close", resolve);
    ws.close();
  });
  await server.close();
});

test("SSE 事件订阅与列表展示正确", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startSseServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const ssePath = `/sse-test?token=${token}`;

  await streamSseViaProxy(`http://127.0.0.1:${server.port}${ssePath}`);

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  let recordId: string | null = null;
  await expect
    .poll(async () => {
      const row = page
        .getByTestId("traffic-row")
        .filter({ hasText: "/sse-test" })
        .last();
      recordId = await row.getAttribute("data-record-id");
      return recordId;
    })
    .toMatch(/^REQ-/);

  const sseRow = page.locator(
    `[data-testid="traffic-row"][data-record-id="${recordId}"]`,
  );
  await expect(sseRow).toHaveAttribute(
    "data-response-size",
    expect.stringMatching(/^[1-9]\d*$/),
  );
  await sseRow.click();
  await page.getByTestId("response-tab-messages").click();

  await expect(page.getByTestId("sse-message-container")).toBeVisible();
  await expect(page.getByTestId("sse-message-count")).toContainText("events");
  await expect(page.getByTestId("sse-message-list")).toContainText("alpha");
  await expect(page.getByTestId("sse-message-list")).toContainText("beta");

  await server.close();
});

test("SSE 展开/折叠后相邻项不应出现高度错位", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startSseServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const ssePath = `/sse-test?token=${token}`;

  await streamSseViaProxy(`http://127.0.0.1:${server.port}${ssePath}`);

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  let recordId: string | null = null;
  await expect
    .poll(async () => {
      const row = page
        .getByTestId("traffic-row")
        .filter({ hasText: "/sse-test" })
        .last();
      recordId = await row.getAttribute("data-record-id");
      return recordId;
    })
    .toMatch(/^REQ-/);

  const sseRow = page.locator(
    `[data-testid="traffic-row"][data-record-id="${recordId}"]`,
  );
  await sseRow.click();
  await page.getByTestId("response-tab-messages").click();

  const container = page.getByTestId("sse-message-container");
  await expect(container).toBeVisible();

  const search = container.getByPlaceholder("Search events...");
  await search.fill("target-long");

  const expandedCard = container
    .getByTestId("sse-event-card")
    .filter({ hasText: "target-long" })
    .first();

  await expect(expandedCard).toBeVisible();
  const beforeBox = await expandedCard.boundingBox();
  expect(beforeBox).not.toBeNull();

  await expandedCard.getByTestId("sse-event-toggle").click();

  await expect
    .poll(async () => (await expandedCard.boundingBox())?.height || 0)
    .toBeGreaterThan((beforeBox?.height || 0) + 10);

  const cards = container.getByTestId("sse-event-card");
  const cardCount = await cards.count();
  const boxes: Array<{ index: number; y: number; bottom: number }> = [];
  for (let i = 0; i < cardCount; i += 1) {
    const box = await cards.nth(i).boundingBox();
    if (!box) continue;
    boxes.push({ index: i, y: box.y, bottom: box.y + box.height });
  }
  boxes.sort((a, b) => a.y - b.y);

  const expandedBox = await expandedCard.boundingBox();
  expect(expandedBox).not.toBeNull();
  const expandedBottom = (expandedBox?.y || 0) + (expandedBox?.height || 0);

  const scroll = container.getByTestId("sse-message-scroll");
  let next: { index: number; y: number; bottom: number } | undefined;
  let currentExpandedBottom = expandedBottom;
  for (let i = 0; i < 4; i += 1) {
    const currentExpandedBox = await expandedCard.boundingBox();
    expect(currentExpandedBox).not.toBeNull();
    currentExpandedBottom =
      (currentExpandedBox?.y || 0) + (currentExpandedBox?.height || 0);

    boxes.length = 0;
    const count = await cards.count();
    for (let j = 0; j < count; j += 1) {
      const box = await cards.nth(j).boundingBox();
      if (!box) continue;
      boxes.push({ index: j, y: box.y, bottom: box.y + box.height });
    }
    boxes.sort((a, b) => a.y - b.y);

    next = boxes.find((b) => b.y > currentExpandedBottom);
    if (next) break;

    await scroll.evaluate((el) => {
      el.scrollTop += 320;
    });
    await page.waitForTimeout(60);
  }

  expect(next).toBeTruthy();
  expect((next?.y || 0) - currentExpandedBottom).toBeGreaterThanOrEqual(6);

  await server.close();
});

test("SSE 外部站点流式更新与 size 增长", async ({ page, request }) => {
  await clearTraffic(request);
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const ssePath = `/external-sse-${token}`;
  const server = createServer((req, res) => {
    if (req.url !== ssePath) {
      res.statusCode = 404;
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    res.write("id: 1\ndata: alpha\n\n");
    const timer = setInterval(() => {
      res.write(`id: ${Date.now()}\ndata: update-${Date.now()}\n\n`);
    }, 400);
    req.on("close", () => {
      clearInterval(timer);
      res.end();
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  const stream = spawn(
    "curl",
    [
      "-sS",
      "-N",
      "--max-time",
      "6",
      "-x",
      proxyUrl,
      `http://127.0.0.1:${port}${ssePath}`,
    ],
    { stdio: "ignore" },
  );

  try {
    await page.goto("/_bifrost/traffic");
    await expect(page.getByTestId("traffic-table")).toBeVisible();

    const row = page
      .getByTestId("traffic-row")
      .filter({ hasText: ssePath })
      .first();
    await expect(row).toBeVisible();

    const sizeBefore = Number((await row.getAttribute("data-response-size")) || "0");
    await expect
      .poll(async () => {
        const size = await row.getAttribute("data-response-size");
        return Number(size || "0");
      })
      .toBeGreaterThan(sizeBefore);

    await row.click();
    await page.getByTestId("response-tab-messages").click();
    await expect(page.getByTestId("sse-message-container")).toBeVisible();
    await expect
      .poll(async () => {
        const text = await page.getByTestId("sse-message-count").textContent();
        const match = text?.match(/(\d+)\s+events/);
        return match ? Number(match[1]) : 0;
      })
      .toBeGreaterThan(0);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((err?: Error) => (err ? reject(err) : resolve())),
    );
    const waitForClose = new Promise<void>((resolve) => {
      if (stream.exitCode !== null) {
        resolve();
        return;
      }
      stream.once("close", () => resolve());
    });
    if (stream.exitCode === null) {
      stream.kill("SIGTERM");
    }
    await waitForClose;
  }
});

test("SSE 详情 Messages 面板打开后可实时收到新事件", async ({ page, request }) => {
  await clearTraffic(request);
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const ssePath = `/sse-live-${token}`;
  const server = createServer((req, res) => {
    if (req.url !== ssePath) {
      res.statusCode = 404;
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    res.write(`id: 1\ndata: seed-${token}\n\n`);
    let counter = 0;
    const timer = setInterval(() => {
      counter += 1;
      res.write(`id: ${counter + 1}\ndata: after-open-${token}-${counter}\n\n`);
      if (counter >= 6) {
        clearInterval(timer);
        res.end();
      }
    }, 400);
    req.on("close", () => {
      clearInterval(timer);
      res.end();
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  const stream = spawn(
    "curl",
    [
      "-sS",
      "-N",
      "--max-time",
      "8",
      "-x",
      proxyUrl,
      `http://127.0.0.1:${port}${ssePath}`,
    ],
    { stdio: "ignore" },
  );

  try {
    await page.goto("/_bifrost/traffic");
    await expect(page.getByTestId("traffic-table")).toBeVisible();

    const sseRow = page.getByTestId("traffic-row").filter({ hasText: ssePath }).first();
    await expect(sseRow).toBeVisible();
    await sseRow.click();
    await page.getByTestId("response-tab-messages").click();

    await expect(page.getByTestId("sse-message-container")).toBeVisible();
    await expect(page.getByTestId("sse-message-list")).toContainText(`seed-${token}`);
    await expect(page.getByTestId("sse-message-list")).toContainText(`after-open-${token}`);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((err?: Error) => (err ? reject(err) : resolve())),
    );
    const waitForClose = new Promise<void>((resolve) => {
      if (stream.exitCode !== null) {
        resolve();
        return;
      }
      stream.once("close", () => resolve());
    });
    if (stream.exitCode === null) {
      stream.kill("SIGTERM");
    }
    await waitForClose;
  }
});
