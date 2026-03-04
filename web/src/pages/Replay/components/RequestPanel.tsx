import {
  useCallback,
  useState,
  useMemo,
  useEffect,
  useRef,
  type CSSProperties,
} from "react";
import {
  Input,
  InputNumber,
  Tabs,
  Button,
  Select,
  Space,
  Switch,
  Dropdown,
  Modal,
  message,
  Tooltip,
  theme,
  ConfigProvider,
  Table,
} from "antd";
import type { MenuProps } from "antd";
import type { ColumnsType } from "antd/es/table";
import {
  PlusOutlined,
  DeleteOutlined,
  SendOutlined,
  SaveOutlined,
  SettingOutlined,
  CaretDownOutlined,
  DisconnectOutlined,
  LinkOutlined,
  ClockCircleOutlined,
  StopOutlined,
} from "@ant-design/icons";
import {
  useReplayStore,
  type RequestPanelTab,
} from "../../../stores/useReplayStore";
import { useRulesStore } from "../../../stores/useRulesStore";
import CodeEditor from "./CodeEditor";
import {
  DEFAULT_TIMEOUT_MS,
  type ReplayKeyValueItem,
  type ReplayRequest,
  type BodyType,
  type RawType,
  type RuleMode,
} from "../../../types";

const HTTP_METHODS = [
  "GET",
  "POST",
  "PUT",
  "DELETE",
  "PATCH",
  "HEAD",
  "OPTIONS",
];

const METHOD_COLORS: Record<string, string> = {
  GET: "#52c41a",
  POST: "#1890ff",
  PUT: "#fa8c16",
  DELETE: "#f5222d",
  PATCH: "#722ed1",
  OPTIONS: "#8c8c8c",
  HEAD: "#13c2c2",
};

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

interface ParsedCurl {
  method: string;
  url: string;
  headers: ReplayKeyValueItem[];
  body?: {
    type: BodyType;
    raw_type?: RawType;
    content?: string;
    form_data?: ReplayKeyValueItem[];
  };
}

function tokenizeCommand(input: string): string[] {
  const normalized = input.trim().replace(/\\\r?\n/g, " ");
  const tokens: string[] = [];
  let i = 0;
  let current = "";
  let quote: "'" | '"' | null = null;

  const pushCurrent = () => {
    if (current !== "") {
      tokens.push(current);
      current = "";
    }
  };

  while (i < normalized.length) {
    const ch = normalized[i];

    if (quote === null) {
      if (/\s/.test(ch)) {
        pushCurrent();
        i += 1;
        continue;
      }

      if (ch === "'" || ch === '"') {
        quote = ch;
        i += 1;
        continue;
      }

      if (ch === "\\") {
        if (i + 1 < normalized.length) {
          current += normalized[i + 1];
          i += 2;
          continue;
        }
        i += 1;
        continue;
      }

      current += ch;
      i += 1;
      continue;
    }

    if (quote === "'") {
      if (ch === "'") {
        quote = null;
        i += 1;
        continue;
      }
      current += ch;
      i += 1;
      continue;
    }

    if (ch === '"') {
      quote = null;
      i += 1;
      continue;
    }

    if (ch === "\\") {
      if (i + 1 < normalized.length) {
        current += normalized[i + 1];
        i += 2;
        continue;
      }
      i += 1;
      continue;
    }

    current += ch;
    i += 1;
  }

  pushCurrent();
  return tokens;
}

