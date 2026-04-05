import { useEffect, useMemo, useCallback, useState } from "react";
import { Empty, Splitter, Spin } from "antd";
import type { CSSProperties } from "react";
import type {
  TrafficRecord,
  DisplayFormat,
  RecordContentType,
  SSEEvent,
} from "../../types";
import { useTrafficDetailStore } from "../../stores/useTrafficDetailStore";
import {
  getContentTypeFromHeader,
  isImageContentType,
  shouldDisableJsonStructuredView,
} from "./helper/contentType";
import { Header } from "./Header";
import { Panel } from "./Panel";
import { Overview } from "./panes/Overview";
import { HeaderView } from "./panes/Header";
import { Body } from "./panes/Body";
import { Raw } from "./panes/Raw";
import { CookieView } from "./panes/Cookie";
import { QueryView } from "./panes/Query";
import { Messages } from "./panes/Messages";
import ScriptLogsPane from "./panes/ScriptLogs";
import { getResponseBodyContentUrl } from "../../api/traffic";
import { parseSseTextToEvents } from "../VirtualMessageViewer";
import {
  assembleOpenAiLikeSse,
  parseOpenAiLikeJsonResponse,
  assembleTraeLikeSse,
  assembleDouBaoLikeSse,
  parseOpenAiLikeRequest,
  OpenAiChatView,
  SseResponseView,
} from "../AiResponse";

interface TrafficDetailProps {
  record: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  loading?: boolean;
  error?: string | null;
  onOpenInNewWindow?: ((record: TrafficRecord) => void) | undefined;
  onResponseBodyChange?: ((body: string | null, recordId: string) => void) | undefined;
}

const hasQueryParams = (url: string): boolean => {
  try {
    const urlObj = new URL(url);
    return urlObj.searchParams.toString().length > 0;
  } catch {
    return false;
  }
};

const hasCookies = (headers: [string, string][] | null): boolean => {
  if (!headers) return false;
  return headers.some(([name]) => name.toLowerCase() === "cookie");
};

const hasSetCookies = (headers: [string, string][] | null): boolean => {
  if (!headers) return false;
  return headers.some(([name]) => name.toLowerCase() === "set-cookie");
};

const COLLAPSED_HEIGHT = 28;

const styles: Record<string, CSSProperties> = {
  container: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
  },
  emptyContainer: {
    height: "100%",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  splitterWrapper: {
    flex: 1,
    overflow: "hidden",
    minHeight: 0,
  },
  panelWrapper: {
    height: "100%",
    padding: "0 8px",
    overflow: "hidden",
  },
};

