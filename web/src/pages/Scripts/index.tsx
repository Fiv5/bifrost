import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Tree,
  Button,
  Space,
  Typography,
  Spin,
  message,
  Modal,
  Input,
  Tag,
  Empty,
  Dropdown,
  theme,
  Tooltip,
  Switch,
  Form,
} from "antd";
import type { MenuProps, TreeDataNode } from "antd";
import {
  FileOutlined,
  FolderOutlined,
  FolderOpenOutlined,
  PlusOutlined,
  SaveOutlined,
  DeleteOutlined,
  PlayCircleOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  SearchOutlined,
  UpOutlined,
  DownOutlined,
  ExportOutlined,
  MoreOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import Editor from "@monaco-editor/react";
import { useScriptsStore } from "../../stores/useScriptsStore";
import type {
  ScriptType,
  ScriptLogEntry,
  ScriptInfo,
  ScriptExecutionResult,
} from "../../api/scripts";
import { useThemeStore } from "../../stores/useThemeStore";
import SplitPane from "../../components/SplitPane";
import VerticalSplitPane from "../../components/VerticalSplitPane";
import { ImportBifrostButton } from "../../components/ImportBifrostButton";
import { useExportBifrost } from "../../hooks/useExportBifrost";
import { getSandboxConfig, updateSandboxConfig } from "../../api/config";

const { Text } = Typography;

const BIFROST_TYPES_DECODE = `
/**
 * Bifrost Decode Script Types
 *
 * Decode scripts are executed BEFORE body is stored and pushed.
 * - ctx.phase === "request"  : request body decode (response is null)
 * - ctx.phase === "response" : response body decode (response.request carries request snapshot)
 */

interface BifrostDecodeRequest {
  readonly url: string;
  readonly host: string;
  readonly path: string;
  readonly protocol: string;
  readonly clientIp: string;
  readonly clientApp: string | null;
  readonly method: string;
  readonly headers: Record<string, string>;

  /** UTF-8 preview (may be truncated) */
  readonly body: string;
  /** Hex preview (may be truncated) */
  readonly bodyHex: string;
  /** Original byte length */
  readonly bodySize: number;
  readonly bodyHexTruncated: boolean;
  readonly bodyTextTruncated: boolean;
}

interface BifrostDecodeResponse {
  readonly status: number;
  readonly statusText: string;
  readonly headers: Record<string, string>;
  readonly body: string;
  readonly bodyHex: string;
  readonly bodySize: number;
  readonly bodyHexTruncated: boolean;
  readonly bodyTextTruncated: boolean;
  readonly request: {
    url: string;
    method: string;
    host: string;
    path: string;
    protocol: string;
    clientIp: string;
    clientApp: string | null;
    headers: Record<string, string>;
  };
}

interface BifrostDecodeOutput {
  data: string;
  code: string;
  msg: string;
}

interface BifrostContext {
  readonly requestId: string;
  readonly scriptName: string;
  readonly scriptType: "request" | "response" | "decode";
  readonly phase?: "request" | "response";
  output?: BifrostDecodeOutput;
  readonly values: Record<string, string>;
  readonly matchedRules: Array<{ pattern: string; protocol: string; value: string }>;
}

interface BifrostLog {
  log(...args: any[]): void;
  debug(...args: any[]): void;
  info(...args: any[]): void;
  warn(...args: any[]): void;
  error(...args: any[]): void;
}

interface BifrostFile {
  readonly enabled: boolean;
  readText(path: string): string;
  writeText(path: string, content: string): boolean;
  appendText(path: string, content: string): boolean;
  exists(path: string): boolean;
  remove(path: string): boolean;
  listDir(path?: string): string[];
}

interface BifrostNet {
  readonly enabled: boolean;
  fetch(url: string, optionsJson?: string): string;
  request(url: string, optionsJson?: string): string;
}

declare const request: BifrostDecodeRequest;
declare const response: BifrostDecodeResponse | null;
declare const ctx: BifrostContext;
declare let output: BifrostDecodeOutput | undefined;
declare const log: BifrostLog;
declare const console: BifrostLog;
declare const file: BifrostFile;
declare const net: BifrostNet;
`;

const BIFROST_TYPES_REQUEST = `
/**
 * Bifrost Request Script Types
 * 
 * Request scripts are executed BEFORE the request is sent to the upstream server.
 * You can modify: method, headers, body
 * Read-only properties: url, host, path, protocol, clientIp, clientApp
 */

/** HTTP Request object - available in request scripts */
interface BifrostRequest {
  /** Full request URL (read-only) */
  readonly url: string;
  /** Host name from the request (read-only) */
  readonly host: string;
  /** Request path (read-only) */
  readonly path: string;
  /** Protocol: "http" or "https" (read-only) */
  readonly protocol: string;
  /** Client IP address (read-only) */
  readonly clientIp: string;
  /** Client application identifier, if available (read-only) */
  readonly clientApp: string | null;
  /** HTTP method (GET, POST, PUT, DELETE, etc.) - modifiable */
  method: string;
  /** Request headers as key-value pairs - modifiable */
  headers: Record<string, string>;
  /** Request body content - modifiable */
  body: string | null;
}

/** Script execution context - provides metadata and configuration */
interface BifrostContext {
  /** Unique identifier for this request */
  readonly requestId: string;
  /** Name of the current script */
  readonly scriptName: string;
  /** Type of script: "request" | "response" | "decode" */
  readonly scriptType: "request" | "response" | "decode";
  /** Current phase for decode: "request" | "response" */
  readonly phase?: "request" | "response";
  /** Custom key-value configuration from Bifrost settings */
  readonly values: Record<string, string>;
  /** List of rules that matched this request */
  readonly matchedRules: Array<{
    /** Rule pattern (e.g., "*.example.com") */
    pattern: string;
    /** Protocol (http/https) */
    protocol: string;
    /** Rule value/target */
    value: string;
  }>;
}

/** Logging interface - logs are captured and displayed in test results */
interface BifrostLog {
  /** Log a message (alias for info) */
  log(...args: any[]): void;
  /** Log debug level message */
  debug(...args: any[]): void;
  /** Log info level message */
  info(...args: any[]): void;
  /** Log warning level message */
  warn(...args: any[]): void;
  /** Log error level message */
  error(...args: any[]): void;
}

/** Sandbox file API (path is relative to scripts/_sandbox) */
interface BifrostFile {
  /** Whether file APIs are enabled */
  readonly enabled: boolean;
  readText(path: string): string;
  writeText(path: string, content: string): boolean;
  appendText(path: string, content: string): boolean;
  exists(path: string): boolean;
  remove(path: string): boolean;
  listDir(path?: string): string[];
}

/** Network request API (returns JSON string, use JSON.parse) */
interface BifrostNet {
  /** Whether net APIs are enabled */
  readonly enabled: boolean;
  /**
   * net.fetch(url, optionsJson?) -> JSON string
   * optionsJson example: {"method":"POST","headers":{"Content-Type":"application/json"},"body":"...","timeoutMs":3000}
   */
  fetch(url: string, optionsJson?: string): string;
  request(url: string, optionsJson?: string): string;
}

/** The request object to inspect and modify */
declare const request: BifrostRequest;
/** Script execution context with metadata */
declare const ctx: BifrostContext;
/** Logging interface */
declare const log: BifrostLog;
/** Console logging (alias for log) */
declare const console: BifrostLog;
/** File API */
declare const file: BifrostFile;
/** Network API */
declare const net: BifrostNet;
`;

const BIFROST_TYPES_RESPONSE = `
/**
 * Bifrost Response Script Types
 * 
 * Response scripts are executed AFTER receiving the response from upstream.
 * You can modify: status, statusText, headers, body
 * Read-only: request (original request information)
 */

/** HTTP Response object - available in response scripts */
interface BifrostResponse {
  /** HTTP status code (e.g., 200, 404, 500) - modifiable */
  status: number;
  /** HTTP status text (e.g., "OK", "Not Found") - modifiable */
  statusText: string;
  /** Response headers as key-value pairs - modifiable */
  headers: Record<string, string>;
  /** Response body content - modifiable */
  body: string | null;
  /** Original request information (read-only) */
  readonly request: {
    /** Full request URL */
    url: string;
    /** HTTP method used */
    method: string;
    /** Host name */
    host: string;
    /** Request path */
    path: string;
    /** Request headers */
    headers: Record<string, string>;
  };
}

/** Script execution context - provides metadata and configuration */
interface BifrostContext {
  /** Unique identifier for this request */
  readonly requestId: string;
  /** Name of the current script */
  readonly scriptName: string;
  /** Type of script: "request" | "response" | "decode" */
  readonly scriptType: "request" | "response" | "decode";
  /** Current phase for decode: "request" | "response" */
  readonly phase?: "request" | "response";
  /** Custom key-value configuration from Bifrost settings */
  readonly values: Record<string, string>;
  /** List of rules that matched this request */
  readonly matchedRules: Array<{
    /** Rule pattern (e.g., "*.example.com") */
    pattern: string;
    /** Protocol (http/https) */
    protocol: string;
    /** Rule value/target */
    value: string;
  }>;
}

/** Logging interface - logs are captured and displayed in test results */
interface BifrostLog {
  /** Log a message (alias for info) */
  log(...args: any[]): void;
  /** Log debug level message */
  debug(...args: any[]): void;
  /** Log info level message */
  info(...args: any[]): void;
  /** Log warning level message */
  warn(...args: any[]): void;
  /** Log error level message */
  error(...args: any[]): void;
}

/** Sandbox file API (path is relative to scripts/_sandbox) */
interface BifrostFile {
  readonly enabled: boolean;
  readText(path: string): string;
  writeText(path: string, content: string): boolean;
  appendText(path: string, content: string): boolean;
  exists(path: string): boolean;
  remove(path: string): boolean;
  listDir(path?: string): string[];
}

/** Network request API (returns JSON string, use JSON.parse) */
interface BifrostNet {
  readonly enabled: boolean;
  fetch(url: string, optionsJson?: string): string;
  request(url: string, optionsJson?: string): string;
}

/** The response object to inspect and modify */
declare const response: BifrostResponse;
/** Script execution context with metadata */
declare const ctx: BifrostContext;
/** Logging interface */
declare const log: BifrostLog;
/** Console logging (alias for log) */
declare const console: BifrostLog;
/** File API */
declare const file: BifrostFile;
/** Network API */
declare const net: BifrostNet;
`;

function LogLevel({ level }: { level: ScriptLogEntry["level"] }) {
  const colors: Record<string, string> = {
    debug: "default",
    info: "blue",
    warn: "orange",
    error: "red",
  };
  return <Tag color={colors[level]}>{level.toUpperCase()}</Tag>;
}

function HighlightText({
  text,
  highlight,
  highlightStyle,
}: {
  text: string;
  highlight: string;
  highlightStyle?: React.CSSProperties;
}) {
  if (!highlight.trim()) {
    return <span>{text}</span>;
  }

  const regex = new RegExp(
    `(${highlight.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
    "gi",
  );
  const parts = text.split(regex);

  return (
    <span>
      {parts.map((part, index) =>
        regex.test(part) ? (
          <span
            key={index}
            style={{
              backgroundColor: "#faad14",
              color: "#000",
              padding: "0 2px",
              borderRadius: 2,
              ...highlightStyle,
            }}
          >
            {part}
          </span>
        ) : (
          <span key={index}>{part}</span>
        ),
      )}
    </span>
  );
}

function ScriptListPanel({
  searchValue,
  onSearchChange,
  onNewScript,
  loading,
  treeData,
  expandedKeys,
  autoExpandParent,
  onExpand,
  onSelect,
  renderTreeTitle,
  selectedKeys,
  onImportSuccess,
  onExportAll,
  onOpenSandboxSettings,
  hasScripts,
}: {
  searchValue: string;
  onSearchChange: (value: string) => void;
  onNewScript: (type: ScriptType) => void;
  loading: boolean;
  treeData: TreeDataNode[];
  expandedKeys: React.Key[];
  autoExpandParent: boolean;
  onExpand: (keys: React.Key[]) => void;
  onSelect: (
    keys: React.Key[],
    info: { node: { key: string; isLeaf?: boolean } },
  ) => void;
  renderTreeTitle: (nodeData: TreeDataNode) => React.ReactNode;
  selectedKeys: string[];
  onImportSuccess: () => void;
  onExportAll: () => void;
  onOpenSandboxSettings: () => void;
  hasScripts: boolean;
}) {
  const { token } = theme.useToken();
  const resolvedTheme = useThemeStore((s) => s.resolvedTheme);

  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: resolvedTheme === "dark" ? "#141414" : "#fff",
        borderRight: `1px solid ${token.colorBorderSecondary}`,
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          padding: "8px 12px",
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
          flexShrink: 0,
        }}
      >
        <span
          style={{
            fontSize: 13,
            fontWeight: 600,
            color: token.colorText,
          }}
        >
          Scripts
        </span>
        <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <Tooltip title="Sandbox Settings">
            <Button
              type="text"
              size="small"
              icon={<SettingOutlined />}
              onClick={onOpenSandboxSettings}
            />
          </Tooltip>
          <Button
            type="text"
            size="small"
            icon={<PlusOutlined />}
            onClick={() => onNewScript("request")}
          >
            Req
          </Button>
          <Button
            type="text"
            size="small"
            icon={<PlusOutlined />}
            onClick={() => onNewScript("response")}
            style={{ color: token.colorSuccess }}
          >
            Res
          </Button>
          <Button
            type="text"
            size="small"
            icon={<PlusOutlined />}
            onClick={() => onNewScript("decode")}
            style={{ color: "#722ed1" }}
          >
            Dec
          </Button>
          {hasScripts && (
            <Tooltip title="Export All">
              <Button
                type="text"
                size="small"
                icon={<ExportOutlined />}
                onClick={onExportAll}
              />
            </Tooltip>
          )}
          <ImportBifrostButton
            expectedType="script"
            onImportSuccess={onImportSuccess}
            buttonText=""
            buttonType="text"
            size="small"
          />
        </div>
      </div>

      <div
        style={{
          padding: "8px 12px",
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
          flexShrink: 0,
        }}
      >
        <Input
          placeholder="Search scripts..."
          prefix={<SearchOutlined />}
          value={searchValue}
          onChange={(e) => onSearchChange(e.target.value)}
          allowClear
          size="small"
        />
      </div>

      <div style={{ flex: 1, overflow: "auto", padding: "4px 0" }}>
        <Spin spinning={loading}>
          {treeData.length > 0 ? (
            <Tree
              blockNode
              treeData={treeData}
              expandedKeys={expandedKeys}
              autoExpandParent={autoExpandParent}
              onExpand={onExpand}
              onSelect={onSelect as Parameters<typeof Tree>["0"]["onSelect"]}
              titleRender={renderTreeTitle}
              selectedKeys={selectedKeys}
            />
          ) : searchValue ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No matching scripts"
            />
          ) : (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No scripts yet"
            />
          )}
        </Spin>
      </div>
    </div>
  );
}

function EditorPanel({
  selectedScript,
  selectedType,
  isNewScript,
  editorContent,
  onEditorChange,
  onSave,
  onDelete,
  onTest,
  saving,
  testing,
}: {
  selectedScript: ScriptInfo | null;
  selectedType: ScriptType;
  isNewScript: boolean;
  editorContent: string;
  onEditorChange: (value: string) => void;
  onSave: () => void;
  onDelete: () => void;
  onTest: () => void;
  saving: boolean;
  testing: boolean;
}) {
  const resolvedTheme = useThemeStore((s) => s.resolvedTheme);
  const saveRef = useRef(onSave);

  useEffect(() => {
    saveRef.current = onSave;
  }, [onSave]);

  const handleEditorMount = useCallback(
    (
      editor: Parameters<
        NonNullable<Parameters<typeof Editor>[0]["onMount"]>
      >[0],
      monaco: Parameters<
        NonNullable<Parameters<typeof Editor>[0]["onMount"]>
      >[1],
    ) => {
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
        saveRef.current();
      });
    },
    [],
  );

  if (!selectedScript) {
    return (
      <div
        style={{
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <div style={{ textAlign: "center", maxWidth: 560 }}>
          <Empty description="Select a script or create a new one" />
          <div
            style={{
              marginTop: 24,
              padding: "16px 24px",
              borderRadius: 8,
              textAlign: "left",
              background: resolvedTheme === "dark" ? "#1f1f1f" : "#fafafa",
            }}
          >
            <Text strong style={{ display: "block", marginBottom: 12 }}>
              How to use Scripts
            </Text>
            <ul
              style={{
                margin: 0,
                paddingLeft: 20,
                color: resolvedTheme === "dark" ? "#a6a6a6" : "#666",
              }}
            >
              <li style={{ marginBottom: 8 }}>
                <Text type="secondary">
                  <b>Request scripts</b> run before forwarding to upstream -
                  modify method, headers, or body
                </Text>
              </li>
              <li style={{ marginBottom: 8 }}>
                <Text type="secondary">
                  <b>Response scripts</b> run after receiving response - modify
                  status, headers, or body
                </Text>
              </li>
              <li style={{ marginBottom: 8 }}>
                <Text type="secondary">
                  Use <code>log.info()</code>, <code>log.warn()</code> to debug
                  your scripts
                </Text>
              </li>
              <li style={{ marginBottom: 8 }}>
                <Text type="secondary">
                  Access <code>ctx.values</code> for custom configuration values
                </Text>
              </li>
            </ul>
            <Text
              strong
              style={{ display: "block", marginTop: 16, marginBottom: 8 }}
            >
              Bind scripts to rules
            </Text>
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 8 }}
            >
              In the <b>Rules</b> page, use <code>reqScript://</code> or{" "}
              <code>resScript://</code> protocol:
            </Text>
            <pre
              style={{
                margin: 0,
                padding: "8px 12px",
                borderRadius: 4,
                fontSize: 12,
                background: resolvedTheme === "dark" ? "#141414" : "#f0f0f0",
                color: resolvedTheme === "dark" ? "#d9d9d9" : "#434343",
                overflow: "auto",
              }}
            >{`# Add auth header to API requests
api.example.com reqScript://add-auth-header

# Modify response for testing
*.example.com resScript://mock-response`}</pre>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div
        style={{
          padding: "8px 12px",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          flexShrink: 0,
          borderBottom: `1px solid ${resolvedTheme === "dark" ? "#303030" : "#e8e8e8"}`,
          backgroundColor: resolvedTheme === "dark" ? "#1f1f1f" : "#fafafa",
        }}
      >
        <Space size={8}>
          <Text
            style={{
              fontSize: 13,
              fontWeight: 500,
              fontFamily: "monospace",
            }}
          >
            {isNewScript ? "New Script" : selectedScript.name}
          </Text>
          <Tag
            color={
              selectedType === "request"
                ? "blue"
                : selectedType === "response"
                  ? "green"
                  : "purple"
            }
            style={{ margin: 0 }}
          >
            {selectedType}
          </Tag>
        </Space>
        <Space size={4}>
          <Tooltip title="Test Script">
            <Button
              type="text"
              size="small"
              icon={<PlayCircleOutlined />}
              onClick={onTest}
              loading={testing}
            />
          </Tooltip>
          <Tooltip title="Save (Cmd+S)">
            <Button
              type="text"
              size="small"
              icon={<SaveOutlined />}
              onClick={onSave}
              loading={saving}
            />
          </Tooltip>
          {!isNewScript && (
            <Tooltip title="Delete">
              <Button
                type="text"
                size="small"
                danger
                icon={<DeleteOutlined />}
                onClick={onDelete}
              />
            </Tooltip>
          )}
        </Space>
      </div>

      <div style={{ flex: 1, minHeight: 0 }}>
        <Editor
          height="100%"
          language="typescript"
          theme={resolvedTheme === "dark" ? "vs-dark" : "light"}
          value={editorContent}
          onChange={(value) => onEditorChange(value || "")}
          onMount={handleEditorMount}
          beforeMount={(monaco) => {
            monaco.languages.typescript.typescriptDefaults.setCompilerOptions({
              target: monaco.languages.typescript.ScriptTarget.ES2020,
              allowNonTsExtensions: true,
              moduleResolution:
                monaco.languages.typescript.ModuleResolutionKind.NodeJs,
              module: monaco.languages.typescript.ModuleKind.CommonJS,
              noEmit: true,
              strict: false,
              allowJs: true,
              checkJs: true,
              noImplicitAny: false,
              noUnusedLocals: false,
              noUnusedParameters: false,
            });
            monaco.languages.typescript.typescriptDefaults.setDiagnosticsOptions(
              {
                noSemanticValidation: false,
                noSyntaxValidation: false,
              },
            );
            monaco.languages.typescript.typescriptDefaults.setExtraLibs([]);
            const typeDefinition =
              selectedType === "request"
                ? BIFROST_TYPES_REQUEST
                : selectedType === "response"
                  ? BIFROST_TYPES_RESPONSE
                  : BIFROST_TYPES_DECODE;
            monaco.languages.typescript.typescriptDefaults.addExtraLib(
              typeDefinition,
              "bifrost.d.ts",
            );
          }}
          key={selectedType}
          options={{
            minimap: { enabled: false },
            fontSize: 13,
            lineNumbers: "on",
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 2,
            quickSuggestions: true,
            suggestOnTriggerCharacters: true,
            parameterHints: { enabled: true },
            wordBasedSuggestions: "currentDocument",
            folding: true,
            foldingHighlight: true,
            showFoldingControls: "mouseover",
            bracketPairColorization: { enabled: true },
          }}
        />
      </div>
    </div>
  );
}

function TestResultPanel({
  testResult,
  isExpanded,
  onToggle,
}: {
  testResult: ScriptExecutionResult | null;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const resolvedTheme = useThemeStore((s) => s.resolvedTheme);
  const { token } = theme.useToken();

  if (!isExpanded) {
    return (
      <div
        style={{
          position: "absolute",
          bottom: 12,
          right: 12,
          zIndex: 10,
        }}
      >
        <Tooltip title="Show Test Results">
          <Button
            type="default"
            size="small"
            icon={<UpOutlined />}
            onClick={onToggle}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              backgroundColor: testResult
                ? testResult.success
                  ? token.colorSuccessBg
                  : token.colorErrorBg
                : undefined,
              borderColor: testResult
                ? testResult.success
                  ? token.colorSuccess
                  : token.colorError
                : undefined,
            }}
          >
            {testResult ? (
              <>
                {testResult.success ? (
                  <CheckCircleOutlined style={{ color: token.colorSuccess }} />
                ) : (
                  <CloseCircleOutlined style={{ color: token.colorError }} />
                )}
                <span>Test Result</span>
              </>
            ) : (
              <span>Test Results</span>
            )}
          </Button>
        </Tooltip>
      </div>
    );
  }

  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      <div
        style={{
          padding: "8px 12px",
          flexShrink: 0,
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {testResult ? (
            <>
              {testResult.success ? (
                <CheckCircleOutlined style={{ color: "#52c41a" }} />
              ) : (
                <CloseCircleOutlined style={{ color: "#ff4d4f" }} />
              )}
              <Text strong>Test Result ({testResult.duration_ms}ms)</Text>
            </>
          ) : (
            <Text strong>Test Results</Text>
          )}
        </div>
        <Tooltip title="Hide Test Results">
          <Button
            type="text"
            size="small"
            icon={<DownOutlined />}
            onClick={onToggle}
          />
        </Tooltip>
      </div>

      <div style={{ flex: 1, overflow: "auto", padding: 12 }}>
        {!testResult ? (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: token.colorTextSecondary,
            }}
          >
            <Text type="secondary">Run a test to see results here</Text>
          </div>
        ) : (
          <>
            {testResult.error && (
              <div style={{ marginBottom: 12 }}>
                <pre
                  style={{
                    background:
                      resolvedTheme === "dark" ? "#1f1f1f" : "#f5f5f5",
                    color: token.colorError,
                    padding: 8,
                    borderRadius: 4,
                    fontSize: 12,
                    overflow: "auto",
                    maxHeight: 200,
                    margin: 0,
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                  }}
                >
                  {testResult.error}
                </pre>
              </div>
            )}

            {testResult.decode_output && (
              <div style={{ marginBottom: 12 }}>
                <Text strong>Decode Output:</Text>
                <pre
                  style={{
                    background:
                      resolvedTheme === "dark" ? "#1f1f1f" : "#f5f5f5",
                    color: resolvedTheme === "dark" ? "#e6e6e6" : "#1f1f1f",
                    padding: 8,
                    borderRadius: 4,
                    fontSize: 12,
                    overflow: "auto",
                    maxHeight: 200,
                    margin: "8px 0 0",
                  }}
                >
                  {JSON.stringify(testResult.decode_output, null, 2)}
                </pre>
              </div>
            )}

            {testResult.request_modifications && (
              <div style={{ marginBottom: 12 }}>
                <Text strong>Request Modifications:</Text>
                <pre
                  style={{
                    background:
                      resolvedTheme === "dark" ? "#1f1f1f" : "#f5f5f5",
                    color: resolvedTheme === "dark" ? "#e6e6e6" : "#1f1f1f",
                    padding: 8,
                    borderRadius: 4,
                    fontSize: 12,
                    overflow: "auto",
                    maxHeight: 150,
                    margin: "8px 0 0",
                  }}
                >
                  {JSON.stringify(testResult.request_modifications, null, 2)}
                </pre>
              </div>
            )}

            {testResult.response_modifications && (
              <div style={{ marginBottom: 12 }}>
                <Text strong>Response Modifications:</Text>
                <pre
                  style={{
                    background:
                      resolvedTheme === "dark" ? "#1f1f1f" : "#f5f5f5",
                    color: resolvedTheme === "dark" ? "#e6e6e6" : "#1f1f1f",
                    padding: 8,
                    borderRadius: 4,
                    fontSize: 12,
                    overflow: "auto",
                    maxHeight: 150,
                    margin: "8px 0 0",
                  }}
                >
                  {JSON.stringify(testResult.response_modifications, null, 2)}
                </pre>
              </div>
            )}

            <div>
              <Text strong>Logs:</Text>
              {testResult.logs.length > 0 ? (
                <div
                  style={{
                    fontFamily: "monospace",
                    fontSize: 12,
                    marginTop: 8,
                  }}
                >
                  {testResult.logs.map(
                    (logEntry: ScriptLogEntry, i: number) => (
                      <div key={i} style={{ marginBottom: 4 }}>
                        <LogLevel level={logEntry.level} />
                        <Text code style={{ marginLeft: 8 }}>
                          {new Date(logEntry.timestamp).toLocaleTimeString()}
                        </Text>
                        <Text style={{ marginLeft: 8 }}>
                          {logEntry.message}
                        </Text>
                      </div>
                    ),
                  )}
                </div>
              ) : (
                <Text type="secondary" style={{ marginLeft: 8 }}>
                  No logs
                </Text>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default function ScriptsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    requestScripts,
    responseScripts,
    decodeScripts,
    selectedScript,
    selectedType,
    loading,
    saving,
    testing,
    testResult,
    fetchScripts,
    selectScript,
    saveScript,
    deleteScript,
    testScript,
    createNewScript,
  } = useScriptsStore();
  const { exportFile } = useExportBifrost();

  const [editorContent, setEditorContent] = useState("");
  const [newScriptName, setNewScriptName] = useState("");
  const [isNewScript, setIsNewScript] = useState(false);
  const [showNameModal, setShowNameModal] = useState(false);
  const [searchValue, setSearchValue] = useState("");
  const [expandedKeys, setExpandedKeys] = useState<React.Key[]>([]);
  const [autoExpandParent, setAutoExpandParent] = useState(true);
  const [lastSelectedScriptId, setLastSelectedScriptId] = useState<
    string | null
  >(null);
  const [lastScriptsHash, setLastScriptsHash] = useState<string>("");
  const [testResultExpanded, setTestResultExpanded] = useState(false);
  const urlParamRef = useRef(false);

  const [sandboxModalOpen, setSandboxModalOpen] = useState(false);
  const [sandboxLoading, setSandboxLoading] = useState(false);
  const [sandboxSaving, setSandboxSaving] = useState(false);
  const [sandboxEnabled, setSandboxEnabled] = useState(true);
  const [sandboxDirsText, setSandboxDirsText] = useState("");
  const [sandboxDirName, setSandboxDirName] = useState("_sandbox");
  const [sandboxFileMaxBytes, setSandboxFileMaxBytes] = useState(1024 * 1024);
  const [sandboxNetMaxReqBytes, setSandboxNetMaxReqBytes] = useState(256 * 1024);
  const [sandboxNetMaxRespBytes, setSandboxNetMaxRespBytes] = useState(1024 * 1024);
  const [sandboxNetTimeoutMs, setSandboxNetTimeoutMs] = useState(5000);
  const [sandboxTimeoutMs, setSandboxTimeoutMs] = useState(10000);
  const [sandboxMaxMemoryBytes, setSandboxMaxMemoryBytes] = useState(32 * 1024 * 1024);
  const [sandboxMaxDecodeInputBytes, setSandboxMaxDecodeInputBytes] = useState(2 * 1024 * 1024);
  const [sandboxMaxDecompressOutputBytes, setSandboxMaxDecompressOutputBytes] = useState(
    10 * 1024 * 1024,
  );

  useEffect(() => {
    fetchScripts();
  }, [fetchScripts]);

  useEffect(() => {
    if (testResult && !testResultExpanded) {
      setTestResultExpanded(true);
    }
  }, [testResult, testResultExpanded]);

  useEffect(() => {
    const typeParam = searchParams.get("type") as ScriptType | null;
    const nameParam = searchParams.get("name");
    const scripts = [...requestScripts, ...responseScripts, ...decodeScripts];

    if (typeParam && nameParam && scripts.length > 0 && !urlParamRef.current) {
      const exists = scripts.some(
        (s) => s.script_type === typeParam && s.name === nameParam,
      );
      if (exists) {
        urlParamRef.current = true;
        selectScript(typeParam, nameParam);
        setSearchParams({}, { replace: true });
      }
    }
  }, [
    searchParams,
    requestScripts,
    responseScripts,
    decodeScripts,
    selectScript,
    setSearchParams,
  ]);

  const allScripts = useMemo(
    () => [...requestScripts, ...responseScripts, ...decodeScripts],
    [requestScripts, responseScripts, decodeScripts],
  );

  const getAllFolderKeys = useCallback((scripts: ScriptInfo[]): string[] => {
    const folderSet = new Set<string>();
    for (const script of scripts) {
      const parts = script.name.split("/");
      parts.pop();
      let currentPath = "";
      for (const part of parts) {
        currentPath = currentPath ? `${currentPath}/${part}` : part;
        folderSet.add(`folder:${currentPath}`);
      }
    }
    return Array.from(folderSet);
  }, []);

  const currentScriptId = selectedScript
    ? `${selectedType}/${selectedScript.name}`
    : null;
  if (currentScriptId !== lastSelectedScriptId) {
    setLastSelectedScriptId(currentScriptId);
    if (selectedScript) {
      setEditorContent(selectedScript.content);
      setIsNewScript(!selectedScript.name);
    }
  }

  const scriptsHash = allScripts.map((s) => s.name).join(",");
  if (scriptsHash !== lastScriptsHash) {
    setLastScriptsHash(scriptsHash);
    const allFolderKeys = getAllFolderKeys(allScripts);
    setExpandedKeys(allFolderKeys);
  }

  const getScriptsInFolder = useCallback(
    (folderPath: string): { type: ScriptType; name: string }[] => {
      const result: { type: ScriptType; name: string }[] = [];
      for (const script of allScripts) {
        if (
          script.name.startsWith(folderPath + "/") ||
          script.name === folderPath
        ) {
          result.push({ type: script.script_type, name: script.name });
        }
      }
      return result;
    },
    [allScripts],
  );

  const handleSelectScript = useCallback(
    (_keys: React.Key[], info: { node: { key: string; isLeaf?: boolean } }) => {
      const key = info.node.key;
      if (key.startsWith("folder:")) {
        return;
      }
      if (
        key.startsWith("request/") ||
        key.startsWith("response/") ||
        key.startsWith("decode/")
      ) {
        const [type, ...nameParts] = key.split("/");
        const name = nameParts.join("/");
        selectScript(type as ScriptType, name);
        setIsNewScript(false);
      }
    },
    [selectScript],
  );

  const handleExpand = useCallback((keys: React.Key[]) => {
    setExpandedKeys(keys);
    setAutoExpandParent(false);
  }, []);

  const handleSave = useCallback(async () => {
    if (isNewScript) {
      setShowNameModal(true);
      return;
    }
    if (selectedScript?.name) {
      await saveScript(selectedType, selectedScript.name, editorContent);
      message.success("Script saved");
    }
  }, [isNewScript, selectedScript, selectedType, editorContent, saveScript]);

  const handleSaveNewScript = useCallback(async () => {
    if (!newScriptName.trim()) {
      message.error("Please enter a script name");
      return;
    }
    const validName = /^[a-zA-Z0-9_\-/]+$/.test(newScriptName);
    if (!validName) {
      message.error(
        "Script name can only contain letters, numbers, hyphens, underscores and slashes",
      );
      return;
    }
    await saveScript(selectedType, newScriptName, editorContent);
    setShowNameModal(false);
    setNewScriptName("");
    setIsNewScript(false);
    message.success("Script created");
  }, [newScriptName, selectedType, editorContent, saveScript]);

  const handleDelete = useCallback(async () => {
    if (!selectedScript?.name) return;
    Modal.confirm({
      title: "Delete Script",
      content: `Are you sure you want to delete "${selectedScript.name}"?`,
      okText: "Delete",
      okType: "danger",
      onOk: async () => {
        await deleteScript(selectedType, selectedScript.name);
        message.success("Script deleted");
      },
    });
  }, [selectedScript, selectedType, deleteScript]);

  const handleDeleteFolder = useCallback(
    async (folderPath: string) => {
      const scriptsToDelete = getScriptsInFolder(folderPath);
      if (scriptsToDelete.length === 0) {
        message.info("No scripts in this folder");
        return;
      }

      Modal.confirm({
        title: "Delete Folder",
        content: `Are you sure you want to delete folder "${folderPath}" and all ${scriptsToDelete.length} script(s) inside?`,
        okText: "Delete All",
        okType: "danger",
        onOk: async () => {
          for (const script of scriptsToDelete) {
            await deleteScript(script.type, script.name);
          }
          message.success(`Deleted ${scriptsToDelete.length} script(s)`);
        },
      });
    },
    [getScriptsInFolder, deleteScript],
  );

  const handleTest = useCallback(async () => {
    await testScript(selectedType, editorContent);
  }, [selectedType, editorContent, testScript]);

  const handleNewScript = useCallback(
    (type: ScriptType) => {
      createNewScript(type);
      setIsNewScript(true);
    },
    [createNewScript],
  );

  const handleExport = useCallback(
    async (scriptNames: string[]) => {
      if (scriptNames.length === 0) return;
      await exportFile("script", { script_names: scriptNames });
    },
    [exportFile],
  );

  const handleExportAll = useCallback(async () => {
    const names = allScripts.map((s) => `${s.script_type}/${s.name}`);
    if (names.length === 0) return;
    await exportFile("script", { script_names: names });
  }, [allScripts, exportFile]);

  const handleImportSuccess = useCallback(() => {
    fetchScripts();
  }, [fetchScripts]);

  const openSandboxSettings = useCallback(async () => {
    setSandboxModalOpen(true);
    setSandboxLoading(true);
    try {
      const cfg = await getSandboxConfig();
      setSandboxEnabled(cfg.net.enabled);
      setSandboxDirName(cfg.file.sandbox_dir || "_sandbox");
      setSandboxDirsText((cfg.file.allowed_dirs || []).join("\n"));
      setSandboxFileMaxBytes(cfg.file.max_bytes);
      setSandboxNetMaxReqBytes(cfg.net.max_request_bytes);
      setSandboxNetMaxRespBytes(cfg.net.max_response_bytes);
      setSandboxNetTimeoutMs(cfg.net.timeout_ms);
      setSandboxTimeoutMs(cfg.limits.timeout_ms);
      setSandboxMaxMemoryBytes(cfg.limits.max_memory_bytes);
      setSandboxMaxDecodeInputBytes(cfg.limits.max_decode_input_bytes);
      setSandboxMaxDecompressOutputBytes(cfg.limits.max_decompress_output_bytes);
    } catch {
      message.error("加载 Sandbox 配置失败");
    } finally {
      setSandboxLoading(false);
    }
  }, []);

  const saveSandboxSettings = useCallback(async () => {
    const dirName = sandboxDirName.trim();
    if (!dirName) {
      message.error("Sandbox 目录不能为空");
      return;
    }

    const asPositiveInt = (v: number) => {
      if (!Number.isFinite(v)) return null;
      const n = Math.floor(v);
      return n > 0 ? n : null;
    };
    const fileMax = asPositiveInt(sandboxFileMaxBytes);
    const netReqMax = asPositiveInt(sandboxNetMaxReqBytes);
    const netRespMax = asPositiveInt(sandboxNetMaxRespBytes);
    const netTimeout = asPositiveInt(sandboxNetTimeoutMs);
    const timeoutMs = asPositiveInt(sandboxTimeoutMs);
    const memBytes = asPositiveInt(sandboxMaxMemoryBytes);
    const maxDecodeBytes = asPositiveInt(sandboxMaxDecodeInputBytes);
    const maxDecompressBytes = asPositiveInt(sandboxMaxDecompressOutputBytes);
    if (
      !fileMax ||
      !netReqMax ||
      !netRespMax ||
      !netTimeout ||
      !timeoutMs ||
      !memBytes ||
      !maxDecodeBytes ||
      !maxDecompressBytes
    ) {
      message.error("数值配置必须为正整数");
      return;
    }

    setSandboxSaving(true);
    try {
      const dirs = sandboxDirsText
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean);

      const invalid = dirs.find((d) => !d.startsWith("/"));
      if (invalid) {
        message.error(`allowed_dirs 必须是绝对路径：${invalid}`);
        return;
      }

      await updateSandboxConfig({
        file: {
          sandbox_dir: dirName,
          allowed_dirs: dirs,
          max_bytes: fileMax,
        },
        net: {
          enabled: sandboxEnabled,
          timeout_ms: netTimeout,
          max_request_bytes: netReqMax,
          max_response_bytes: netRespMax,
        },
        limits: {
          timeout_ms: timeoutMs,
          max_memory_bytes: memBytes,
          max_decode_input_bytes: maxDecodeBytes,
          max_decompress_output_bytes: maxDecompressBytes,
        },
      });
      message.success("Sandbox 配置已保存");
      setSandboxModalOpen(false);
    } catch {
      message.error("保存 Sandbox 配置失败（请检查目录是否为绝对路径）");
    } finally {
      setSandboxSaving(false);
    }
  }, [
    sandboxDirsText,
    sandboxDirName,
    sandboxFileMaxBytes,
    sandboxEnabled,
    sandboxNetTimeoutMs,
    sandboxNetMaxReqBytes,
    sandboxNetMaxRespBytes,
    sandboxTimeoutMs,
    sandboxMaxMemoryBytes,
    sandboxMaxDecodeInputBytes,
    sandboxMaxDecompressOutputBytes,
  ]);

  const handleSearch = useCallback(
    (value: string) => {
      setSearchValue(value);
      if (value) {
        const matchedFolders = new Set<string>();
        for (const script of allScripts) {
          if (script.name.toLowerCase().includes(value.toLowerCase())) {
            const parts = script.name.split("/");
            parts.pop();
            let currentPath = "";
            for (const part of parts) {
              currentPath = currentPath ? `${currentPath}/${part}` : part;
              matchedFolders.add(`folder:${currentPath}`);
            }
          }
        }
        setExpandedKeys(Array.from(matchedFolders));
        setAutoExpandParent(true);
      } else {
        const allFolderKeys = getAllFolderKeys(allScripts);
        setExpandedKeys(allFolderKeys);
      }
    },
    [allScripts, getAllFolderKeys],
  );

  const buildTreeData = useCallback(
    (scripts: ScriptInfo[], search: string): TreeDataNode[] => {
      const folderMap = new Map<string, TreeDataNode>();
      const rootItems: TreeDataNode[] = [];

      const filteredScripts = search
        ? scripts.filter((s) =>
            s.name.toLowerCase().includes(search.toLowerCase()),
          )
        : scripts;

      const sortedScripts = [...filteredScripts].sort((a, b) =>
        a.name.localeCompare(b.name),
      );

      for (const script of sortedScripts) {
        const parts = script.name.split("/");
        const fileName = parts.pop()!;
        const folderPath = parts.join("/");

        const scriptNode: TreeDataNode = {
          title: (
            <Space size={4}>
              <HighlightText text={fileName} highlight={search} />
              <Tag
                color={
                  script.script_type === "request"
                    ? "blue"
                    : script.script_type === "response"
                      ? "green"
                      : "purple"
                }
                style={{
                  fontSize: 10,
                  padding: "0 2px",
                  lineHeight: "16px",
                  marginLeft: 4,
                }}
              >
                {script.script_type === "request"
                  ? "REQ"
                  : script.script_type === "response"
                    ? "RES"
                    : "DEC"}
              </Tag>
            </Space>
          ),
          key: `${script.script_type}/${script.name}`,
          isLeaf: true,
        };

        if (folderPath) {
          let currentPath = "";
          let parentChildren = rootItems;

          for (const part of parts) {
            currentPath = currentPath ? `${currentPath}/${part}` : part;

            if (!folderMap.has(currentPath)) {
              const folderNode: TreeDataNode = {
                title: <HighlightText text={part} highlight={search} />,
                key: `folder:${currentPath}`,
                children: [],
                selectable: false,
              };
              folderMap.set(currentPath, folderNode);
              parentChildren.push(folderNode);
            }

            parentChildren = folderMap.get(currentPath)!
              .children as TreeDataNode[];
          }

          parentChildren.push(scriptNode);
        } else {
          rootItems.push(scriptNode);
        }
      }

      const sortNodes = (nodes: TreeDataNode[]): TreeDataNode[] => {
        return nodes
          .sort((a, b) => {
            const aIsFolder = !a.isLeaf;
            const bIsFolder = !b.isLeaf;
            if (aIsFolder !== bIsFolder) return aIsFolder ? -1 : 1;
            const aKey = a.key as string;
            const bKey = b.key as string;
            const aName = aKey.split("/").pop() || aKey.replace("folder:", "");
            const bName = bKey.split("/").pop() || bKey.replace("folder:", "");
            return aName.localeCompare(bName);
          })
          .map((node) => {
            if (node.children) {
              return {
                ...node,
                children: sortNodes(node.children as TreeDataNode[]),
              };
            }
            return node;
          });
      };

      return sortNodes(rootItems);
    },
    [],
  );

  const treeData = useMemo(
    () => buildTreeData(allScripts, searchValue),
    [allScripts, searchValue, buildTreeData],
  );

  const renderTreeTitle = useCallback(
    (nodeData: TreeDataNode) => {
      const key = nodeData.key as string;
      let menuItems: MenuProps["items"] = [];

      if (key.startsWith("folder:")) {
        const folderPath = key.replace("folder:", "");
        const scriptsInFolder = getScriptsInFolder(folderPath);
        const scriptNames = scriptsInFolder.map((s) => `${s.type}/${s.name}`);
        menuItems = [
          {
            key: "new-request",
            label: "New Request Script",
            onClick: () => handleNewScript("request"),
          },
          {
            key: "new-response",
            label: "New Response Script",
            onClick: () => handleNewScript("response"),
          },
          {
            key: "new-decode",
            label: "New Decode Script",
            onClick: () => handleNewScript("decode"),
          },
          { type: "divider" },
          {
            key: "export-folder",
            icon: <ExportOutlined />,
            label: `Export Folder (${scriptsInFolder.length} scripts)`,
            onClick: () => handleExport(scriptNames),
            disabled: scriptsInFolder.length === 0,
          },
          { type: "divider" },
          {
            key: "delete-folder",
            label: `Delete Folder (${scriptsInFolder.length} scripts)`,
            danger: true,
            onClick: () => handleDeleteFolder(folderPath),
          },
        ];
      } else if (
        key.startsWith("request/") ||
        key.startsWith("response/") ||
        key.startsWith("decode/")
      ) {
        const [type, ...nameParts] = key.split("/");
        const scriptType = type as ScriptType;
        const scriptName = nameParts.join("/");
        const fullName = `${scriptType}/${scriptName}`;
        menuItems = [
          {
            key: "export",
            icon: <ExportOutlined />,
            label: "Export",
            onClick: () => handleExport([fullName]),
          },
          { type: "divider" },
          {
            key: "delete",
            label: "Delete",
            danger: true,
            onClick: () => {
              Modal.confirm({
                title: "Delete Script",
                content: `Are you sure you want to delete "${scriptName}"?`,
                okText: "Delete",
                okType: "danger",
                onOk: async () => {
                  await deleteScript(scriptType, scriptName);
                  message.success("Script deleted");
                },
              });
            },
          },
        ];
      }

      const isFolder = key.startsWith("folder:");
      const isExpanded = expandedKeys.includes(key);

      return (
        <span
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            width: "100%",
          }}
        >
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              minWidth: 0,
              flex: 1,
            }}
          >
            {isFolder ? (
              isExpanded ? (
                <FolderOpenOutlined />
              ) : (
                <FolderOutlined />
              )
            ) : (
              <FileOutlined />
            )}
            {nodeData.title as React.ReactNode}
          </span>
          <Dropdown menu={{ items: menuItems }} trigger={["click"]}>
            <Button
              type="text"
              size="small"
              icon={<MoreOutlined />}
              onClick={(e) => e.stopPropagation()}
              style={{ flexShrink: 0 }}
            />
          </Dropdown>
        </span>
      );
    },
    [
      expandedKeys,
      getScriptsInFolder,
      handleNewScript,
      handleDeleteFolder,
      handleExport,
      deleteScript,
    ],
  );

  const leftPanel = (
    <ScriptListPanel
      searchValue={searchValue}
      onSearchChange={handleSearch}
      onNewScript={handleNewScript}
      loading={loading}
      treeData={treeData}
      expandedKeys={expandedKeys}
      autoExpandParent={autoExpandParent}
      onExpand={handleExpand}
      onSelect={handleSelectScript}
      renderTreeTitle={renderTreeTitle}
      selectedKeys={
        selectedScript?.name ? [`${selectedType}/${selectedScript.name}`] : []
      }
      onImportSuccess={handleImportSuccess}
      onExportAll={handleExportAll}
      onOpenSandboxSettings={openSandboxSettings}
      hasScripts={allScripts.length > 0}
    />
  );

  const editorPanel = (
    <EditorPanel
      selectedScript={selectedScript}
      selectedType={selectedType}
      isNewScript={isNewScript}
      editorContent={editorContent}
      onEditorChange={setEditorContent}
      onSave={handleSave}
      onDelete={handleDelete}
      onTest={handleTest}
      saving={saving}
      testing={testing}
    />
  );

  const handleToggleTestResult = useCallback(() => {
    setTestResultExpanded((prev) => !prev);
  }, []);

  const testResultPanel = (
    <TestResultPanel
      testResult={testResult}
      isExpanded={testResultExpanded}
      onToggle={handleToggleTestResult}
    />
  );

  const rightPanel = testResultExpanded ? (
    <VerticalSplitPane
      top={editorPanel}
      bottom={testResultPanel}
      defaultTopHeight="60%"
      minTopHeight={200}
      minBottomHeight={120}
    />
  ) : (
    <div style={{ height: "100%", position: "relative" }}>
      {editorPanel}
      {testResultPanel}
    </div>
  );

  return (
    <>
      <div style={{ height: "100%", overflow: "hidden" }}>
        <SplitPane
          left={leftPanel}
          right={rightPanel}
          defaultLeftWidth="280px"
          minLeftWidth={200}
          minRightWidth={400}
        />
      </div>

      <Modal
        title="Save New Script"
        open={showNameModal}
        onOk={handleSaveNewScript}
        onCancel={() => setShowNameModal(false)}
        okText="Save"
      >
        <Input
          placeholder="Enter script name (e.g., api/add-auth-header)"
          value={newScriptName}
          onChange={(e) => setNewScriptName(e.target.value)}
          onPressEnter={handleSaveNewScript}
        />
        <Text type="secondary" style={{ display: "block", marginTop: 8 }}>
          Use "/" to create folders. Only letters, numbers, hyphens, underscores
          and slashes are allowed.
        </Text>
      </Modal>

      <Modal
        title="Sandbox 设置"
        open={sandboxModalOpen}
        onOk={saveSandboxSettings}
        onCancel={() => setSandboxModalOpen(false)}
        okText="保存"
        confirmLoading={sandboxSaving}
      >
        <Spin spinning={sandboxLoading}>
          <Form layout="vertical">
            <Form.Item label="网络请求 (net.fetch) 开关">
              <Switch checked={sandboxEnabled} onChange={setSandboxEnabled} />
            </Form.Item>
            <Form.Item label="Sandbox 目录 (scripts 下的相对目录名，或绝对路径)">
              <Input value={sandboxDirName} onChange={(e) => setSandboxDirName(e.target.value)} />
            </Form.Item>
            <Form.Item label="允许访问的系统目录 (allowed_dirs，每行一个绝对路径)">
              <Input.TextArea
                value={sandboxDirsText}
                onChange={(e) => setSandboxDirsText(e.target.value)}
                autoSize={{ minRows: 4, maxRows: 10 }}
                placeholder="/Users/xxx/data\n/var/log"
              />
            </Form.Item>
            <Form.Item label="文件最大字节数 (file.max_bytes)">
              <Input
                type="number"
                value={sandboxFileMaxBytes}
                onChange={(e) => setSandboxFileMaxBytes(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="网络请求体最大字节数 (net.max_request_bytes)">
              <Input
                type="number"
                value={sandboxNetMaxReqBytes}
                onChange={(e) => setSandboxNetMaxReqBytes(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="网络响应体最大字节数 (net.max_response_bytes)">
              <Input
                type="number"
                value={sandboxNetMaxRespBytes}
                onChange={(e) => setSandboxNetMaxRespBytes(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="网络超时 (net.timeout_ms)">
              <Input
                type="number"
                value={sandboxNetTimeoutMs}
                onChange={(e) => setSandboxNetTimeoutMs(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="脚本超时 (limits.timeout_ms)">
              <Input
                type="number"
                value={sandboxTimeoutMs}
                onChange={(e) => setSandboxTimeoutMs(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="脚本最大内存 (limits.max_memory_bytes)">
              <Input
                type="number"
                value={sandboxMaxMemoryBytes}
                onChange={(e) => setSandboxMaxMemoryBytes(Number(e.target.value))}
              />
            </Form.Item>
            <Form.Item label="decode 输入最大字节数 (limits.max_decode_input_bytes)">
              <Input
                type="number"
                value={sandboxMaxDecodeInputBytes}
                onChange={(e) =>
                  setSandboxMaxDecodeInputBytes(Number(e.target.value))
                }
              />
            </Form.Item>
            <Form.Item label="HTTP 解压输出最大字节数 (limits.max_decompress_output_bytes)">
              <Input
                type="number"
                value={sandboxMaxDecompressOutputBytes}
                onChange={(e) =>
                  setSandboxMaxDecompressOutputBytes(Number(e.target.value))
                }
              />
            </Form.Item>
          </Form>
        </Spin>
      </Modal>
    </>
  );
}
