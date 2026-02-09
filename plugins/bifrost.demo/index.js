const http = require('http');

const PLUGIN_NAME = 'bifrost.demo';
const PORT = process.env.BIFROST_PLUGIN_PORT || 18000;

let requestCount = 0;

const handlers = {
  http: async (ctx) => {
    requestCount++;
    const timestamp = new Date().toISOString();
    
    ctx.headers = ctx.headers || {};
    ctx.headers['X-Demo-Plugin'] = PLUGIN_NAME;
    ctx.headers['X-Demo-Request-Count'] = String(requestCount);
    ctx.headers['X-Demo-Timestamp'] = timestamp;
    
    console.log(`[${PLUGIN_NAME}] #${requestCount} ${ctx.method} ${ctx.url}`);
    
    return { modified: true, ctx };
  },

  reqRead: async (ctx) => {
    if (ctx.data && ctx.data.length > 0) {
      console.log(`[${PLUGIN_NAME}] Request data: ${ctx.data.length} bytes`);
    }
    return { modified: false, ctx };
  },

  resRead: async (ctx) => {
    if (ctx.data && ctx.data.length > 0) {
      console.log(`[${PLUGIN_NAME}] Response data: ${ctx.data.length} bytes`);
    }
    return { modified: false, ctx };
  },

  reqRules: async (ctx) => {
    const rules = [];
    
    if (ctx.url && ctx.url.includes('/mock/')) {
      rules.push('statusCode://200');
      rules.push('replaceStatus://200');
      console.log(`[${PLUGIN_NAME}] Added mock rules for ${ctx.url}`);
    }
    
    return { rules, ctx };
  },

  resRules: async (ctx) => {
    const rules = [];
    return { rules, ctx };
  },

  reqStats: async (ctx) => {
    console.log(`[${PLUGIN_NAME}] Request stats: ${ctx.bytesTransferred} bytes, ${ctx.durationMs}ms`);
    return { ctx };
  },

  resStats: async (ctx) => {
    console.log(`[${PLUGIN_NAME}] Response stats: ${ctx.bytesTransferred} bytes, ${ctx.durationMs}ms`);
    return { ctx };
  },
};

const server = http.createServer(async (req, res) => {
  if (req.method !== 'POST') {
    res.writeHead(405);
    res.end('Method Not Allowed');
    return;
  }

  const url = new URL(req.url, `http://localhost:${PORT}`);
  const hook = url.pathname.slice(1);

  if (!handlers[hook]) {
    res.writeHead(404);
    res.end(JSON.stringify({ error: `Unknown hook: ${hook}` }));
    return;
  }

  let body = '';
  for await (const chunk of req) {
    body += chunk;
  }

  try {
    const ctx = JSON.parse(body);
    const result = await handlers[hook](ctx);
    
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(result));
  } catch (err) {
    console.error(`[${PLUGIN_NAME}] Error handling ${hook}:`, err);
    res.writeHead(500);
    res.end(JSON.stringify({ error: err.message }));
  }
});

server.listen(PORT, () => {
  console.log(`[${PLUGIN_NAME}] Plugin server started on port ${PORT}`);
  console.log(`[${PLUGIN_NAME}] Registered hooks: ${Object.keys(handlers).join(', ')}`);
});

process.on('SIGINT', () => {
  console.log(`\n[${PLUGIN_NAME}] Shutting down...`);
  server.close(() => {
    console.log(`[${PLUGIN_NAME}] Total requests processed: ${requestCount}`);
    process.exit(0);
  });
});
