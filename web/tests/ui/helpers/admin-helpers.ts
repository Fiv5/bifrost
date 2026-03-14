import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";
import { createServer, type IncomingMessage, type ServerResponse, request as httpRequest } from "node:http";
import type { AddressInfo } from "node:net";
import { promisify } from "node:util";
import { execFile } from "node:child_process";
import WebSocket, { WebSocketServer } from "ws";
import { HttpProxyAgent } from "http-proxy-agent";

const execFileAsync = promisify(execFile);

export const backendPort = Number(
  process.env.BIFROST_UI_TEST_PORT ?? process.env.BACKEND_PORT ?? 9910,
);
export const proxyUrl = process.env.PROXY_URL || `http://127.0.0.1:${backendPort}`;
export const apiBase =
  process.env.ADMIN_API_BASE || `${proxyUrl.replace(/\/$/, "")}/_bifrost/api`;

export function uniqueName(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function readJson(response: Awaited<ReturnType<APIRequestContext["get"]>>) {
  const text = await response.text();
  return text ? JSON.parse(text) : null;
}

export async function clearTraffic(request: APIRequestContext): Promise<void> {
  await request.delete(`${apiBase}/traffic`);
}

export async function clearRules(request: APIRequestContext): Promise<void> {
  const response = await request.get(`${apiBase}/rules`);
  const rules = (await readJson(response)) as Array<{ name: string }>;
  for (const rule of rules || []) {
    await request.delete(`${apiBase}/rules/${encodeURIComponent(rule.name)}`);
  }
}

export async function clearValues(request: APIRequestContext): Promise<void> {
  const response = await request.get(`${apiBase}/values`);
  const payload = (await readJson(response)) as { values?: Array<{ name: string }> };
  for (const value of payload?.values || []) {
    await request.delete(`${apiBase}/values/${encodeURIComponent(value.name)}`);
  }
}

export async function clearScripts(request: APIRequestContext): Promise<void> {
  const response = await request.get(`${apiBase}/scripts`);
  const payload = (await readJson(response)) as Record<string, Array<{ name: string }> | undefined>;
  for (const type of ["request", "response", "decode"] as const) {
    for (const script of payload?.[type] || []) {
      await request.delete(`${apiBase}/scripts/${type}/${encodeURIComponent(script.name)}`);
    }
  }
}

export async function clearReplay(request: APIRequestContext): Promise<void> {
  const requestsRes = await request.get(`${apiBase}/replay/requests?saved=true&limit=500`);
  const requestsPayload = (await readJson(requestsRes)) as { requests?: Array<{ id: string }> };
  for (const item of requestsPayload?.requests || []) {
    await request.delete(`${apiBase}/replay/requests/${encodeURIComponent(item.id)}`);
  }

  const groupsRes = await request.get(`${apiBase}/replay/groups`);
  const groupsPayload = (await readJson(groupsRes)) as { groups?: Array<{ id: string }> } | Array<{ id: string }>;
  const groups = Array.isArray(groupsPayload) ? groupsPayload : groupsPayload?.groups || [];
  for (const group of groups) {
    await request.delete(`${apiBase}/replay/groups/${encodeURIComponent(group.id)}`);
  }

  await request.delete(`${apiBase}/replay/history`);
}

export async function resetAccessControl(request: APIRequestContext): Promise<void> {
  await request.put(`${apiBase}/whitelist/mode`, { data: { mode: "allow_all" } });
  await request.put(`${apiBase}/whitelist/allow-lan`, { data: { allow_lan: false } });
  const statusRes = await request.get(`${apiBase}/whitelist`);
  const status = (await readJson(statusRes)) as {
    whitelist?: string[];
    temporary_whitelist?: string[];
  };
  for (const ip of status?.whitelist || []) {
    await request.delete(`${apiBase}/whitelist`, { data: { ip_or_cidr: ip } });
  }
  for (const ip of status?.temporary_whitelist || []) {
    await request.delete(`${apiBase}/whitelist/temporary`, { data: { ip } });
  }
  await request.delete(`${apiBase}/whitelist/pending`);
}

export async function sendProxyRequest(
  url: string,
  options: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
  } = {},
): Promise<void> {
  const args = ["-sS", "--fail", "-x", proxyUrl, "-X", options.method || "GET"];
  for (const [key, value] of Object.entries(options.headers || {})) {
    args.push("-H", `${key}: ${value}`);
  }
  if (options.body !== undefined) {
    args.push("--data", options.body);
  }
  args.push(url);
  await execFileAsync("curl", args, { timeout: 15000 });
}

