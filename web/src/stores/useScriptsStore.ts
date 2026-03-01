import { create } from 'zustand';
import scriptsApi from '../api/scripts';
import type { ScriptDetail, ScriptInfo, ScriptType, ScriptExecutionResult } from '../api/scripts';

const REQUEST_SCRIPT_TEMPLATE = `// ============================================================
// Bifrost Request Script Template
// ============================================================
// This script runs BEFORE the request is sent to the upstream server.
// 
// Available objects:
//   - request: The HTTP request (modifiable: method, headers, body)
//   - ctx: Script context with metadata and configuration
//   - log/console: Logging interface
// ============================================================

// --- Logging Examples ---
log.info("Processing request:", request.method, request.url);
log.debug("Client IP:", request.clientIp);

// --- Read Request Properties (read-only) ---
// request.url       - Full URL (e.g., "https://api.example.com/users")
// request.host      - Host name (e.g., "api.example.com")
// request.path      - Path (e.g., "/users")
// request.protocol  - Protocol ("http" or "https")
// request.clientIp  - Client IP address
// request.clientApp - Client app identifier (may be null)

// --- Modify Request Method ---
// request.method = "POST";

// --- Modify Request Headers ---
// Add a custom header
request.headers["X-Custom-Header"] = "custom-value";

// Add authentication from context values
var apiToken = ctx.values["API_TOKEN"];
if (apiToken) {
  request.headers["Authorization"] = "Bearer " + apiToken;
  log.info("Added Authorization header");
}

// Add request tracing header
request.headers["X-Request-ID"] = ctx.requestId;

// --- Modify Request Body ---
// Example: Add timestamp to JSON body
if (request.body && request.headers["Content-Type"]?.includes("json")) {
  try {
    var bodyData = JSON.parse(request.body);
    bodyData._timestamp = Date.now();
    bodyData._requestId = ctx.requestId;
    request.body = JSON.stringify(bodyData);
    log.info("Modified request body");
  } catch (e) {
    log.error("Failed to parse request body:", e.message);
  }
}

// --- Access Context Information ---
log.debug("Script name:", ctx.scriptName);
log.debug("Script type:", ctx.scriptType);
log.debug("Request ID:", ctx.requestId);

// Access custom values from Bifrost configuration
// var myValue = ctx.values["MY_CONFIG_KEY"];

// Access matched rules
if (ctx.matchedRules.length > 0) {
  log.info("Matched rules:", ctx.matchedRules.length);
  ctx.matchedRules.forEach(function(rule) {
    log.debug("  Pattern:", rule.pattern, "->", rule.value);
  });
}
`;

const RESPONSE_SCRIPT_TEMPLATE = `// ============================================================
// Bifrost Response Script Template
// ============================================================
// This script runs AFTER receiving the response from upstream.
// 
// Available objects:
//   - response: The HTTP response (modifiable: status, statusText, headers, body)
//   - ctx: Script context with metadata and configuration
//   - log/console: Logging interface
// ============================================================

// --- Logging Examples ---
log.info("Processing response:", response.status, response.statusText);
log.debug("Original request:", response.request.method, response.request.url);

// --- Read Response Properties ---
// response.status     - HTTP status code (e.g., 200)
// response.statusText - Status text (e.g., "OK")
// response.headers    - Response headers
// response.body       - Response body content
// response.request    - Original request info (read-only)

// --- Modify Response Status ---
// response.status = 200;
// response.statusText = "OK";

// --- Modify Response Headers ---
// Add processing metadata
response.headers["X-Processed-By"] = "Bifrost";
response.headers["X-Request-ID"] = ctx.requestId;
response.headers["X-Processing-Time"] = String(Date.now());

// Remove sensitive headers
// delete response.headers["X-Internal-Debug"];

// --- Modify Response Body ---
// Example: Transform JSON response
var contentType = response.headers["Content-Type"] || "";
if (contentType.includes("application/json") && response.body) {
  try {
    var data = JSON.parse(response.body);
    
    // Add metadata to response
    data._meta = {
      processedAt: new Date().toISOString(),
      requestId: ctx.requestId,
      scriptName: ctx.scriptName
    };
    
    // Example: Filter sensitive fields
    // delete data.internalId;
    // delete data.secretKey;
    
    // Example: Transform data structure
    // if (data.items) {
    //   data.count = data.items.length;
    // }
    
    response.body = JSON.stringify(data, null, 2);
    log.info("Modified JSON response");
  } catch (e) {
    log.error("Failed to parse JSON response:", e.message);
  }
}

// --- Conditional Processing Based on Status ---
if (response.status >= 400) {
  log.warn("Error response:", response.status, response.statusText);
  
  // Example: Add error tracking header
  response.headers["X-Error-Logged"] = "true";
}

if (response.status === 404) {
  log.info("Resource not found:", response.request.path);
}

// --- Access Original Request Information ---
log.debug("Request URL:", response.request.url);
log.debug("Request Method:", response.request.method);
log.debug("Request Host:", response.request.host);
log.debug("Request Path:", response.request.path);
// response.request.headers - Original request headers

// --- Access Context Information ---
log.debug("Script name:", ctx.scriptName);
log.debug("Request ID:", ctx.requestId);

// Access custom values
// var debugMode = ctx.values["DEBUG_MODE"];
// if (debugMode === "true") {
//   response.headers["X-Debug-Info"] = JSON.stringify(ctx.matchedRules);
// }

// Access matched rules
if (ctx.matchedRules.length > 0) {
  log.info("Matched", ctx.matchedRules.length, "rules");
}
`;

