import http from "node:http";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

const proxyPort = Number(process.env.BIFROST_PROXY_PORT || "9900");
const upstreamPort = Number(process.env.BIFROST_UPSTREAM_PORT || "18080");
const proxyUrl = `http://127.0.0.1:${proxyPort}`;
const apiBase = `${proxyUrl}/_bifrost/api`;

const durations = {
  baselineMs: Number(process.env.LOADTEST_BASELINE_MS || "15000"),
  warmupMs: Number(process.env.LOADTEST_WARMUP_MS || "15000"),
  steadyMs: Number(process.env.LOADTEST_STEADY_MS || "60000"),
  burstMs: Number(process.env.LOADTEST_BURST_MS || "15000"),
  cooldownMs: Number(process.env.LOADTEST_COOLDOWN_MS || "60000"),
};

const profile = {
  warmup: { small: 20, large: 4, sse: 8 },
  steady: { small: 50, large: 8, sse: 16 },
  burst: { small: 80, large: 12, sse: 24 },
};

const reportDir = path.join(repoRoot, ".artifacts", "loadtest");
const runId = new Date().toISOString().replaceAll(":", "-");
const outputPath = path.join(reportDir, `proxy-stability-${runId}.json`);
const dataDir = path.join(repoRoot, ".bifrost-stability", runId);

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function fetchJson(url, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 2000);
  const response = await fetch(url, {
    ...options,
    signal: controller.signal,
  });
  clearTimeout(timeout);
  if (!response.ok) {
    throw new Error(`Request failed: ${response.status} ${response.statusText}`);
  }
  return response.json();
}

function proxiedHttpRequest(targetUrl, { signal } = {}) {
  return new Promise((resolve, reject) => {
    const target = new URL(targetUrl);
    const req = http.request(
      {
        host: "127.0.0.1",
        port: proxyPort,
        method: "GET",
        path: targetUrl,
        headers: {
          Host: target.host,
          Connection: "keep-alive",
        },
        signal,
      },
      (res) => {
        let bytes = 0;
        res.on("data", (chunk) => {
          bytes += chunk.length;
        });
        res.on("end", () => {
          resolve({
            statusCode: res.statusCode || 0,
            bytes,
          });
        });
      },
    );
    req.setTimeout(5000, () => {
      req.destroy(new Error("request timeout"));
    });
    req.on("error", reject);
    req.end();
  });
}

function startUpstreamServer() {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url || "/", `http://127.0.0.1:${upstreamPort}`);

    if (url.pathname === "/small") {
      const body = JSON.stringify({
        ok: true,
        payload: "x".repeat(1024),
        ts: Date.now(),
      });
      res.writeHead(200, {
        "Content-Type": "application/json",
        "Content-Length": Buffer.byteLength(body),
      });
      res.end(body);
      return;
    }

    if (url.pathname === "/large") {
      const chunk = Buffer.alloc(16 * 1024, "L");
      const chunkCount = 32;
      res.writeHead(200, {
        "Content-Type": "application/octet-stream",
        "Transfer-Encoding": "chunked",
      });
      let sent = 0;
      const timer = setInterval(() => {
        if (sent >= chunkCount) {
          clearInterval(timer);
          res.end();
          return;
        }
        res.write(chunk);
        sent += 1;
      }, 5);
      req.on("close", () => clearInterval(timer));
      return;
    }

    if (url.pathname === "/sse") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      });
      let seq = 0;
      const timer = setInterval(() => {
        seq += 1;
        res.write(`id: ${seq}\ndata: ${"s".repeat(512)}\n\n`);
      }, 200);
      req.on("close", () => {
        clearInterval(timer);
        res.end();
      });
      return;
    }

    res.writeHead(404);
    res.end("not found");
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(upstreamPort, "127.0.0.1", () => {
      resolve(server);
    });
  });
}

function startBifrost(logPath) {
  return spawn(
    "./target/debug/bifrost",
    ["start", "--host", "127.0.0.1", "--port", String(proxyPort), "--skip-cert-check"],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        BIFROST_DATA_DIR: dataDir,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
}

async function waitForProxyReady(timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await fetchJson(`${apiBase}/system/overview`);
      return;
    } catch {
      await sleep(500);
    }
  }
  throw new Error("Timed out waiting for Bifrost admin API");
}

function createCounters() {
  return {
    started: 0,
    completed: 0,
    ok: 0,
    non2xx: 0,
    errors: 0,
    bytes: 0,
    latenciesMs: [],
  };
}

