import fs from "node:fs/promises";

export default async () => {
  const trafficPidFile = process.env.BIFROST_UI_TEST_TRAFFIC_PID_FILE;
  try {
    if (!trafficPidFile) {
      throw new Error("missing traffic pid file");
    }
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
  } catch {
    void 0;
  }
  const pidFile = process.env.BIFROST_UI_TEST_PID_FILE;
  try {
    if (!pidFile) {
      throw new Error("missing backend pid file");
    }
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
