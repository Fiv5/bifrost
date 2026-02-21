import { useEffect, useMemo, useCallback } from "react";
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

interface TrafficDetailProps {
  record: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  loading?: boolean;
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
  flexPanelsWrapper: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
    minHeight: 0,
  },
  collapsedPanel: {
    height: COLLAPSED_HEIGHT,
    flexShrink: 0,
    padding: "0 8px",
    overflow: "hidden",
  },
  expandedPanel: {
    flex: 1,
    minHeight: 0,
    padding: "0 8px",
    overflow: "hidden",
  },
};

export default function TrafficDetail({
  record,
  requestBody,
  responseBody,
  loading,
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

  useEffect(() => {
    reset();
  }, [record?.id, reset]);

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

    return [
      {
        key: "Header",
        label: "Header",
        children: (
          <HeaderView
            headers={record.response_headers}
            searchValue={responseSearch}
            onSearch={setResponseSearch}
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
        label: `Messages${(record.frame_count ?? 0) > 0 ? ` (${record.frame_count})` : ""}`,
        enable: hasMessages,
        children: (
          <Messages
            recordId={record.id}
            isWebSocket={record.is_websocket || false}
            frameCount={record.frame_count ?? 0}
            searchValue={responseSearch}
            onSearch={setResponseSearch}
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
          />
        ),
      },
    ];
  }, [
    record,
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
    return (
      <div style={styles.emptyContainer}>
        <Empty description="Select a request to view details" />
      </div>
    );
  }

  const hasCollapsed = requestCollapsed || responseCollapsed;

  const renderPanels = () => {
    if (hasCollapsed) {
      return (
        <div style={styles.flexPanelsWrapper}>
          <div
            style={
              requestCollapsed ? styles.collapsedPanel : styles.expandedPanel
            }
          >
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
          <div
            style={
              responseCollapsed ? styles.collapsedPanel : styles.expandedPanel
            }
          >
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
            />
          </div>
        </div>
      );
    }

    return (
      <div style={styles.splitterWrapper}>
        <Splitter layout="vertical">
          <Splitter.Panel min="20%" max="80%" defaultSize="50%">
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
          <Splitter.Panel>
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
              />
            </div>
          </Splitter.Panel>
        </Splitter>
      </div>
    );
  };

  return (
    <div style={styles.container}>
      <Header record={record} />
      {renderPanels()}
    </div>
  );
}
