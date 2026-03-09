import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { createWriteStream } from "node:fs";
import fs from "node:fs/promises";

const PID_PATH = ".ui-backend.pid";
const TRAFFIC_PID_PATH = ".ui-traffic.pid";
const BASE_PROXY_URL = process.env.PROXY_URL || "http://127.0.0.1:9900";
const BACKEND_URL =
  process.env.ADMIN_STATUS_URL ||
  `${BASE_PROXY_URL.replace(/\/$/, "")}/_bifrost/api/proxy/address`;

const getRepoRoot = () => {
  const current = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(current), "../../..");
};

const isBackendReady = async () => {
  try {
    const res = await fetch(BACKEND_URL);
    return res.ok;
  } catch {
    return false;
  }
};

const waitForBackend = async () => {
  for (let i = 0; i < 600; i += 1) {
    if (await isBackendReady()) return true;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
};

const isProcessAlive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const startTrafficGenerator = async (repoRoot: string) => {
  const trafficPidFile = path.join(repoRoot, TRAFFIC_PID_PATH);
  try {
    const pidText = await fs.readFile(trafficPidFile, "utf-8");
    const pid = Number(pidText);
    if (!Number.isNaN(pid) && isProcessAlive(pid)) {
      return;
    }
  } catch {
    void 0;
  }

  const generatorPath = path.join(repoRoot, "web", "tests", "ui", "traffic-generator.cjs");
  const child = spawn("node", [generatorPath], {
    cwd: path.join(repoRoot, "web"),
    env: {
      ...process.env,
      PROXY_URL: process.env.PROXY_URL || "http://127.0.0.1:9900",
    },
    stdio: "ignore",
    detached: true,
  });
  const pid = child.pid;
  if (!pid) {
    throw new Error("Failed to start traffic generator");
  }
  await fs.writeFile(trafficPidFile, String(pid));
};

export default async () => {
  const ready = await isBackendReady();
  const repoRoot = getRepoRoot();
  if (!ready) {
    const dataDir = path.join(repoRoot, ".bifrost-ui-test");
    const targetDir = path.join(repoRoot, ".bifrost-ui-target");
    const binPath = path.join(targetDir, "debug", "bifrost");
    const logPath = path.join(repoRoot, ".ui-backend.log");
    const logStream = createWriteStream(logPath, { flags: "a" });
    const { cmd, args } = await fs
      .access(binPath)
      .then(() => ({
        cmd: binPath,
        args: ["start", "-p", "9900", "--unsafe-ssl"],
      }))
      .catch(() => ({
        cmd: "cargo",
        args: [
          "run",
          "--bin",
          "bifrost",
          "--",
          "start",
          "-p",
          "9900",
          "--unsafe-ssl",
        ],
      }));
    const child = spawn(cmd, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
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
      throw new Error("Failed to start Bifrost backend process");
    }
    await fs.writeFile(path.join(repoRoot, PID_PATH), String(pid));

    const ok = await waitForBackend();
    if (!ok) {
      try {
        process.kill(-pid);
      } catch {
        try {
          process.kill(pid);
        } catch {
          return;
        }
      }
      throw new Error("Bifrost backend failed to start for UI tests");
    }
  }

  await startTrafficGenerator(repoRoot);
};