async function runLoop({ name, concurrency, makeUrl, counters, stopAt }) {
  const workers = Array.from({ length: concurrency }, async () => {
    while (Date.now() < stopAt) {
      counters.started += 1;
      const startedAt = Date.now();
      try {
        const result = await proxiedHttpRequest(makeUrl());
        const latency = Date.now() - startedAt;
        counters.completed += 1;
        counters.bytes += result.bytes;
        counters.latenciesMs.push(latency);
        if (result.statusCode >= 200 && result.statusCode < 300) {
          counters.ok += 1;
        } else {
          counters.non2xx += 1;
        }
      } catch (error) {
        counters.completed += 1;
        counters.errors += 1;
        counters.latenciesMs.push(Date.now() - startedAt);
      }
    }
  });
  await Promise.all(workers);
  return { name, concurrency, ...counters };
}

function startSseClients(concurrency, counters) {
  const requests = [];
  for (let i = 0; i < concurrency; i += 1) {
    const url = `http://127.0.0.1:${upstreamPort}/sse?client=${i}`;
    const target = new URL(url);
    counters.started += 1;
    const req = http.request(
      {
        host: "127.0.0.1",
        port: proxyPort,
        method: "GET",
        path: url,
        headers: {
          Host: target.host,
          Accept: "text/event-stream",
          Connection: "keep-alive",
        },
      },
      (res) => {
        if ((res.statusCode || 0) < 200 || (res.statusCode || 0) >= 300) {
          counters.non2xx += 1;
          counters.completed += 1;
          return;
        }
        counters.ok += 1;
        res.on("data", (chunk) => {
          counters.bytes += chunk.length;
        });
        res.on("end", () => {
          counters.completed += 1;
        });
      },
    );
    req.setTimeout(5000, () => {
      req.destroy(new Error("sse timeout"));
    });
    req.on("error", () => {
        counters.errors += 1;
        counters.completed += 1;
      });
    req.end();
    requests.push(req);
  }
  return {
    stop() {
      for (const req of requests) {
        req.destroy();
      }
    },
  };
}

function percentile(values, p) {
  if (!values.length) {
    return 0;
  }
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return sorted[index];
}

function memoryRawToMiB(rawValue, sample) {
  const total = sample?.memory?.process?.system_total_kib || 0;
  const divisor = total > 1024 * 1024 * 1024 ? 1024 * 1024 : 1024;
  return Number((rawValue / divisor).toFixed(2));
}

async function sampleOnce() {
  const [metrics, memory] = await Promise.all([
    fetchJson(`${apiBase}/metrics`),
    fetchJson(`${apiBase}/system/memory`),
  ]);
  return {
    ts: Date.now(),
    metrics,
    memory,
  };
}

function summarizeWindow(samples, name, startTs, endTs) {
  const windowSamples = samples.filter((sample) => sample.ts >= startTs && sample.ts <= endTs);
  const rssSeriesRaw = windowSamples.map((sample) => sample.memory.process.rss_kib);
  const rssSeriesMiB = windowSamples.map((sample) =>
    memoryRawToMiB(sample.memory.process.rss_kib, sample),
  );
  const cpuSeries = windowSamples.map((sample) =>
    Math.max(sample.memory.process.cpu_usage_percent || 0, sample.metrics.cpu_usage || 0),
  );
  const frameSeries = windowSamples.map(
    (sample) => sample.memory.connections.ws_monitor.total_frames_in_memory,
  );
  const recentCacheSeries = windowSamples.map(
    (sample) => sample.memory.traffic_db?.recent_cache?.len || 0,
  );

  return {
    name,
    sampleCount: windowSamples.length,
    rssMiB: {
      min: Math.min(...rssSeriesMiB, 0),
      max: Math.max(...rssSeriesMiB, 0),
      end: rssSeriesMiB.at(-1) || 0,
    },
    rssRaw: {
      min: Math.min(...rssSeriesRaw, 0),
      max: Math.max(...rssSeriesRaw, 0),
      end: rssSeriesRaw.at(-1) || 0,
    },
    cpuPercent: {
      min: Math.min(...cpuSeries, 0),
      max: Math.max(...cpuSeries, 0),
      end: cpuSeries.at(-1) || 0,
    },
    wsFramesInMemory: {
      max: Math.max(...frameSeries, 0),
      end: frameSeries.at(-1) || 0,
    },
    recentCacheLen: {
      max: Math.max(...recentCacheSeries, 0),
      end: recentCacheSeries.at(-1) || 0,
    },
  };
}

