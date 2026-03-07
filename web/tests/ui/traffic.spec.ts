import { test, expect } from "@playwright/test";
import type { APIRequestContext } from "@playwright/test";
import { createServer, request as httpRequest } from "node:http";
import type { AddressInfo } from "node:net";
import { promisify } from "node:util";
import { execFile } from "node:child_process";
import WebSocket, { WebSocketServer } from "ws";
import { HttpProxyAgent } from "http-proxy-agent";

const execFileAsync = promisify(execFile);
const proxyUrl = process.env.PROXY_URL || "http://127.0.0.1:9900";
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
    if (!url.startsWith("/sse")) {
      res.statusCode = 404;
      res.end();
      return;
    }
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    });
    res.write(`id: 1\ndata: alpha\n\n`);
    res.write(`id: 2\ndata: beta\n\n`);
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

test("SSE 事件订阅与列表展示正确", async ({ page, request }) => {
  await clearTraffic(request);
  const server = await startSseServer();
  const token = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const ssePath = `/sse?token=${token}`;

  await streamSseViaProxy(`http://127.0.0.1:${server.port}${ssePath}`);

  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();

  const sseRow = page
    .getByTestId("traffic-row")
    .filter({ hasText: "/sse" })
    .last();
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
