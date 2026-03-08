import { useEffect, useMemo, useCallback, useState } from "react";
import { Empty, Splitter, Spin } from "antd";
import type { CSSProperties } from "react";
import type {
  TrafficRecord,
  DisplayFormat,
  RecordContentType,
} from "../../types";
import { useTrafficDetailStore } from "../../stores/useTrafficDetailStore";
import { getContentTypeFromHeader } from "./helper/contentType";
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

interface TrafficDetailProps {
  record: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  loading?: boolean;
  error?: string | null;
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

const COLLAPSED_HEIGHT = 32;

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
}: TrafficDetailProps) {
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
  }, [record?.id]);

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

  const requestContentType = useMemo<RecordContentType>(() => {
    return getContentTypeFromHeader(record?.request_content_type);
  }, [record?.request_content_type]);

  const responseContentType = useMemo<RecordContentType>(() => {
    return getContentTypeFromHeader(record?.content_type);
  }, [record?.content_type]);

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
          />
        ),
      },
      {
        key: "Body",
        label: "Body",
        enable: !!responseBody,
        children: (
          <Body
            data={responseBody}
            contentType={responseContentType}
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
    responseBody,
    responseSearch,
    setResponseSearch,
    responseContentType,
    responseDisplayFormat,
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

  const hasCollapsed = requestCollapsed || responseCollapsed;
  const requestPanelSize = requestCollapsed ? COLLAPSED_HEIGHT : undefined;
  const responsePanelSize = responseCollapsed ? COLLAPSED_HEIGHT : undefined;
  const requestPanelProps = hasCollapsed
    ? { size: requestPanelSize, resizable: false }
    : { min: "20%", max: "80%", defaultSize: "50%" };
  const responsePanelProps = hasCollapsed
    ? { size: responsePanelSize, resizable: false }
    : {};

  return (
    <div style={styles.container} data-testid="traffic-detail">
      <Header record={record} />
      <div style={styles.splitterWrapper}>
        <Splitter layout="vertical">
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
                collapsed={requestCollapsed}
                onCollapsedChange={handleRequestCollapsedChange}
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
                contentType={responseContentType}
                collapsed={responseCollapsed}
                onCollapsedChange={handleResponseCollapsedChange}
                keepAliveTabs={["Messages"]}
                contentOverflow={responseTab === "Messages" ? "hidden" : "auto"}
              />
            </div>
          </Splitter.Panel>
        </Splitter>
      </div>
    </div>
  );
}