async function main() {
  await ensureDir(reportDir);
  await ensureDir(dataDir);

  const logPath = path.join(reportDir, `proxy-stability-${runId}.log`);
  const upstreamServer = await startUpstreamServer();
  const bifrost = startBifrost(logPath);

  const logChunks = [];
  bifrost.stdout?.on("data", (chunk) => logChunks.push(chunk));
  bifrost.stderr?.on("data", (chunk) => logChunks.push(chunk));

  try {
    await waitForProxyReady();

    const samples = [];
    let samplerStopped = false;
    const sampler = (async () => {
      while (!samplerStopped) {
        try {
          samples.push(await sampleOnce());
        } catch {
          // Keep sampling best-effort so a transient failure does not abort the run.
        }
        await sleep(1000);
      }
    })();

    const phaseResults = [];
    const phaseWindows = [];

    async function runIdlePhase(name, durationMs) {
      const startTs = Date.now();
      await sleep(durationMs);
      const endTs = Date.now();
      phaseWindows.push({ name, startTs, endTs });
    }

    async function runLoadPhase(name, durationMs, loadProfile) {
      const startTs = Date.now();
      const stopAt = startTs + durationMs;
      const smallCounters = createCounters();
      const largeCounters = createCounters();
      const sseCounters = createCounters();

      const sse = startSseClients(loadProfile.sse, sseCounters);
      const [small, large] = await Promise.all([
        runLoop({
          name: `${name}-small`,
          concurrency: loadProfile.small,
          makeUrl: () => `http://127.0.0.1:${upstreamPort}/small?ts=${Date.now()}`,
          counters: smallCounters,
          stopAt,
        }),
        runLoop({
          name: `${name}-large`,
          concurrency: loadProfile.large,
          makeUrl: () => `http://127.0.0.1:${upstreamPort}/large?ts=${Date.now()}`,
          counters: largeCounters,
          stopAt,
        }),
      ]);
      sse.stop();
      await sleep(1000);

      const endTs = Date.now();
      phaseWindows.push({ name, startTs, endTs });
      phaseResults.push({
        name,
        durationMs,
        small: {
          ...small,
          p50Ms: percentile(small.latenciesMs, 50),
          p95Ms: percentile(small.latenciesMs, 95),
          p99Ms: percentile(small.latenciesMs, 99),
        },
        large: {
          ...large,
          p50Ms: percentile(large.latenciesMs, 50),
          p95Ms: percentile(large.latenciesMs, 95),
          p99Ms: percentile(large.latenciesMs, 99),
        },
        sse: {
          ...sseCounters,
        },
      });
    }

    await runIdlePhase("baseline", durations.baselineMs);
    await runLoadPhase("warmup", durations.warmupMs, profile.warmup);
    await runLoadPhase("steady", durations.steadyMs, profile.steady);
    await runLoadPhase("burst", durations.burstMs, profile.burst);
    await runIdlePhase("cooldown", durations.cooldownMs);

    samplerStopped = true;
    await sampler;

    const summaries = phaseWindows.map((window) =>
      summarizeWindow(samples, window.name, window.startTs, window.endTs),
    );

    const baselineSummary = summaries.find((summary) => summary.name === "baseline");
    const cooldownSummary = summaries.find((summary) => summary.name === "cooldown");
    const peakRssMiB = Math.max(
      ...samples.map((sample) => memoryRawToMiB(sample.memory.process.rss_kib, sample)),
      0,
    );
    const peakCpuPercent = Math.max(
      ...samples.map((sample) =>
        Math.max(sample.memory.process.cpu_usage_percent || 0, sample.metrics.cpu_usage || 0),
      ),
      0,
    );

    const report = {
      runId,
      proxyPort,
      upstreamPort,
      durations,
      profile,
      outputPath,
      phaseResults,
      phaseSummaries: summaries,
      samples,
      analysis: {
        baselineRssMiB: baselineSummary?.rssMiB.end || 0,
        peakRssMiB,
        cooldownRssMiB: cooldownSummary?.rssMiB.end || 0,
        peakCpuPercent,
        cooldownRecentCacheLen: cooldownSummary?.recentCacheLen.end || 0,
        cooldownWsFramesInMemory: cooldownSummary?.wsFramesInMemory.end || 0,
      },
    };

    await fs.writeFile(outputPath, JSON.stringify(report, null, 2));
    await fs.writeFile(logPath, Buffer.concat(logChunks));

    console.log(JSON.stringify(report.analysis, null, 2));
    console.log(`report: ${outputPath}`);
  } finally {
    upstreamServer.closeAllConnections?.();
    upstreamServer.closeIdleConnections?.();
    upstreamServer.close();
    if (!bifrost.killed) {
      bifrost.kill("SIGINT");
      await Promise.race([
        new Promise((resolve) => bifrost.once("exit", resolve)),
        sleep(5000).then(() => {
          if (!bifrost.killed) {
            bifrost.kill("SIGKILL");
          }
        }),
      ]);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
