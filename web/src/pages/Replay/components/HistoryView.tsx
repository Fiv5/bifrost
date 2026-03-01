import { useMemo, useCallback, useRef } from "react";
import { Input, Empty, Spin, Tag, theme, Typography } from "antd";
import { SearchOutlined } from "@ant-design/icons";
import { useReplayStore } from "../../../stores/useReplayStore";
import TrafficDetail from "../../../components/TrafficDetail";
import type { ReplayHistory } from "../../../types";

const { useToken } = theme;
const { Text } = Typography;

const methodColors: Record<string, string> = {
  GET: "#52c41a",
  POST: "#1677ff",
  PUT: "#faad14",
  DELETE: "#ff4d4f",
  PATCH: "#722ed1",
  HEAD: "#13c2c2",
  OPTIONS: "#eb2f96",
};

const statusColors = (status?: number) => {
  if (!status) return "default";
  if (status >= 200 && status < 300) return "success";
  if (status >= 300 && status < 400) return "warning";
  if (status >= 400) return "error";
  return "default";
};

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp);
  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const minutes = Math.floor(diff / 60000);
  const hours = Math.floor(diff / 3600000);
  const days = Math.floor(diff / 86400000);

  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 7) return `${days}d ago`;
  return date.toLocaleDateString();
};

const formatDuration = (ms?: number) => {
  if (ms === undefined || ms === null) return "-";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
};

const extractPath = (url: string) => {
  try {
    const parsed = new URL(url);
    return parsed.pathname + parsed.search;
  } catch {
    return url;
  }
};

interface HistoryItemProps {
  item: ReplayHistory;
  isSelected: boolean;
  onClick: () => void;
}

const HistoryItem = ({ item, isSelected, onClick }: HistoryItemProps) => {
  const { token } = useToken();

  return (
    <div
      onClick={onClick}
      style={{
        padding: "6px 12px",
        cursor: "pointer",
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        backgroundColor: isSelected ? token.colorBgTextHover : "transparent",
        transition: "background-color 0.2s",
      }}
      onMouseEnter={(e) => {
        if (!isSelected) {
          e.currentTarget.style.backgroundColor = token.colorBgTextHover;
        }
      }}
      onMouseLeave={(e) => {
        if (!isSelected) {
          e.currentTarget.style.backgroundColor = "transparent";
        }
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          marginBottom: 2,
        }}
      >
        <Tag
          style={{
            margin: 0,
            color: methodColors[item.method] || token.colorText,
            backgroundColor: "transparent",
            border: "none",
            padding: 0,
            fontWeight: 600,
            fontSize: 11,
            flexShrink: 0,
          }}
        >
          {item.method}
        </Tag>
        <Text
          ellipsis
          style={{
            fontSize: 12,
            flex: 1,
            minWidth: 0,
          }}
          title={item.url}
        >
          {extractPath(item.url)}
        </Text>
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <Tag
          color={statusColors(item.status)}
          style={{ margin: 0, fontSize: 10, lineHeight: "16px" }}
        >
          {item.status || "Pending"}
        </Tag>
        <Text type="secondary" style={{ fontSize: 10 }}>
          {formatDuration(item.duration_ms)}
        </Text>
        <Text
          type="secondary"
          style={{ fontSize: 10, marginLeft: "auto", flexShrink: 0 }}
        >
          {formatTime(item.executed_at)}
        </Text>
      </div>
    </div>
  );
};

export const HistoryView = () => {
  const { token } = useToken();
  const listRef = useRef<HTMLDivElement>(null);

  const {
    currentRequest,
    allHistory,
    allHistoryTotal,
    loading,
    historyDetailLoading,
    selectedHistoryRecord,
    historyRequestBody,
    historyResponseBody,
    uiState,
    updateUIState,
    selectHistoryForDetail,
  } = useReplayStore();

  const searchText = uiState.historySearchText;
  const selectedHistoryId = uiState.selectedHistoryId;

  const filteredHistory = useMemo(() => {
    if (!searchText) return allHistory;
    const lower = searchText.toLowerCase();
    return allHistory.filter(
      (h) =>
        h.url.toLowerCase().includes(lower) ||
        h.method.toLowerCase().includes(lower),
    );
  }, [allHistory, searchText]);

  const handleSearchChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      updateUIState({ historySearchText: e.target.value });
    },
    [updateUIState],
  );

  const handleSelectHistory = useCallback(
    (history: ReplayHistory) => {
      selectHistoryForDetail(history);
    },
    [selectHistoryForDetail],
  );

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      <div
        style={{
          width: 300,
          minWidth: 240,
          maxWidth: 400,
          borderRight: `1px solid ${token.colorBorderSecondary}`,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            padding: "12px",
            borderBottom: `1px solid ${token.colorBorderSecondary}`,
          }}
        >
          {currentRequest && (
            <div
              style={{
                marginBottom: 8,
                padding: "6px 8px",
                backgroundColor: token.colorBgLayout,
                borderRadius: 4,
                fontSize: 12,
              }}
            >
              <Text type="secondary" style={{ fontSize: 10 }}>
                Template:
              </Text>
              <div style={{ fontWeight: 500 }}>
                {currentRequest.name || "Unnamed"}
              </div>
            </div>
          )}
          <Input
            prefix={<SearchOutlined />}
            placeholder="Search history..."
            allowClear
            value={searchText}
            onChange={handleSearchChange}
            style={{ width: "100%" }}
          />
          <div
            style={{
              marginTop: 8,
              fontSize: 11,
              color: token.colorTextSecondary,
            }}
          >
            {filteredHistory.length} of {allHistoryTotal} records
          </div>
        </div>

        <div
          ref={listRef}
          style={{
            flex: 1,
            overflow: "auto",
          }}
        >
          {loading ? (
            <div
              style={{ display: "flex", justifyContent: "center", padding: 24 }}
            >
              <Spin />
            </div>
          ) : filteredHistory.length === 0 ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No history records"
              style={{ marginTop: 40 }}
            />
          ) : (
            filteredHistory.map((item) => (
              <HistoryItem
                key={item.id}
                item={item}
                isSelected={selectedHistoryId === item.id}
                onClick={() => handleSelectHistory(item)}
              />
            ))
          )}
        </div>
      </div>

      <div style={{ flex: 1, overflow: "hidden" }}>
        {historyDetailLoading ? (
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              alignItems: "center",
              height: "100%",
            }}
          >
            <Spin size="large" />
          </div>
        ) : selectedHistoryRecord ? (
          <TrafficDetail
            record={selectedHistoryRecord}
            requestBody={historyRequestBody}
            responseBody={historyResponseBody}
          />
        ) : (
          <Empty
            description="Select a history record to view details"
            style={{
              display: "flex",
              flexDirection: "column",
              justifyContent: "center",
              height: "100%",
            }}
          />
        )}
      </div>
    </div>
  );
};

export default HistoryView;
