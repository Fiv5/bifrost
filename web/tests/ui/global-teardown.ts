import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs/promises";

const PID_PATH = ".ui-backend.pid";
const TRAFFIC_PID_PATH = ".ui-traffic.pid";

const getRepoRoot = () => {
  const current = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(current), "../../..");
};

export default async () => {
  const repoRoot = getRepoRoot();
  const trafficPidFile = path.join(repoRoot, TRAFFIC_PID_PATH);
  try {
    const pidText = await fs.readFile(trafficPidFile, "utf-8");
    const pid = Number(pidText);
    if (!Number.isNaN(pid)) {
      try {
        process.kill(-pid);
      } catch {
        try {
          process.kill(pid);
        } catch {
          return;
        }
      }
    }
    await fs.unlink(trafficPidFile);
  } catch {}
  const pidFile = path.join(repoRoot, PID_PATH);
  try {
    const pidText = await fs.readFile(pidFile, "utf-8");
    const pid = Number(pidText);
    if (!Number.isNaN(pid)) {
      try {
        process.kill(-pid);
      } catch {
        try {
          process.kill(pid);
        } catch {
          return;
        }
      }
    }
    await fs.unlink(pidFile);
  } catch {
    return;
  }
};
