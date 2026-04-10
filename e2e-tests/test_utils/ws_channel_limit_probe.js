#!/usr/bin/env node

const path = require("node:path");

const wsModulePath = path.join(process.cwd(), "web", "node_modules", "ws");
const { WebSocket } = require(wsModulePath);

const url = process.argv[2];
const count = Number(process.argv[3] || "4");
const waitMs = Number(process.argv[4] || "3000");

if (!url) {
  console.error("usage: ws_channel_limit_probe.js <ws-url> [count] [wait-ms]");
  process.exit(2);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function openClient(name) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`${url}?x_client_id=${encodeURIComponent(name)}`);
    const messages = [];
    const errors = [];
    let connectedResolve = null;
    const connectedPromise = new Promise((r) => {
      connectedResolve = r;
    });

    ws.on("open", () => {
      ws.send(JSON.stringify({ need_overview: true }));
    });

    ws.on("message", (buf) => {
      const text = buf.toString();
      messages.push(text);
      if (text.includes('"type":"connected"') && connectedResolve) {
        connectedResolve();
        connectedResolve = null;
      }
    });

    ws.on("error", (err) => {
      errors.push(err.message);
      if (connectedResolve) {
        connectedResolve();
        connectedResolve = null;
      }
    });

    ws.on("unexpected-response", (_req, res) => {
      errors.push(`unexpected-response:${res.statusCode}`);
      if (connectedResolve) {
        connectedResolve();
        connectedResolve = null;
      }
    });

    ws.on("close", (code, reason) => {
      if (reason && reason.length > 0) {
        messages.push(
          JSON.stringify({
            type: "close",
            data: { code, reason: reason.toString() },
          }),
        );
      }
      if (connectedResolve) {
        connectedResolve();
        connectedResolve = null;
      }
    });

    resolve({ name, ws, messages, errors, connectedPromise });
  });
}

(async () => {
  const clients = [];
  for (let i = 1; i <= count; i += 1) {
    const client = await openClient(
      `e2e_chan_${i}_${process.pid}_${Date.now()}`,
    );
    await Promise.race([client.connectedPromise, sleep(3000)]);
    clients.push(client);
    await sleep(300);
  }

  await sleep(waitMs);

  const anyDisconnect = clients.some((c) =>
    c.messages.some((msg) => msg.includes('"type":"disconnect"')),
  );
  const oldest = clients[0];
  const result = {
    oldest_disconnect: anyDisconnect,
    oldest_messages: oldest.messages,
    oldest_errors: oldest.errors,
    total_clients: clients.length,
  };

  for (const client of clients) {
    client.ws.close();
  }

  console.log(JSON.stringify(result));
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