interface ScriptsState {
  requestScripts: ScriptInfo[];
  responseScripts: ScriptInfo[];
  selectedScript: ScriptDetail | null;
  selectedType: ScriptType;
  loading: boolean;
  saving: boolean;
  testing: boolean;
  testResult: ScriptExecutionResult | null;
  error: string | null;

  fetchScripts: () => Promise<void>;
  selectScript: (type: ScriptType, name: string) => Promise<void>;
  saveScript: (type: ScriptType, name: string, content: string) => Promise<void>;
  deleteScript: (type: ScriptType, name: string) => Promise<void>;
  testScript: (type: ScriptType, content: string) => Promise<void>;
  createNewScript: (type: ScriptType) => void;
  clearSelection: () => void;
  clearTestResult: () => void;
}

export const useScriptsStore = create<ScriptsState>((set, get) => ({
  requestScripts: [],
  responseScripts: [],
  selectedScript: null,
  selectedType: 'request',
  loading: false,
  saving: false,
  testing: false,
  testResult: null,
  error: null,

  fetchScripts: async () => {
    set({ loading: true, error: null });
    try {
      const data = await scriptsApi.list();
      set({
        requestScripts: data.request,
        responseScripts: data.response,
        loading: false,
      });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  selectScript: async (type: ScriptType, name: string) => {
    set({ loading: true, error: null, selectedType: type });
    try {
      const script = await scriptsApi.get(type, name);
      set({ selectedScript: script, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  saveScript: async (type: ScriptType, name: string, content: string) => {
    set({ saving: true, error: null });
    try {
      const script = await scriptsApi.save(type, name, { content });
      set({ selectedScript: script, saving: false });
      await get().fetchScripts();
    } catch (e) {
      set({ error: String(e), saving: false });
    }
  },

  deleteScript: async (type: ScriptType, name: string) => {
    set({ loading: true, error: null });
    try {
      await scriptsApi.delete(type, name);
      set({ selectedScript: null, loading: false });
      await get().fetchScripts();
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  testScript: async (type: ScriptType, content: string) => {
    set({ testing: true, testResult: null, error: null });
    try {
      const result = await scriptsApi.test({ type, content });
      set({ testResult: result, testing: false });
    } catch (e) {
      set({ error: String(e), testing: false });
    }
  },

  createNewScript: (type: ScriptType) => {
    set({
      selectedType: type,
      selectedScript: {
        name: '',
        script_type: type,
        content: type === 'request'
          ? REQUEST_SCRIPT_TEMPLATE
          : RESPONSE_SCRIPT_TEMPLATE,
        created_at: Date.now(),
        updated_at: Date.now(),
      },
      testResult: null,
    });
  },

  clearSelection: () => {
    set({ selectedScript: null, testResult: null });
  },

  clearTestResult: () => {
    set({ testResult: null });
  },
}));