export async function sendSseViaProxy(url: string): Promise<void> {
  const target = new URL(url);
  await new Promise<void>((resolve, reject) => {
    const req = httpRequest(
      {
        host: "127.0.0.1",
        port: backendPort,
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
}

export async function sendWsViaProxy(
  url: string,
  messages: Array<string | Buffer> = ["hello", Buffer.from([1, 2, 3, 4])],
): Promise<void> {
  const agent = new HttpProxyAgent(proxyUrl);
  await new Promise<void>((resolve, reject) => {
    const ws = new WebSocket(url, { agent });
    ws.on("open", () => {
      for (const message of messages) {
        ws.send(message);
      }
      ws.close();
    });
    ws.on("close", () => resolve());
    ws.on("error", reject);
  });
}

export async function waitForTrafficRow(page: Page, text: string): Promise<Locator> {
  const row = page.getByTestId("traffic-row").filter({ hasText: text }).first();
  await expect(row).toBeVisible();
  return row;
}

export async function openPage(page: Page, path: string): Promise<void> {
  await page.goto(`/_bifrost/${path.replace(/^\//, "")}`);
}

export async function setMonacoEditor(page: Page, container: Locator, value: string): Promise<void> {
  const input = container.locator("textarea").last();
  await input.click({ force: true });
  await page.keyboard.press(process.platform === "darwin" ? "Meta+A" : "Control+A");
  await page.keyboard.press("Backspace");
  if (value.length > 0) {
    await input.type(value, { delay: 0 });
  }
}

export async function waitForToast(page: Page, text: string): Promise<void> {
  await expect(page.locator(".ant-message-notice").filter({ hasText: text }).last()).toBeVisible();
}

export async function setSelectValue(page: Page, trigger: Locator, optionText: string): Promise<void> {
  await trigger.click();
  await page.locator(".ant-select-dropdown").getByText(optionText, { exact: true }).click();
}

export async function setSliderValue(page: Page, testId: string, targetDelta: number): Promise<void> {
  const slider = page.getByTestId(testId);
  await expect(slider).toBeVisible();
  const handle = slider.locator(".ant-slider-handle").first();
  await handle.focus();
  const key = targetDelta >= 0 ? "ArrowRight" : "ArrowLeft";
  for (let i = 0; i < Math.abs(targetDelta); i += 1) {
    await page.keyboard.press(key);
  }
}

export interface MockHttpRequestRecord {
  method: string;
  url: string;
  headers: Record<string, string | string[] | undefined>;
  body: string;
}

export interface MockHttpServer {
  port: number;
  requests: MockHttpRequestRecord[];
  close: () => Promise<void>;
}

async function readBody(req: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

export async function startMockHttpServer(
  responder?: (req: IncomingMessage, res: ServerResponse, body: string) => void,
): Promise<MockHttpServer> {
  const requests: MockHttpRequestRecord[] = [];
  const server = createServer(async (req, res) => {
    const body = await readBody(req);
    requests.push({
      method: req.method || "GET",
      url: req.url || "/",
      headers: req.headers,
      body,
    });
    if (responder) {
      responder(req, res, body);
      return;
    }
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ path: req.url || "/", echoedHeaders: req.headers, body }));
  });

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  return {
    port,
    requests,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((err) => (err ? reject(err) : resolve())),
      ),
  };
}

export async function startSseServer(): Promise<{ port: number; close: () => Promise<void> }> {
  const server = createServer((req, res) => {
    if (!(req.url || "").startsWith("/sse")) {
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
    res.write("id: 2\ndata: beta\n\n");
    res.write("id: 3\ndata: gamma\n\n");
    res.end();
  });

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  return {
    port,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((err) => (err ? reject(err) : resolve())),
      ),
  };
}

export async function startWsServer(): Promise<{ port: number; close: () => Promise<void> }> {
  const wss = new WebSocketServer({ port: 0, host: "127.0.0.1" });
  wss.on("connection", (socket) => {
    socket.on("message", (data) => {
      socket.send(data);
    });
  });
  await new Promise<void>((resolve) => wss.on("listening", resolve));
  const port = (wss.address() as AddressInfo).port;
  return {
    port,
    close: () =>
      new Promise<void>((resolve, reject) =>
        wss.close((err) => (err ? reject(err) : resolve())),
      ),
  };
}
