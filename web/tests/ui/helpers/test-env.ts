import fs from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

export interface UiTestEnv {
  runId: string;
  backendPort: number;
  webPort: number;
  repoRoot: string;
  runtimeDir: string;
  dataDir: string;
  targetDir: string;
  backendPidFile: string;
  trafficPidFile: string;
  backendLogPath: string;
  proxyUrl: string;
  adminApiBase: string;
}

const getRepoRoot = () => {
  const current = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(current), "../../../..");
};

async function findFreePort(start: number, end: number): Promise<number> {
  for (let port = start; port <= end; port += 1) {
    const available = await new Promise<boolean>((resolve) => {
      const server = net.createServer();
      server.once("error", () => resolve(false));
      server.listen(port, "127.0.0.1", () => {
        server.close(() => resolve(true));
      });
    });
    if (available) {
      return port;
    }
  }
  throw new Error(`No free port found in range ${start}-${end}`);
}

function getExistingEnv(): UiTestEnv | null {
  const runId = process.env.BIFROST_UI_TEST_RUN_ID;
  const backendPort = Number(process.env.BIFROST_UI_TEST_PORT);
  const webPort = Number(process.env.WEB_PORT);
  const repoRoot = process.env.BIFROST_UI_TEST_REPO_ROOT;
  const runtimeDir = process.env.BIFROST_UI_TEST_RUNTIME_DIR;
  const dataDir = process.env.BIFROST_DATA_DIR;
  const targetDir = process.env.BIFROST_UI_TEST_TARGET_DIR;
  const backendPidFile = process.env.BIFROST_UI_TEST_PID_FILE;
  const trafficPidFile = process.env.BIFROST_UI_TEST_TRAFFIC_PID_FILE;
  const backendLogPath = process.env.BIFROST_UI_TEST_LOG_FILE;

  if (
    !runId ||
    !backendPort ||
    !webPort ||
    !repoRoot ||
    !runtimeDir ||
    !dataDir ||
    !targetDir ||
    !backendPidFile ||
    !trafficPidFile ||
    !backendLogPath
  ) {
    return null;
  }

  return {
    runId,
    backendPort,
    webPort,
    repoRoot,
    runtimeDir,
    dataDir,
    targetDir,
    backendPidFile,
    trafficPidFile,
    backendLogPath,
    proxyUrl: process.env.PROXY_URL || `http://127.0.0.1:${backendPort}`,
    adminApiBase:
      process.env.ADMIN_API_BASE || `http://127.0.0.1:${backendPort}/_bifrost/api`,
  };
}

export async function allocateUiTestEnv(): Promise<UiTestEnv> {
  const existing = getExistingEnv();
  if (existing) {
    return existing;
  }

  const repoRoot = getRepoRoot();
  const runId = `ui-${Date.now()}-${process.pid}-${Math.random().toString(16).slice(2, 8)}`;
  const backendPort = await findFreePort(39100, 39599);
  const webPort = await findFreePort(3010, 3099);
  const runtimeDir = path.join(repoRoot, ".bifrost-ui-test-runs", runId);
  const dataDir = path.join(runtimeDir, "data");
  const targetDir = path.join(repoRoot, ".bifrost-ui-target");
  const backendPidFile = path.join(runtimeDir, "backend.pid");
  const trafficPidFile = path.join(runtimeDir, "traffic.pid");
  const backendLogPath = path.join(runtimeDir, "backend.log");
  const proxyUrl = `http://127.0.0.1:${backendPort}`;
  const adminApiBase = `${proxyUrl}/_bifrost/api`;

  await fs.mkdir(runtimeDir, { recursive: true });
  await fs.mkdir(dataDir, { recursive: true });

  process.env.BIFROST_UI_TEST_RUN_ID = runId;
  process.env.BIFROST_UI_TEST_PORT = String(backendPort);
  process.env.BACKEND_PORT = String(backendPort);
  process.env.WEB_PORT = String(webPort);
  process.env.PROXY_URL = proxyUrl;
  process.env.ADMIN_API_BASE = adminApiBase;
  process.env.BIFROST_DATA_DIR = dataDir;
  process.env.BIFROST_UI_TEST_REPO_ROOT = repoRoot;
  process.env.BIFROST_UI_TEST_RUNTIME_DIR = runtimeDir;
  process.env.BIFROST_UI_TEST_TARGET_DIR = targetDir;
  process.env.BIFROST_UI_TEST_PID_FILE = backendPidFile;
  process.env.BIFROST_UI_TEST_TRAFFIC_PID_FILE = trafficPidFile;
  process.env.BIFROST_UI_TEST_LOG_FILE = backendLogPath;

  return {
    runId,
    backendPort,
    webPort,
    repoRoot,
    runtimeDir,
    dataDir,
    targetDir,
    backendPidFile,
    trafficPidFile,
    backendLogPath,
    proxyUrl,
    adminApiBase,
  };
}