function parseCurl(curlCommand: string): ParsedCurl | null {
  const tokens = tokenizeCommand(curlCommand);
  if (tokens.length === 0 || tokens[0].toLowerCase() !== "curl") {
    return null;
  }

  let method = "GET";
  let url = "";
  const headers: ReplayKeyValueItem[] = [];
  const bodyParts: string[] = [];
  let contentType = "";

  const positional: string[] = [];
  const optionsWithValue = new Set([
    "-X",
    "--request",
    "--url",
    "-H",
    "--header",
    "-d",
    "--data",
    "--data-raw",
    "--data-binary",
    "--data-urlencode",
    "--json",
    "-e",
    "--referer",
    "-u",
    "--user",
    "-b",
    "--cookie",
    "-x",
    "--proxy",
  ]);

  for (let i = 1; i < tokens.length; i += 1) {
    const token = tokens[i];

    if (token === "--") {
      positional.push(...tokens.slice(i + 1));
      break;
    }

    if (token === "-X" || token === "--request") {
      const value = tokens[i + 1];
      if (value) {
        method = value.toUpperCase();
        i += 1;
      }
      continue;
    }

    if (token === "--url") {
      const value = tokens[i + 1];
      if (value) {
        url = value;
        i += 1;
      }
      continue;
    }

    if (token === "-e" || token === "--referer") {
      if (tokens[i + 1]) {
        i += 1;
      }
      continue;
    }

    if (token === "-H" || token === "--header") {
      const headerValue = tokens[i + 1];
      if (headerValue) {
        const colonIndex = headerValue.indexOf(":");
        if (colonIndex !== -1) {
          const key = headerValue.substring(0, colonIndex).trim();
          const value = headerValue.substring(colonIndex + 1).trim();
          headers.push({ id: generateId(), key, value, enabled: true });
          if (key.toLowerCase() === "content-type") {
            contentType = value.toLowerCase();
          }
        }
        i += 1;
      }
      continue;
    }

    if (
      token === "-d" ||
      token === "--data" ||
      token === "--data-raw" ||
      token === "--data-binary" ||
      token === "--data-urlencode" ||
      token === "--json"
    ) {
      const value = tokens[i + 1];
      if (value !== undefined) {
        bodyParts.push(value);
        i += 1;
      }
      if (method === "GET") {
        method = "POST";
      }
      continue;
    }

    if (token.startsWith("-")) {
      if (optionsWithValue.has(token) && tokens[i + 1]) {
        i += 1;
      }
      continue;
    }

    positional.push(token);
  }

  if (!url) {
    const urlCandidates = positional.filter((t) => /^(https?|wss?):\/\//i.test(t));
    if (urlCandidates.length > 0) {
      url = urlCandidates[urlCandidates.length - 1];
    } else if (positional.length > 0) {
      url = positional[positional.length - 1];
    }
  }

  if (!url) {
    return null;
  }

  const result: ParsedCurl = {
    method,
    url,
    headers,
  };

  if (bodyParts.length > 0) {
    let rawType: RawType = "text";
    if (contentType.includes("json")) {
      rawType = "json";
    } else if (contentType.includes("xml")) {
      rawType = "xml";
    }

    result.body = {
      type: "raw",
      raw_type: rawType,
      content: bodyParts.join("&"),
    };
  }

  return result;
}

const VALID_URL_PROTOCOLS = ["http://", "https://", "ws://", "wss://"];

function hasValidProtocol(url: string): boolean {
  const lowerUrl = url.toLowerCase();
  return VALID_URL_PROTOCOLS.some((protocol) => lowerUrl.startsWith(protocol));
}

function normalizeUrl(url: string): string {
  if (!url || url.trim() === "") {
    return url;
  }

  const trimmedUrl = url.trim();
  if (hasValidProtocol(trimmedUrl)) {
    return trimmedUrl;
  }

  return `https://${trimmedUrl}`;
}

function isValidUrl(url: string): boolean {
  if (!url || url.trim() === "") {
    return false;
  }

  if (!hasValidProtocol(url)) {
    return false;
  }

  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

export default function RequestPanel() {
  const { token } = theme.useToken();
  const {
    currentRequest,
    ruleConfig,
    timeoutMs,
    executing,
    streamingConnection,
    uiState,
    updateCurrentRequest,
    saveRequest,
    executeRequest,
    setRuleConfig,
    setTimeoutMs,
    updateUIState,
    connectWebSocket,
    disconnectSSE,
    disconnectWebSocket,
    cancelRequest,
  } = useReplayStore();

  const { rules, fetchRules } = useRulesStore();
  const activeTab = uiState.requestPanelActiveTab;
  const requestType = uiState.requestType;
  const saveModalVisible = uiState.saveModalVisible;
  const saveName = uiState.saveName;
  const ruleSelectVisible = uiState.ruleSelectVisible;
  const isConnected = streamingConnection?.status === "connected";
  const isConnecting = streamingConnection?.status === "connecting";

  const setActiveTab = useCallback(
    (tab: string) => {
      updateUIState({ requestPanelActiveTab: tab as RequestPanelTab });
    },
    [updateUIState],
  );

  const setSaveModalVisible = useCallback(
    (visible: boolean) => {
      updateUIState({ saveModalVisible: visible });
    },
    [updateUIState],
  );

  const setSaveName = useCallback(
    (name: string) => {
      updateUIState({ saveName: name });
    },
    [updateUIState],
  );

  const setRuleSelectVisible = useCallback(
    (visible: boolean) => {
      updateUIState({ ruleSelectVisible: visible });
    },
    [updateUIState],
  );

  const handleMethodChange = useCallback(
    (method: string) => {
      updateCurrentRequest({ method });
    },
    [updateCurrentRequest],
  );

  const handleUrlChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const url = e.target.value;
      updateCurrentRequest({ url });

      if (url) {
        const lowerUrl = url.toLowerCase();
        if (lowerUrl.startsWith("ws://") || lowerUrl.startsWith("wss://")) {
          if (requestType !== "websocket") {
            updateUIState({ requestType: "websocket" });
          }
        } else if (
          lowerUrl.startsWith("http://") ||
          lowerUrl.startsWith("https://")
        ) {
          if (requestType === "websocket") {
            updateUIState({ requestType: "http" });
          }
        }
      }
    },
    [updateCurrentRequest, requestType, updateUIState],
  );

  const handleUrlPaste = useCallback(
    (e: React.ClipboardEvent<HTMLInputElement>) => {
      const pastedText = e.clipboardData.getData("text");
      const parsed = parseCurl(pastedText);
      if (parsed) {
        e.preventDefault();
        const updates: Partial<ReplayRequest> = {
          method: parsed.method,
          url: parsed.url,
        };
        if (parsed.headers.length > 0) {
          updates.headers = parsed.headers;
        }
        if (parsed.body) {
          updates.body = parsed.body;
        }
        updateCurrentRequest(updates);
        message.success("cURL command imported successfully");
      }
    },
    [updateCurrentRequest],
  );

  const handleUrlBlur = useCallback(() => {
    const url = currentRequest?.url;
    if (url && url.trim() !== "") {
      const normalizedUrl = normalizeUrl(url);
      if (normalizedUrl !== url) {
        updateCurrentRequest({ url: normalizedUrl });

        const lowerUrl = normalizedUrl.toLowerCase();
        if (lowerUrl.startsWith("ws://") || lowerUrl.startsWith("wss://")) {
          if (requestType !== "websocket") {
            updateUIState({ requestType: "websocket" });
          }
        } else if (
          lowerUrl.startsWith("http://") ||
          lowerUrl.startsWith("https://")
        ) {
          if (requestType === "websocket") {
            updateUIState({ requestType: "http" });
          }
        }
      }
    }
  }, [currentRequest?.url, updateCurrentRequest, requestType, updateUIState]);

  const handleHeadersChange = useCallback(
    (headers: ReplayKeyValueItem[]) => {
      updateCurrentRequest({ headers });
    },
    [updateCurrentRequest],
  );

  const handleBodyTypeChange = useCallback(
    (type: BodyType) => {
      const body = currentRequest?.body || { type: "none" };
      updateCurrentRequest({ body: { ...body, type } });
    },
    [currentRequest, updateCurrentRequest],
  );

  const handleRawTypeChange = useCallback(
    (rawType: RawType) => {
      const body = currentRequest?.body || { type: "raw" };
      updateCurrentRequest({ body: { ...body, raw_type: rawType } });
    },
    [currentRequest, updateCurrentRequest],
  );

  const handleBodyContentChange = useCallback(
    (content: string) => {
      const body = currentRequest?.body || { type: "raw" };
      updateCurrentRequest({ body: { ...body, content } });
    },
    [currentRequest, updateCurrentRequest],
  );

  const handleFormDataChange = useCallback(
    (formData: ReplayKeyValueItem[]) => {
      const body = currentRequest?.body || { type: "form-data" };
      updateCurrentRequest({ body: { ...body, form_data: formData } });
    },
    [currentRequest, updateCurrentRequest],
  );

  const currentUrl = currentRequest?.url;
  const [queryParams, setQueryParams] = useState<ReplayKeyValueItem[]>([
    { id: generateId(), key: "", value: "", enabled: true },
  ]);
  const isUpdatingFromUrlRef = useRef(false);
  const isUpdatingFromParamsRef = useRef(false);
  const lastUrlRef = useRef<string | undefined>(undefined);

  /* eslint-disable react-hooks/set-state-in-effect -- Bidirectional sync between URL and query params requires setState in effect */
  useEffect(() => {
    if (isUpdatingFromParamsRef.current) {
      isUpdatingFromParamsRef.current = false;
      return;
    }

    if (currentUrl === lastUrlRef.current) {
      return;
    }
    lastUrlRef.current = currentUrl;

    isUpdatingFromUrlRef.current = true;

    if (!currentUrl) {
      setQueryParams([{ id: generateId(), key: "", value: "", enabled: true }]);
      isUpdatingFromUrlRef.current = false;
      return;
    }

    const parseUrl = (urlStr: string): URL | null => {
      try {
        return new URL(urlStr);
      } catch {
        const lowerUrl = urlStr.toLowerCase();
        if (
          !lowerUrl.startsWith("http://") &&
          !lowerUrl.startsWith("https://") &&
          !lowerUrl.startsWith("ws://") &&
          !lowerUrl.startsWith("wss://")
        ) {
          try {
            return new URL("http://" + urlStr);
          } catch {
            return null;
          }
        }
        return null;
      }
    };

    const url = parseUrl(currentUrl);
    if (url) {
      const params: ReplayKeyValueItem[] = [];
      url.searchParams.forEach((value, key) => {
        params.push({ id: generateId(), key, value, enabled: true });
      });
      if (params.length === 0) {
        params.push({ id: generateId(), key: "", value: "", enabled: true });
      }
      setQueryParams(params);
    }

    isUpdatingFromUrlRef.current = false;
  }, [currentUrl]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const handleQueryParamsChange = useCallback(
    (params: ReplayKeyValueItem[]) => {
      setQueryParams(params);

      if (isUpdatingFromUrlRef.current) {
        return;
      }

      isUpdatingFromParamsRef.current = true;

      const currentUrlValue = currentUrl || "";
      let baseUrl = currentUrlValue;
      try {
        const qIndex = baseUrl.indexOf("?");
        if (qIndex !== -1) baseUrl = baseUrl.substring(0, qIndex);

        const enabledParams = params.filter((p) => p.enabled && p.key.trim());
        if (enabledParams.length === 0) {
          lastUrlRef.current = baseUrl;
          updateCurrentRequest({ url: baseUrl });
          return;
        }

        const searchParams = new URLSearchParams();
        enabledParams.forEach((p) => searchParams.append(p.key, p.value));
        const newUrl = `${baseUrl}?${searchParams.toString()}`;
        lastUrlRef.current = newUrl;
        updateCurrentRequest({ url: newUrl });
      } catch {
        // ignore
      }
    },
    [currentUrl, updateCurrentRequest],
  );

  const handleSave = useCallback(async () => {
    if (!currentRequest?.url) {
      message.warning("Please enter a URL first");
      return;
    }
    if (currentRequest.is_saved) {
      await saveRequest();
    } else {
      setSaveName(currentRequest.name || "");
      setSaveModalVisible(true);
    }
  }, [currentRequest, saveRequest, setSaveName, setSaveModalVisible]);

  const handleSaveConfirm = useCallback(async () => {
    const success = await saveRequest(saveName || `Request ${Date.now()}`);
    if (success) {
      setSaveModalVisible(false);
    }
  }, [saveRequest, saveName, setSaveModalVisible]);

  const handleSend = useCallback(() => {
    if (!currentRequest?.url) {
      message.warning("Please enter a URL");
      return;
    }

    let urlToUse = currentRequest.url;
    if (!hasValidProtocol(urlToUse)) {
      urlToUse = normalizeUrl(urlToUse);
      updateCurrentRequest({ url: urlToUse });
    }

    if (!isValidUrl(urlToUse)) {
      message.warning(
        "Please enter a valid URL with protocol (http://, https://, ws://, or wss://)",
      );
      return;
    }

    const lowerUrl = urlToUse.toLowerCase();
    if (lowerUrl.startsWith("ws://") || lowerUrl.startsWith("wss://")) {
      connectWebSocket();
    } else {
      executeRequest();
    }
  }, [currentRequest, executeRequest, updateCurrentRequest, connectWebSocket]);

  const handleRuleModeChange = useCallback(
    (mode: RuleMode) => {
      if (mode === "selected") {
        fetchRules();
        setRuleSelectVisible(true);
      } else {
        setRuleConfig({ mode, selected_rules: [] });
      }
    },
    [setRuleConfig, fetchRules, setRuleSelectVisible],
  );

  const handleTimeoutChange = useCallback(
    (value: number | null) => {
      setTimeoutMs(value ?? DEFAULT_TIMEOUT_MS);
    },
    [setTimeoutMs],
  );

  const getRuleModeLabel = () => {
    switch (ruleConfig.mode) {
      case "enabled":
        return "Enabled Rules";
      case "selected":
        return `${ruleConfig.selected_rules?.length || 0} Rules`;
      case "none":
        return "No Rules";
      default:
        return "Rules";
    }
  };

  const ruleMenuItems: MenuProps["items"] = [
    { key: "enabled", label: "Use Enabled Rules" },
    { key: "selected", label: "Select Rules..." },
    { key: "none", label: "No Rules" },
  ];

  const enabledHeadersCount = (currentRequest?.headers || []).filter(
    (h) => h.enabled && h.key,
  ).length;
  const enabledParamsCount = queryParams.filter(
    (p) => p.enabled && p.key,
  ).length;

  const styles: Record<string, CSSProperties> = {
    container: {
      display: "flex",
      flexDirection: "column",
      height: "100%",
      overflow: "hidden",
    },
    urlBar: {
      display: "flex",
      alignItems: "center",
      gap: 8,
      padding: "8px 12px",
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      backgroundColor: token.colorBgLayout,
    },
    tabsContainer: {
      flex: 1,
      display: "flex",
      flexDirection: "column",
      overflow: "hidden",
    },
  };

  const tabItems = [
    {
      key: "params",
      label: `Params${enabledParamsCount > 0 ? ` (${enabledParamsCount})` : ""}`,
      children: (
        <KeyValueEditor
          items={queryParams}
          onChange={handleQueryParamsChange}
          keyPlaceholder="Parameter"
          valuePlaceholder="Value"
        />
      ),
    },
    {
      key: "headers",
      label: `Headers${enabledHeadersCount > 0 ? ` (${enabledHeadersCount})` : ""}`,
      children: (
        <KeyValueEditor
          items={currentRequest?.headers || []}
          onChange={handleHeadersChange}
          keyPlaceholder="Header"
          valuePlaceholder="Value"
        />
      ),
    },
    {
      key: "body",
      label: "Body",
      children: (
        <BodyEditor
          body={currentRequest?.body}
          onTypeChange={handleBodyTypeChange}
          onRawTypeChange={handleRawTypeChange}
          onContentChange={handleBodyContentChange}
          onFormDataChange={handleFormDataChange}
        />
      ),
    },
  ];

  return (
    <div style={styles.container}>
      <div style={styles.urlBar}>
        <Select
          value={currentRequest?.method || "GET"}
          onChange={handleMethodChange}
          style={{ width: 100 }}
          dropdownMatchSelectWidth={false}
          options={HTTP_METHODS.map((m) => ({
            label: (
              <span
                style={{
                  color: METHOD_COLORS[m],
                  fontWeight: 600,
                  fontSize: 12,
                }}
              >
                {m}
              </span>
            ),
            value: m,
          }))}
        />
        <Input
          placeholder="Enter request URL or paste cURL"
          value={currentRequest?.url || ""}
          onChange={handleUrlChange}
          onPaste={handleUrlPaste}
          onBlur={handleUrlBlur}
          style={{ flex: 1, fontSize: 12 }}
          onPressEnter={handleSend}
        />
        <Dropdown
          menu={{
            items: ruleMenuItems,
            onClick: ({ key }) => handleRuleModeChange(key as RuleMode),
          }}
          trigger={["click"]}
        >
          <Button icon={<SettingOutlined />} size="small">
            {getRuleModeLabel()}
            <CaretDownOutlined />
          </Button>
        </Dropdown>
        {requestType !== "websocket" && (
          <Tooltip title="Request Timeout (ms)">
            <InputNumber
              prefix={
                <ClockCircleOutlined
                  style={{ color: token.colorTextSecondary }}
                />
              }
              value={timeoutMs}
              onChange={handleTimeoutChange}
              min={1000}
              max={300000}
              step={1000}
              size="small"
              style={{ width: 120 }}
              addonAfter="ms"
            />
          </Tooltip>
        )}
        <Tooltip title="Save Request">
          <Button icon={<SaveOutlined />} onClick={handleSave} size="small">
            Save
          </Button>
        </Tooltip>
        {requestType === "websocket" ? (
          isConnected ? (
            <Button
              danger
              icon={<DisconnectOutlined />}
              onClick={disconnectWebSocket}
              size="small"
            >
              Disconnect
            </Button>
          ) : isConnecting ? (
            <Button
              danger
              icon={<StopOutlined />}
              onClick={cancelRequest}
              size="small"
            >
              Cancel
            </Button>
          ) : (
            <Button
              type="primary"
              icon={<LinkOutlined />}
              onClick={connectWebSocket}
              size="small"
            >
              Connect
            </Button>
          )
        ) : isConnected ? (
          <Button
            danger
            icon={<DisconnectOutlined />}
            onClick={disconnectSSE}
            size="small"
          >
            Disconnect
          </Button>
        ) : executing ? (
          <Button
            danger
            icon={<StopOutlined />}
            onClick={cancelRequest}
            size="small"
          >
            Cancel
          </Button>
        ) : (
          <Button
            type="primary"
            icon={<SendOutlined />}
            onClick={handleSend}
            size="small"
          >
            Send
          </Button>
        )}
      </div>

      <div style={styles.tabsContainer}>
        <Tabs
          className="replay-request-tabs"
          activeKey={activeTab}
          onChange={setActiveTab}
          items={tabItems}
          size="small"
          style={{ height: "100%" }}
          tabBarStyle={{
            margin: 0,
            padding: "0 12px",
            backgroundColor: token.colorBgLayout,
          }}
        />
      </div>

      <Modal
        title="Save Request"
        open={saveModalVisible}
        onOk={handleSaveConfirm}
        onCancel={() => setSaveModalVisible(false)}
        okText="Save"
      >
        <Input
          placeholder="Request name"
          value={saveName}
          onChange={(e) => setSaveName(e.target.value)}
          onPressEnter={handleSaveConfirm}
        />
      </Modal>

      <Modal
        title="Select Rules"
        open={ruleSelectVisible}
        onOk={() => setRuleSelectVisible(false)}
        onCancel={() => setRuleSelectVisible(false)}
        width={500}
      >
        <Select
          mode="multiple"
          placeholder="Select rules to apply"
          style={{ width: "100%" }}
          value={ruleConfig.selected_rules || []}
          onChange={(values) =>
            setRuleConfig({ mode: "selected", selected_rules: values })
          }
          options={rules.map((r) => ({
            label: r.enabled ? r.name : `${r.name} (disabled)`,
            value: r.name,
          }))}
        />
      </Modal>
    </div>
  );
}

interface KeyValueEditorProps {
  items: ReplayKeyValueItem[];
  onChange: (items: ReplayKeyValueItem[]) => void;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
}

interface KeyValueTableItem extends ReplayKeyValueItem {
  tableKey: string;
}

function KeyValueEditor({
  items,
  onChange,
  keyPlaceholder = "Key",
  valuePlaceholder = "Value",
}: KeyValueEditorProps) {
  const { token } = theme.useToken();

  const handleAdd = useCallback(() => {
    onChange([
      ...items,
      { id: generateId(), key: "", value: "", enabled: true },
    ]);
  }, [items, onChange]);

  const handleRemove = useCallback(
    (id: string) => {
      const newItems = items.filter((item) => item.id !== id);
      if (newItems.length === 0) {
        newItems.push({ id: generateId(), key: "", value: "", enabled: true });
      }
      onChange(newItems);
    },
    [items, onChange],
  );

  const handleChange = useCallback(
    (
      id: string,
      field: "key" | "value" | "enabled" | "description",
      value: string | boolean,
    ) => {
      onChange(
        items.map((item) =>
          item.id === id ? { ...item, [field]: value } : item,
        ),
      );
    },
    [items, onChange],
  );

  const dataSource: KeyValueTableItem[] = useMemo(() => {
    return items.map((item, index) => ({
      ...item,
      tableKey: item.id || String(index),
    }));
  }, [items]);

  const columns: ColumnsType<KeyValueTableItem> = [
    {
      title: "",
      dataIndex: "enabled",
      key: "enabled",
      width: 50,
      render: (enabled: boolean, record: KeyValueTableItem) => (
        <Switch
          size="small"
          checked={enabled}
          onChange={(checked) => handleChange(record.id, "enabled", checked)}
        />
      ),
    },
    {
      title: keyPlaceholder,
      dataIndex: "key",
      key: "key",
      render: (_: string, record: KeyValueTableItem) => (
        <Input
          size="small"
          placeholder={keyPlaceholder}
          value={record.key}
          onChange={(e) => handleChange(record.id, "key", e.target.value)}
          variant="borderless"
          style={{ fontFamily: "monospace", fontSize: 12 }}
        />
      ),
    },
    {
      title: valuePlaceholder,
      dataIndex: "value",
      key: "value",
      render: (_: string, record: KeyValueTableItem) => (
        <Input
          size="small"
          placeholder={valuePlaceholder}
          value={record.value}
          onChange={(e) => handleChange(record.id, "value", e.target.value)}
          variant="borderless"
          style={{ fontFamily: "monospace", fontSize: 12 }}
        />
      ),
    },
    {
      title: "Description",
      dataIndex: "description",
      key: "description",
      width: 180,
      render: (_: string, record: KeyValueTableItem) => (
        <Input
          size="small"
          placeholder="Description"
          value={record.description || ""}
          onChange={(e) =>
            handleChange(record.id, "description", e.target.value)
          }
          variant="borderless"
          style={{ fontSize: 12 }}
        />
      ),
    },
    {
      title: "",
      key: "actions",
      width: 50,
      render: (_: unknown, record: KeyValueTableItem) => (
        <Button
          type="text"
          size="small"
          icon={<DeleteOutlined />}
          onClick={() => handleRemove(record.id)}
          style={{ color: token.colorTextSecondary }}
        />
      ),
    },
  ];

  return (
    <div style={{ padding: 8, height: "100%", overflow: "auto", minHeight: 0 }}>
      <ConfigProvider
        theme={{
          components: {
            Table: {
              cellPaddingBlockSM: 4,
              cellPaddingInlineSM: 8,
            },
          },
        }}
      >
        <Table
          dataSource={dataSource}
          columns={columns}
          rowKey="tableKey"
          pagination={false}
          size="small"
          style={{
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
        />
      </ConfigProvider>
      <Button
        type="text"
        size="small"
        icon={<PlusOutlined />}
        onClick={handleAdd}
        style={{ marginTop: 8, color: token.colorTextSecondary, fontSize: 12 }}
      >
        Add
      </Button>
    </div>
  );
}

interface BodyEditorProps {
  body?: {
    type: BodyType;
    raw_type?: RawType;
    content?: string;
    form_data?: ReplayKeyValueItem[];
  };
  onTypeChange: (type: BodyType) => void;
  onRawTypeChange: (type: RawType) => void;
  onContentChange: (content: string) => void;
  onFormDataChange: (formData: ReplayKeyValueItem[]) => void;
}

function BodyEditor({
  body,
  onTypeChange,
  onRawTypeChange,
  onContentChange,
  onFormDataChange,
}: BodyEditorProps) {
  const { token } = theme.useToken();
  const bodyType = body?.type || "none";

  const styles: Record<string, CSSProperties> = {
    container: {
      display: "flex",
      flexDirection: "column",
      height: "100%",
    },
    typeBar: {
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      padding: "8px 12px",
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
    },
    typeOption: {
      display: "flex",
      alignItems: "center",
      gap: 6,
      cursor: "pointer",
      fontSize: 12,
      color: token.colorTextSecondary,
      padding: "4px 8px",
      borderRadius: 4,
    },
    content: {
      flex: 1,
      overflow: "auto",
      padding: 8,
    },
    noBody: {
      color: token.colorTextTertiary,
      textAlign: "center",
      padding: 40,
      fontSize: 12,
      backgroundColor: token.colorBgLayout,
      borderRadius: 4,
    },
  };

  return (
    <div style={styles.container}>
      <div style={styles.typeBar}>
        <Space size={12}>
          {(
            ["none", "form-data", "x-www-form-urlencoded", "raw"] as BodyType[]
          ).map((type) => (
            <label key={type} style={styles.typeOption}>
              <input
                type="radio"
                name="bodyType"
                checked={bodyType === type}
                onChange={() => onTypeChange(type)}
                style={{ accentColor: token.colorPrimary }}
              />
              <span
                style={{
                  color: bodyType === type ? token.colorPrimary : undefined,
                  fontWeight: bodyType === type ? 500 : 400,
                }}
              >
                {type}
              </span>
            </label>
          ))}
        </Space>
        {bodyType === "raw" && (
          <Select
            value={body?.raw_type || "text"}
            onChange={onRawTypeChange}
            size="small"
            style={{ width: 100 }}
            options={[
              { label: "Text", value: "text" },
              { label: "JSON", value: "json" },
              { label: "XML", value: "xml" },
            ]}
          />
        )}
      </div>

      <div style={styles.content}>
        {bodyType === "none" && (
          <div style={styles.noBody}>This request does not have a body</div>
        )}
        {bodyType === "raw" && (
          <CodeEditor
            value={body?.content || ""}
            onChange={onContentChange}
            language={
              body?.raw_type === "json"
                ? "json"
                : body?.raw_type === "xml"
                  ? "xml"
                  : "plaintext"
            }
            placeholder="Enter request body"
            minHeight={250}
          />
        )}
        {(bodyType === "form-data" || bodyType === "x-www-form-urlencoded") && (
          <KeyValueEditor
            items={
              body?.form_data || [
                { id: generateId(), key: "", value: "", enabled: true },
              ]
            }
            onChange={onFormDataChange}
            keyPlaceholder="Key"
            valuePlaceholder="Value"
          />
        )}
      </div>
    </div>
  );
}
