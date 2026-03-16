import { test, expect, type APIRequestContext, type Page } from "@playwright/test";
import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import fs from "node:fs/promises";
import { createServer as createTcpServer, Socket as NetSocket } from "node:net";
import type { AddressInfo } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  apiBase,
  backendPort,
  clearTraffic,
  sendProxyRequest,
  startMockHttpServer,
  uniqueName,
} from "./helpers/admin-helpers";

const BASE_PROXY_URL = process.env.PROXY_URL || `http://127.0.0.1:${backendPort}`;

const getRepoRoot = () => {
  const current = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(current), "../../..");
};

const isProcessAlive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const isBackendReady = async () => {
  try {
    const res = await fetch(`${apiBase}/proxy/address`);
    return res.ok;
  } catch {
    return false;
  }
};

const waitForBackend = async () => {
  for (let i = 0; i < 240; i += 1) {
    if (await isBackendReady()) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
};

const stopTrackedBackend = async () => {
  const pidFile =
    process.env.BIFROST_UI_TEST_PID_FILE || path.join(getRepoRoot(), ".ui-backend.pid");
  try {
    const pidText = await fs.readFile(pidFile, "utf-8");
    const pid = Number(pidText);
    if (Number.isNaN(pid) || !isProcessAlive(pid)) {
      await fs.rm(pidFile, { force: true });
      return;
    }
    try {
      process.kill(-pid);
    } catch {
      process.kill(pid);
    }
    await fs.rm(pidFile, { force: true });
  } catch {
    void 0;
  }
};

const startTrackedBackend = async () => {
  const repoRoot = getRepoRoot();
  const pidFile =
    process.env.BIFROST_UI_TEST_PID_FILE || path.join(repoRoot, ".ui-backend.pid");
  const dataDir = process.env.BIFROST_DATA_DIR || path.join(repoRoot, ".bifrost-ui-test");
  const targetDir =
    process.env.BIFROST_UI_TEST_TARGET_DIR || path.join(repoRoot, ".bifrost-ui-target");
  const binPath = path.join(targetDir, "debug", "bifrost");
  const logPath =
    process.env.BIFROST_UI_TEST_LOG_FILE || path.join(repoRoot, ".ui-backend.log");
  const logStream = createWriteStream(logPath, { flags: "a" });
  const { cmd, args } = await fs
    .access(binPath)
    .then(() => ({
      cmd: binPath,
      args: [
        "start",
        "--host",
        "127.0.0.1",
        "-p",
        String(backendPort),
        "--unsafe-ssl",
        "--access-mode",
        "allow_all",
      ],
    }))
    .catch(() => ({
      cmd: "cargo",
      args: [
        "run",
        "--bin",
        "bifrost",
        "--",
        "start",
        "--host",
        "127.0.0.1",
        "-p",
        String(backendPort),
        "--unsafe-ssl",
        "--access-mode",
        "allow_all",
      ],
    }));

  const child = spawn(cmd, args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      PROXY_URL: BASE_PROXY_URL,
      BIFROST_UI_TEST_PORT: String(backendPort),
      BIFROST_DATA_DIR: dataDir,
      CARGO_TARGET_DIR: targetDir,
    },
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  child.stdout?.pipe(logStream);
  child.stderr?.pipe(logStream);
  const pid = child.pid;
  if (!pid) {
    throw new Error("Failed to start tracked backend");
  }
  await fs.writeFile(pidFile, String(pid));
  const ok = await waitForBackend();
  if (!ok) {
    throw new Error("Tracked backend failed to start");
  }
};

async function fetchTrafficList(request: APIRequestContext) {
  const response = await request.get(`${apiBase}/traffic?limit=100`);
  return (await response.json()) as { records?: Array<Record<string, unknown>> };
}

async function fetchTrafficDetail(request: APIRequestContext, id: string) {
  const response = await request.get(`${apiBase}/traffic/${id}`);
  return (await response.json()) as {
    id: string;
    response_size: number;
    socket_status?: { is_open?: boolean };
  };
}

async function waitForTrafficRecordId(
  request: APIRequestContext,
  predicate: (record: Record<string, unknown>) => boolean,
) {
  let foundId: string | null = null;
  await expect
    .poll(
      async () => {
        const payload = await fetchTrafficList(request);
        const record = payload.records?.find(predicate);
        foundId = typeof record?.id === "string" ? record.id : null;
        return foundId;
      },
      { timeout: 15000 },
    )
    .toMatch(/^REQ-/);
  return foundId as string;
}

async function startTunnelEchoServer() {
  const sockets = new Set<NetSocket>();
  const server = createTcpServer((socket) => {
    sockets.add(socket);
    socket.on("data", (chunk) => {
      socket.write(chunk);
    });
    socket.on("close", () => sockets.delete(socket));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;
  return {
    port,
    close: async () => {
      for (const socket of sockets) {
        socket.destroy();
      }
      await new Promise<void>((resolve, reject) =>
        server.close((err) => (err ? reject(err) : resolve())),
      );
    },
  };
}

async function waitForConnectEstablished(socket: NetSocket) {
  return new Promise<void>((resolve, reject) => {
    let buffer = "";
    const cleanup = () => {
      socket.off("data", onData);
      socket.off("error", onError);
      socket.off("close", onClose);
    };
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const onClose = () => {
      cleanup();
      reject(new Error("CONNECT tunnel closed before establishment"));
    };
    const onData = (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      if (!buffer.includes("\r\n\r\n")) {
        return;
      }
      cleanup();
      if (!buffer.startsWith("HTTP/1.1 200") && !buffer.startsWith("HTTP/1.0 200")) {
        reject(new Error(`Unexpected CONNECT response: ${buffer}`));
        return;
      }
      resolve();
    };
    socket.on("data", onData);
    socket.on("error", onError);
    socket.on("close", onClose);
  });
}

async function openConnectTunnel(port: number) {
  const socket = new NetSocket();
  await new Promise<void>((resolve, reject) => {
    socket.connect(backendPort, "127.0.0.1", () => resolve());
    socket.once("error", reject);
  });
  socket.write(
    `CONNECT 127.0.0.1:${port} HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nProxy-Connection: Keep-Alive\r\n\r\n`,
  );
  await waitForConnectEstablished(socket);

  socket.write("hello-through-tunnel");
  await new Promise<void>((resolve, reject) => {
    const onData = (chunk: Buffer) => {
      if (chunk.toString("utf8").includes("hello-through-tunnel")) {
        cleanup();
        resolve();
      }
    };
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const cleanup = () => {
      socket.off("data", onData);
      socket.off("error", onError);
    };
    socket.on("data", onData);
    socket.on("error", onError);
  });

  return {
    socket,
    close: async () => {
      if (socket.destroyed) {
        return;
      }
      await new Promise<void>((resolve) => {
        socket.once("close", () => resolve());
        socket.destroy();
      });
    },
  };
}

async function openTrafficPageAndWaitForPush(page: Page) {
  const pushSocketPromise = page.waitForEvent("websocket", (ws) =>
    ws.url().includes("/api/push"),
  );
  await page.goto("/_bifrost/traffic");
  await expect(page.getByTestId("traffic-table")).toBeVisible();
  await pushSocketPromise;
}

test.describe.serial("traffic push regressions", () => {
  test("服务重启后保留数据并恢复实时推送", async ({ page, context, request }) => {
    test.setTimeout(180000);
    await clearTraffic(request);
    const server = await startMockHttpServer();
    const persistedPath = `/${uniqueName("restart-persisted")}`;
    const livePath = `/${uniqueName("restart-live")}`;

    try {
      await sendProxyRequest(`http://127.0.0.1:${server.port}${persistedPath}`);

      await openTrafficPageAndWaitForPush(page);
      await expect(
        page.getByTestId("traffic-row").filter({ hasText: persistedPath }).first(),
      ).toBeVisible();

      await stopTrackedBackend();
      await startTrackedBackend();

      const freshPage = await context.newPage();
      try {
        await openTrafficPageAndWaitForPush(freshPage);
        await expect(
          freshPage.getByTestId("traffic-row").filter({ hasText: persistedPath }).first(),
        ).toBeVisible();

        await sendProxyRequest(`http://127.0.0.1:${server.port}${livePath}`);
        await expect(
          page.getByTestId("traffic-row").filter({ hasText: livePath }).first(),
        ).toBeVisible();
      } finally {
        await freshPage.close();
      }
    } finally {
      await server.close();
    }
  });

  test("多个页面同时打开时都能完整收到实时流量", async ({ page, context, request }) => {
    await clearTraffic(request);
    const server = await startMockHttpServer();
    const requestPath = `/${uniqueName("multi-page")}`;

    try {
      const secondPage = await context.newPage();
      try {
        await Promise.all([
          openTrafficPageAndWaitForPush(page),
          openTrafficPageAndWaitForPush(secondPage),
        ]);

        await sendProxyRequest(`http://127.0.0.1:${server.port}${requestPath}`);

        await expect(
          page.getByTestId("traffic-row").filter({ hasText: requestPath }).first(),
        ).toBeVisible();
        await expect(
          secondPage.getByTestId("traffic-row").filter({ hasText: requestPath }).first(),
        ).toBeVisible();

        await secondPage.reload();
        await expect(
          secondPage.getByTestId("traffic-row").filter({ hasText: requestPath }).first(),
        ).toBeVisible();
      } finally {
        await secondPage.close();
      }
    } finally {
      await server.close();
    }
  });

  test("CONNECT 长连接状态更新能推送到页面并正确落库", async ({ page, request }) => {
    await clearTraffic(request);
    const server = await startTunnelEchoServer();
    let serverClosed = false;
    let tunnel: Awaited<ReturnType<typeof openConnectTunnel>> | null = null;

    try {
      await openTrafficPageAndWaitForPush(page);

      tunnel = await openConnectTunnel(server.port);

      const connectId = await waitForTrafficRecordId(
        request,
        (item) => item.m === "CONNECT" && item.h === "127.0.0.1",
      );
      const row = page.locator(`[data-testid="traffic-row"][data-record-id="${connectId}"]`);
      await expect(row).toBeVisible();

      await expect
        .poll(async () => {
          const detail = await fetchTrafficDetail(request, connectId);
          return detail.socket_status?.is_open === true;
        })
        .toBe(true);

      await tunnel.close();
      tunnel = null;
      await server.close();
      serverClosed = true;

      await expect
        .poll(async () => {
          const detail = await fetchTrafficDetail(request, connectId);
          return detail.socket_status?.is_open === false;
        })
        .toBe(true);

      await expect
        .poll(async () => {
          const responseSize = await row.getAttribute("data-response-size");
          return Number(responseSize || "0");
        })
        .toBeGreaterThan(0);
    } finally {
      if (tunnel) {
        await tunnel.close();
      }
      if (!serverClosed) {
        await server.close();
      }
    }
  });
});