export default function TrafficDetail({
  record,
  requestBody,
  responseBody,
  loading,
  error,
  onOpenInNewWindow,
  onResponseBodyChange,
}: TrafficDetailProps) {
  const [expandedRequestPanelSize, setExpandedRequestPanelSize] = useState<
    number | string
  >("50%");
  const [liveSseEvents, setLiveSseEvents] = useState<SSEEvent[]>([]);
  const [hasAutoOpenedOpenAiTab, setHasAutoOpenedOpenAiTab] = useState(false);
  const [hasAutoOpenedRequestOpenAiTab, setHasAutoOpenedRequestOpenAiTab] = useState(false);
  const [hasAutoOpenedTraeTab, setHasAutoOpenedTraeTab] = useState(false);
  const [hasAutoOpenedDouBaoTab, setHasAutoOpenedDouBaoTab] = useState(false);
  const {
    requestSearch,
    responseSearch,
    requestDisplayFormat,
    responseDisplayFormat,
    requestTab,
    responseTab,
    requestPreferredTab,
    responsePreferredTab,
    requestCollapsed,
    responseCollapsed,
    setRequestSearch,
    setResponseSearch,
    setRequestDisplayFormat,
    setResponseDisplayFormat,
    setRequestTab,
    setResponseTab,
    setRequestPreferredTab,
    setResponsePreferredTab,
    setRequestCollapsed,
    setResponseCollapsed,
    reset,
  } = useTrafficDetailStore();
  const [liveSseCount, setLiveSseCount] = useState<number | null>(null);

  useEffect(() => {
    reset();
  }, [record?.id, reset]);

  useEffect(() => {
    setLiveSseCount(null);
    setLiveSseEvents([]);
    setHasAutoOpenedOpenAiTab(false);
    setHasAutoOpenedRequestOpenAiTab(false);
    setHasAutoOpenedTraeTab(false);
    setHasAutoOpenedDouBaoTab(false);
  }, [record?.id]);

  const responseContentType = useMemo<RecordContentType>(() => {
    return getContentTypeFromHeader(record?.content_type);
  }, [record?.content_type]);

  const canPreviewResponseImage = useMemo(() => {
    return (
      responseContentType === "Media" &&
      isImageContentType(record?.content_type) &&
      !!record?.id
    );
  }, [record?.content_type, record?.id, responseContentType]);

  useEffect(() => {
    if (!record) return;
    setResponseDisplayFormat(
      responseContentType === "Media" && isImageContentType(record.content_type)
        ? "Media"
        : "HighLight"
    );
  }, [
    record?.id,
    record,
    responseContentType,
    setResponseDisplayFormat,
  ]);

  const requestContentType = useMemo<RecordContentType>(() => {
    return getContentTypeFromHeader(record?.request_content_type);
  }, [record?.request_content_type]);

  const openAiRequestParsed = useMemo(() => {
    return parseOpenAiLikeRequest(requestBody);
  }, [requestBody]);

  useEffect(() => {
    if (
      requestDisplayFormat === "Tree" &&
      shouldDisableJsonStructuredView(requestContentType, requestBody)
    ) {
      setRequestDisplayFormat("HighLight");
    }
  }, [
    requestBody,
    requestContentType,
    requestDisplayFormat,
    setRequestDisplayFormat,
  ]);

  const handleRequestTabChange = useCallback(
    (tab: string) => {
      setRequestTab(tab);
      setRequestPreferredTab(tab);
    },
    [setRequestTab, setRequestPreferredTab],
  );

  const handleResponseTabChange = useCallback(
    (tab: string) => {
      setResponseTab(tab);
      setResponsePreferredTab(tab);
    },
    [setResponseTab, setResponsePreferredTab],
  );

  const handleRequestDisplayFormatChange = useCallback(
    (format: string) => {
      setRequestDisplayFormat(format as DisplayFormat);
    },
    [setRequestDisplayFormat],
  );

  const handleResponseDisplayFormatChange = useCallback(
    (format: string) => {
      setResponseDisplayFormat(format as DisplayFormat);
    },
    [setResponseDisplayFormat],
  );

  const handleRequestCollapsedChange = useCallback(
    (collapsed: boolean) => {
      if (collapsed) {
        setRequestCollapsed(true);
        setResponseCollapsed(false);
      } else {
        setRequestCollapsed(false);
      }
    },
    [setRequestCollapsed, setResponseCollapsed],
  );

  const handleResponseCollapsedChange = useCallback(
    (collapsed: boolean) => {
      if (collapsed) {
        setResponseCollapsed(true);
        setRequestCollapsed(false);
      } else {
        setResponseCollapsed(false);
      }
    },
    [setRequestCollapsed, setResponseCollapsed],
  );

  const requestTabs = useMemo(() => {
    if (!record) return [];

    return [
      {
        key: "Overview",
        label: "Overview",
        children: (
          <Overview
            record={record}
            searchValue={requestSearch}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "Header",
        label: "Header",
        children: (
          <HeaderView
            headers={record.request_headers}
            originalHeaders={record.original_request_headers}
            testIdPrefix="request-header-view"
            searchValue={requestSearch}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "Cookie",
        label: "Cookie",
        enable: hasCookies(record.request_headers),
        children: (
          <CookieView
            headers={record.request_headers}
            type="request"
            searchValue={requestSearch}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "Query",
        label: "Query",
        enable: hasQueryParams(record.url),
        children: (
          <QueryView
            url={record.url}
            searchValue={requestSearch}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "OpenAI",
        label: "OpenAI",
        enable: !!openAiRequestParsed,
        children: openAiRequestParsed ? (
          <OpenAiChatView parsed={openAiRequestParsed} />
        ) : null,
      },
      {
        key: "Body",
        label: "Body",
        enable: !!requestBody,
        children: (
          <Body
            data={requestBody}
            contentType={requestContentType}
            searchValue={requestSearch}
            displayFormat={requestDisplayFormat}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "Raw",
        label: "Raw",
        children: (
          <Raw
            type="request"
            method={record.method}
            url={record.url}
            protocol={record.protocol}
            headers={record.request_headers}
            body={requestBody}
            searchValue={requestSearch}
            onSearch={setRequestSearch}
          />
        ),
      },
      {
        key: "Script",
        label: "Script",
        enable: !!(record.req_script_results && record.req_script_results.length > 0),
        children: (
          <ScriptLogsPane results={record.req_script_results || []} />
        ),
      },
    ];
  }, [
    record,
    requestBody,
    requestSearch,
    setRequestSearch,
    requestContentType,
    requestDisplayFormat,
    openAiRequestParsed,
  ]);

  const openAiLikeAssembly = useMemo(() => {
    if (record?.is_sse) {
      const responseSseEvents = liveSseEvents.length > 0
        ? liveSseEvents
        : responseBody
          ? parseSseTextToEvents(responseBody)
          : [];
      return assembleOpenAiLikeSse(responseSseEvents);
    }

    return parseOpenAiLikeJsonResponse(responseBody);
  }, [liveSseEvents, record?.is_sse, responseBody]);

  const traeLikeAssembly = useMemo(() => {
    if (!record?.is_sse) {
      return null;
    }
    if (openAiLikeAssembly) {
      return null;
    }

    const responseSseEvents = liveSseEvents.length > 0
      ? liveSseEvents
      : responseBody
        ? parseSseTextToEvents(responseBody)
        : [];
    return assembleTraeLikeSse(responseSseEvents);
  }, [liveSseEvents, openAiLikeAssembly, record?.is_sse, responseBody]);

  const douBaoLikeAssembly = useMemo(() => {
    if (!record?.is_sse) {
      return null;
    }
    if (openAiLikeAssembly || traeLikeAssembly) {
      return null;
    }

    const responseSseEvents = liveSseEvents.length > 0
      ? liveSseEvents
      : responseBody
        ? parseSseTextToEvents(responseBody)
        : [];
    return assembleDouBaoLikeSse(responseSseEvents);
  }, [liveSseEvents, openAiLikeAssembly, traeLikeAssembly, record?.is_sse, responseBody]);

  const responsePanelContentType = responseTab === "OpenAI"
    ? openAiLikeAssembly?.contentType ?? "Other"
    : responseContentType;
  const responsePanelBodyData = responseTab === "OpenAI"
    ? openAiLikeAssembly?.body ?? null
    : responseBody;

  useEffect(() => {
    if (
      responseDisplayFormat === "Tree" &&
      shouldDisableJsonStructuredView(responsePanelContentType, responsePanelBodyData)
    ) {
      setResponseDisplayFormat("HighLight");
    }
  }, [
    responseDisplayFormat,
    responsePanelBodyData,
    responsePanelContentType,
    setResponseDisplayFormat,
  ]);

  const responseTabs = useMemo(() => {
    if (!record) return [];
    const hasMessages = record.is_websocket || record.is_sse;
    const socketCount = record.socket_status?.frame_count ?? record.frame_count ?? 0;
    const messageCount = record.is_sse ? liveSseCount ?? socketCount : socketCount;
    const isSseOpen = record.is_sse
      ? record.socket_status?.is_open ?? !record.end_time
      : false;

    return [
      {
        key: "Header",
        label: "Header",
        children: (
          <HeaderView
            headers={record.response_headers}
            actualHeaders={record.actual_response_headers}
            testIdPrefix="response-header-view"
            searchValue={responseSearch}
            onSearch={setResponseSearch}
            isTunnel={record.is_tunnel}
            host={record.host}
            clientApp={record.client_app}
          />
        ),
      },
      {
        key: "Set-Cookie",
        label: "Set-Cookie",
        enable: hasSetCookies(record.response_headers),
        children: (
          <CookieView
            headers={record.response_headers}
            type="response"
            searchValue={responseSearch}
            onSearch={setResponseSearch}
          />
        ),
      },
      {
        key: "Messages",
        label: `Messages${messageCount > 0 ? ` (${messageCount})` : ""}`,
        enable: hasMessages,
        children: (
          <Messages
            recordId={record.id}
            isWebSocket={record.is_websocket || false}
            frameCount={record.frame_count ?? 0}
            isConnectionOpen={record.is_websocket ? record.socket_status?.is_open ?? false : isSseOpen}
            searchValue={responseSearch}
            onSearch={setResponseSearch}
            onSseCountChange={record.is_sse ? setLiveSseCount : undefined}
            onSseEventsChange={record.is_sse ? setLiveSseEvents : undefined}
            responseBodyOverride={responseBody}
            onResponseBodyChange={onResponseBodyChange}
          />
        ),
      },
      {
        key: "OpenAI",
        label: "OpenAI",
        enable: !!openAiLikeAssembly,
        children: openAiLikeAssembly ? (
          <SseResponseView
            body={openAiLikeAssembly.body}
            mode="openai"
          />
        ) : null,
      },
      {
        key: "Trae",
        label: "Trae",
        enable: !!traeLikeAssembly,
        children: traeLikeAssembly ? (
          <SseResponseView
            body={traeLikeAssembly.body}
            mode="trae"
          />
        ) : null,
      },
      {
        key: "DouBao",
        label: "DouBao",
        enable: !!douBaoLikeAssembly,
        children: douBaoLikeAssembly ? (
          <SseResponseView
            body={douBaoLikeAssembly.body}
            mode="doubao"
          />
        ) : null,
      },
      {
        key: "Body",
        label: "Body",
        enable: !!responseBody || canPreviewResponseImage,
        children: (
          <Body
            data={responseBody}
            contentType={responseContentType}
            rawContentType={record.content_type}
            mediaSrc={getResponseBodyContentUrl(record.id)}
            searchValue={responseSearch}
            displayFormat={responseDisplayFormat}
            onSearch={setResponseSearch}
          />
        ),
      },
      {
        key: "Raw",
        label: "Raw",
        children: (
          <Raw
            type="response"
            protocol={record.protocol}
            status={record.status}
            headers={record.response_headers}
            body={responseBody}
            searchValue={responseSearch}
            onSearch={setResponseSearch}
            isTunnel={record.is_tunnel}
            host={record.host}
            clientApp={record.client_app}
          />
        ),
      },
      {
        key: "Script",
        label: "Script",
        enable: !!(record.res_script_results && record.res_script_results.length > 0),
        children: (
          <ScriptLogsPane results={record.res_script_results || []} />
        ),
      },
    ];
  }, [
    record,
    liveSseCount,
    openAiLikeAssembly,
    traeLikeAssembly,
    douBaoLikeAssembly,
    responseBody,
    onResponseBodyChange,
    canPreviewResponseImage,
    responseSearch,
    setResponseSearch,
    responseContentType,
    responseDisplayFormat,
  ]);

  useEffect(() => {
    if (!openAiLikeAssembly) return;
    if (hasAutoOpenedOpenAiTab) return;
    if (responseTab === "OpenAI") {
      setHasAutoOpenedOpenAiTab(true);
      return;
    }

    setResponseTab("OpenAI");
    setHasAutoOpenedOpenAiTab(true);
  }, [
    hasAutoOpenedOpenAiTab,
    openAiLikeAssembly,
    responseTab,
    setResponseTab,
  ]);

  useEffect(() => {
    if (!record?.is_sse) return;
    if (!traeLikeAssembly) return;
    if (openAiLikeAssembly) return;
    if (hasAutoOpenedTraeTab) return;
    if (responseTab === "Trae") {
      setHasAutoOpenedTraeTab(true);
      return;
    }

    setResponseTab("Trae");
    setHasAutoOpenedTraeTab(true);
  }, [
    hasAutoOpenedTraeTab,
    traeLikeAssembly,
    openAiLikeAssembly,
    record?.is_sse,
    responseTab,
    setResponseTab,
  ]);

  useEffect(() => {
    if (!record?.is_sse) return;
    if (!douBaoLikeAssembly) return;
    if (openAiLikeAssembly || traeLikeAssembly) return;
    if (hasAutoOpenedDouBaoTab) return;
    if (responseTab === "DouBao") {
      setHasAutoOpenedDouBaoTab(true);
      return;
    }

    setResponseTab("DouBao");
    setHasAutoOpenedDouBaoTab(true);
  }, [
    hasAutoOpenedDouBaoTab,
    douBaoLikeAssembly,
    openAiLikeAssembly,
    traeLikeAssembly,
    record?.is_sse,
    responseTab,
    setResponseTab,
  ]);

  useEffect(() => {
    if (!openAiRequestParsed) return;
    if (hasAutoOpenedRequestOpenAiTab) return;
    if (requestTab === "OpenAI") {
      setHasAutoOpenedRequestOpenAiTab(true);
      return;
    }

    setRequestTab("OpenAI");
    setHasAutoOpenedRequestOpenAiTab(true);
  }, [
    hasAutoOpenedRequestOpenAiTab,
    openAiRequestParsed,
    requestTab,
    setRequestTab,
  ]);

  useEffect(() => {
    if (!record) return;

    const requestEnabledTabs = requestTabs.filter(
      (tab) => tab.enable !== false,
    );
    const responseEnabledTabs = responseTabs.filter(
      (tab) => tab.enable !== false,
    );

    const calculateEffectiveTab = (
      preferredTab: string | null,
      enabledTabs: { key: string; enable?: boolean }[],
      defaultTab: string,
    ): string => {
      if (!preferredTab) return defaultTab;
      const preferredTabEnabled = enabledTabs.some(
        (tab) => tab.key === preferredTab,
      );
      return preferredTabEnabled
        ? preferredTab
        : (enabledTabs[0]?.key ?? defaultTab);
    };

    const effectiveRequestTab = calculateEffectiveTab(
      requestPreferredTab,
      requestEnabledTabs,
      "Overview",
    );
    const effectiveResponseTab = calculateEffectiveTab(
      responsePreferredTab,
      responseEnabledTabs,
      "Header",
    );

    if (effectiveRequestTab !== requestTab) {
      setRequestTab(effectiveRequestTab);
    }
    if (effectiveResponseTab !== responseTab) {
      setResponseTab(effectiveResponseTab);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    record?.id,
    requestTabs,
    responseTabs,
    requestPreferredTab,
    responsePreferredTab,
    requestTab,
    responseTab,
    setRequestTab,
    setResponseTab,
  ]);

  const hasCollapsed = requestCollapsed || responseCollapsed;
  const requestPanelSize = requestCollapsed ? COLLAPSED_HEIGHT : undefined;
  const responsePanelSize = responseCollapsed ? COLLAPSED_HEIGHT : undefined;
  const requestPanelProps = hasCollapsed
    ? { size: requestPanelSize, resizable: false }
    : { min: "20%", max: "80%", defaultSize: expandedRequestPanelSize };
  const responsePanelProps = hasCollapsed
    ? { size: responsePanelSize, resizable: false }
    : {};

  const handleResizeEnd = useCallback(
    (sizes: number[]) => {
      if (hasCollapsed || sizes.length < 2) {
        return;
      }
      setExpandedRequestPanelSize(sizes[0]);
    },
    [hasCollapsed],
  );

  if (!record) {
    if (loading) {
      return (
        <div style={styles.emptyContainer}>
          <Spin />
        </div>
      );
    }
    if (error) {
      return (
        <div style={styles.emptyContainer}>
          <Empty description={error} />
        </div>
      );
    }
    return (
      <div style={styles.emptyContainer}>
        <Empty description="Select a request to view details" />
      </div>
    );
  }

  return (
    <div style={styles.container} data-testid="traffic-detail">
      <Header
        record={record}
        requestBody={requestBody}
        onOpenInNewWindow={onOpenInNewWindow}
      />
      <div style={styles.splitterWrapper}>
        <Splitter
          layout="vertical"
          onResizeEnd={handleResizeEnd}
        >
          <Splitter.Panel {...requestPanelProps}>
            <div style={styles.panelWrapper}>
              <Panel
                name="Request"
                tabs={requestTabs}
                activeTab={requestTab}
                onTabChange={handleRequestTabChange}
                searchValue={requestSearch}
                onSearch={setRequestSearch}
                displayFormat={requestDisplayFormat}
                onDisplayFormatChange={handleRequestDisplayFormatChange}
                contentType={requestContentType}
                bodyData={requestBody}
                collapsed={requestCollapsed}
                onCollapsedChange={handleRequestCollapsedChange}
                keepAliveTabs={["Body", "OpenAI"]}
              />
            </div>
          </Splitter.Panel>
          <Splitter.Panel {...responsePanelProps}>
            <div style={styles.panelWrapper}>
              <Panel
                name="Response"
                tabs={responseTabs}
                activeTab={responseTab}
                onTabChange={handleResponseTabChange}
                searchValue={responseSearch}
                onSearch={setResponseSearch}
                displayFormat={responseDisplayFormat}
                onDisplayFormatChange={handleResponseDisplayFormatChange}
                contentType={responsePanelContentType}
                bodyData={responsePanelBodyData}
                collapsed={responseCollapsed}
                onCollapsedChange={handleResponseCollapsedChange}
                keepAliveTabs={["Body", "Messages", "OpenAI", "Trae", "DouBao"]}
                bodyFormatTabs={["Body"]}
                contentOverflow={responseTab === "Messages" ? "hidden" : "auto"}
              />
            </div>
          </Splitter.Panel>
        </Splitter>
      </div>
    </div>
  );
}
