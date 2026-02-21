import {
  useRef,
  useEffect,
  useCallback,
  useState,
  type CSSProperties,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Tag, Typography, Tooltip, Badge, theme } from "antd";
import { ThunderboltOutlined, ArrowDownOutlined } from "@ant-design/icons";
import type { TrafficSummary } from "../../types";
import AppIcon from "../AppIcon";

const { Text } = Typography;

interface VirtualTrafficTableProps {
  data: TrafficSummary[];
  onSelect?: (record: TrafficSummary) => void;
  selectedId?: string;
  onLoadMore?: () => void;
  hasMore?: boolean;
  autoScroll?: boolean;
  onScrollPositionChange?: (isAtBottom: boolean) => void;
  newRecordsCount?: number;
  onScrollToBottom?: () => void;
  onKeyboardNavigate?: (record: TrafficSummary) => void;
  initialScrollTop?: number;
  onScrollTopChange?: (scrollTop: number) => void;
}

const ROW_HEIGHT = 36;
const SCROLL_THRESHOLD = 50;
const TABLE_MIN_WIDTH = 1340;

const getStatusDotColor = (status: number): string => {
  if (status === 0) return "#d9d9d9";
  if (status >= 100 && status < 200) return "#73d13d";
  if (status >= 200 && status < 300) return "#52c41a";
  if (status >= 300 && status < 400) return "#faad14";
  if (status >= 400 && status < 500) return "#fa8c16";
  if (status >= 500) return "#f5222d";
  return "#d9d9d9";
};

const getStatusColor = (status: number) => {
  if (status >= 500) return "error";
  if (status >= 400) return "warning";
  if (status >= 300) return "processing";
  if (status >= 200) return "success";
  return "default";
};

const getMethodColor = (method: string) => {
  const colors: Record<string, string> = {
    GET: "green",
    POST: "blue",
    PUT: "orange",
    DELETE: "red",
    PATCH: "purple",
    OPTIONS: "default",
    HEAD: "cyan",
    CONNECT: "magenta",
  };
  return colors[method.toUpperCase()] || "default";
};

const formatSize = (bytes: number) => {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
};

const formatSequence = (seq: number): string => {
  return seq.toString().padStart(4, "0");
};

interface ColumnDef {
  key: string;
  title: string;
  width: number | string;
  minWidth?: number;
  align?: "left" | "center" | "right";
  render: (record: TrafficSummary) => React.ReactNode;
}

const columns: ColumnDef[] = [
  {
    key: "sequence",
    title: "#",
    width: 50,
    align: "right",
    render: (record) => (
      <Text type="secondary" style={{ fontSize: 11, fontFamily: "monospace" }}>
        {formatSequence(record.sequence)}
      </Text>
    ),
  },
  {
    key: "status_dot",
    title: "",
    width: 24,
    align: "center",
    render: (record) => (
      <Tooltip
        title={
          record.status === 0
            ? "Pending"
            : record.has_rule_hit
              ? `${record.status} - ${record.matched_rule_count} rule(s) matched`
              : `Status: ${record.status}`
        }
      >
        <div
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            backgroundColor: getStatusDotColor(record.status),
            transition: "background-color 0.3s",
          }}
        />
      </Tooltip>
    ),
  },
  {
    key: "protocol",
    title: "Protocol",
    width: 60,
    render: (record) => (
      <Text type="secondary" style={{ fontSize: 11 }}>
        {record.protocol?.replace("HTTP/", "") || "-"}
      </Text>
    ),
  },
  {
    key: "method",
    title: "Method",
    width: 70,
    render: (record) => (
      <Tag
        color={getMethodColor(record.method)}
        style={{ margin: 0, fontSize: 11 }}
      >
        {record.method}
      </Tag>
    ),
  },
  {
    key: "status",
    title: "Status",
    width: 55,
    align: "center",
    render: (record) =>
      record.status > 0 ? (
        <Tag
          color={getStatusColor(record.status)}
          style={{ margin: 0, fontSize: 11 }}
        >
          {record.status}
        </Tag>
      ) : (
        <Text type="secondary">-</Text>
      ),
  },
  {
    key: "client",
    title: "Client",
    width: 140,
    render: (record) => {
      const clientApp = record.client_app || "";
      const clientIp = record.client_ip || "";
      const hasApp = Boolean(clientApp);
      const display = clientApp || clientIp || "-";
      const tooltip = hasApp
        ? `${clientApp} (PID: ${record.client_pid || "?"}, IP: ${clientIp || "?"})`
        : clientIp || "-";
      return (
        <Tooltip title={tooltip}>
          <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
            {hasApp && <AppIcon appName={clientApp} size={16} />}
            <Text
              type="secondary"
              style={{ fontSize: 11, lineHeight: "16px" }}
              ellipsis
            >
              {display}
            </Text>
          </div>
        </Tooltip>
      );
    },
  },
  {
    key: "host",
    title: "Host",
    width: 160,
    render: (record) => (
      <Tooltip title={record.host}>
        <Text style={{ fontSize: 12 }} ellipsis>
          {record.host}
        </Text>
      </Tooltip>
    ),
  },
  {
    key: "path",
    title: "Path",
    width: "auto",
    minWidth: 250,
    render: (record) => (
      <Tooltip title={record.path}>
        <Text style={{ fontSize: 12 }} ellipsis>
          {record.path}
        </Text>
      </Tooltip>
    ),
  },
  {
    key: "content_type",
    title: "Type",
    width: 80,
    render: (record) => {
      const short = record.content_type?.split(";")[0]?.split("/").pop() || "-";
      return (
        <Text type="secondary" style={{ fontSize: 11 }}>
          {short}
        </Text>
      );
    },
  },
  {
    key: "response_size",
    title: "Size",
    width: 65,
    align: "right",
    render: (record) => {
      const size = (record.is_websocket || record.is_sse || record.is_tunnel) && record.socket_status
        ? record.socket_status.send_bytes + record.socket_status.receive_bytes
        : record.response_size;
      return (
        <Text type="secondary" style={{ fontSize: 11 }}>
          {formatSize(size)}
        </Text>
      );
    },
  },
  {
    key: "duration_ms",
    title: "Time",
    width: 55,
    align: "right",
    render: (record) => (
      <Text
        type={record.duration_ms > 1000 ? "warning" : "secondary"}
        style={{ fontSize: 11 }}
      >
        {record.duration_ms > 0 ? `${record.duration_ms}ms` : "-"}
      </Text>
    ),
  },
  {
    key: "start_time",
    title: "Start Time",
    width: 160,
    render: (record) => (
      <Tooltip title={record.start_time}>
        <Text
          type="secondary"
          style={{ fontSize: 11, fontFamily: "monospace" }}
        >
          {record.start_time || "-"}
        </Text>
      </Tooltip>
    ),
  },
  {
    key: "end_time",
    title: "End Time",
    width: 160,
    render: (record) => (
      <Tooltip title={record.end_time || "-"}>
        <Text
          type="secondary"
          style={{ fontSize: 11, fontFamily: "monospace" }}
        >
          {record.end_time || "-"}
        </Text>
      </Tooltip>
    ),
  },
  {
    key: "rules",
    title: "Rules",
    width: 60,
    align: "center",
    render: (record) =>
      record.has_rule_hit ? (
        <Tooltip
          title={
            <div>
              <div>{record.matched_rule_count} rule(s) matched</div>
              {record.matched_protocols.length > 0 && (
                <div style={{ marginTop: 4 }}>
                  {record.matched_protocols.join(", ")}
                </div>
              )}
            </div>
          }
        >
          <Badge count={record.matched_rule_count} size="small" color="blue">
            <ThunderboltOutlined style={{ fontSize: 14, color: "#1890ff" }} />
          </Badge>
        </Tooltip>
      ) : (
        <Text type="secondary">-</Text>
      ),
  },
];

export default function VirtualTrafficTable({
  data,
  onSelect,
  selectedId,
  onLoadMore,
  hasMore,
  autoScroll = true,
  onScrollPositionChange,
  newRecordsCount = 0,
  onScrollToBottom,
  onKeyboardNavigate,
  initialScrollTop = 0,
  onScrollTopChange,
}: VirtualTrafficTableProps) {
  const { token } = theme.useToken();
  const parentRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const prevDataLengthRef = useRef(data.length);
  const isAtBottomRef = useRef(true);
  const [showNewIndicator, setShowNewIndicator] = useState(false);
  const initialScrollRestoredRef = useRef(false);

  const rowVirtualizer = useVirtualizer({
    count: data.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
    getItemKey: (index) => data[index]?.id ?? index,
  });

  const checkIsAtBottom = useCallback(() => {
    if (!parentRef.current) return true;
    const { scrollTop, scrollHeight, clientHeight } = parentRef.current;
    return scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
  }, []);

  const handleScroll = useCallback(() => {
    if (!parentRef.current) return;

    const { scrollTop } = parentRef.current;
    onScrollTopChange?.(scrollTop);

    const isAtBottom = checkIsAtBottom();

    if (isAtBottomRef.current !== isAtBottom) {
      isAtBottomRef.current = isAtBottom;
      onScrollPositionChange?.(isAtBottom);

      if (isAtBottom) {
        setShowNewIndicator(false);
      }
    }

    if (onLoadMore && hasMore) {
      const { scrollHeight, clientHeight } = parentRef.current;
      if (scrollHeight - scrollTop - clientHeight < 200) {
        onLoadMore();
      }
    }
  }, [checkIsAtBottom, onScrollPositionChange, onLoadMore, hasMore, onScrollTopChange]);

  useEffect(() => {
    if (!initialScrollRestoredRef.current && data.length > 0 && parentRef.current) {
      initialScrollRestoredRef.current = true;
      if (initialScrollTop > 0) {
        parentRef.current.scrollTop = initialScrollTop;
        isAtBottomRef.current = checkIsAtBottom();
      }
      prevDataLengthRef.current = data.length;
      return;
    }

    const prevLength = prevDataLengthRef.current;
    const currLength = data.length;

    if (currLength > prevLength && prevLength > 0) {
      if (autoScroll && isAtBottomRef.current) {
        requestAnimationFrame(() => {
          rowVirtualizer.scrollToIndex(currLength - 1, {
            align: "end",
            behavior: "auto",
          });
        });
      } else if (!isAtBottomRef.current) {
        setShowNewIndicator(true);
      }
    }

    prevDataLengthRef.current = currLength;
  }, [data.length, autoScroll, rowVirtualizer, initialScrollTop, checkIsAtBottom]);

  useEffect(() => {
    if (data.length === 0) {
      prevDataLengthRef.current = 0;
      isAtBottomRef.current = true;
      setShowNewIndicator(false);
    }
  }, [data.length]);

  useEffect(() => {
    if (newRecordsCount > 0 && !isAtBottomRef.current) {
      setShowNewIndicator(true);
    } else if (newRecordsCount === 0 || isAtBottomRef.current) {
      setShowNewIndicator(false);
    }
  }, [newRecordsCount]);

  const handleScrollToBottomClick = useCallback(() => {
    rowVirtualizer.scrollToIndex(data.length - 1, {
      align: "end",
      behavior: "smooth",
    });
    setShowNewIndicator(false);
    onScrollToBottom?.();
  }, [rowVirtualizer, data.length, onScrollToBottom]);

  const scrollToRow = useCallback(
    (index: number, smooth: boolean = true) => {
      if (!parentRef.current) return;

      const scrollTop = parentRef.current.scrollTop;
      const clientHeight = parentRef.current.clientHeight;
      const headerHeight = ROW_HEIGHT;
      const rowTop = index * ROW_HEIGHT + headerHeight;
      const rowBottom = rowTop + ROW_HEIGHT;
      const visibleTop = scrollTop + headerHeight;
      const visibleBottom = scrollTop + clientHeight;

      const behavior = smooth ? "smooth" : "auto";
      if (rowTop < visibleTop) {
        rowVirtualizer.scrollToIndex(index, { align: "start", behavior });
      } else if (rowBottom > visibleBottom) {
        rowVirtualizer.scrollToIndex(index, { align: "end", behavior });
      }
    },
    [rowVirtualizer],
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (data.length === 0) return;
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;

      e.preventDefault();

      const currentIndex = selectedId
        ? data.findIndex((r) => r.id === selectedId)
        : -1;

      let nextIndex: number;

      if (e.key === "ArrowDown") {
        if (currentIndex === -1 || currentIndex >= data.length - 1) {
          nextIndex = 0;
        } else {
          nextIndex = currentIndex + 1;
        }
      } else {
        if (currentIndex === -1 || currentIndex <= 0) {
          nextIndex = data.length - 1;
        } else {
          nextIndex = currentIndex - 1;
        }
      }

      const nextRecord = data[nextIndex];
      if (nextRecord) {
        onSelect?.(nextRecord);
        onKeyboardNavigate?.(nextRecord);
        scrollToRow(nextIndex);
      }
    },
    [data, selectedId, onSelect, onKeyboardNavigate, scrollToRow],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    container.addEventListener("keydown", handleKeyDown);
    return () => {
      container.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleKeyDown]);

  const styles: Record<string, CSSProperties> = {
    container: {
      display: "flex",
      flexDirection: "column",
      height: "100%",
      width: "100%",
      overflow: "hidden",
      position: "relative",
      backgroundColor: token.colorBgContainer,
      outline: "none",
    },
    scrollContainer: {
      flex: 1,
      overflow: "auto",
      position: "relative",
      minWidth: 0,
      backgroundColor: token.colorBgContainer,
    },
    tableInner: {
      minWidth: TABLE_MIN_WIDTH,
      display: "flex",
      flexDirection: "column",
      height: "fit-content",
      minHeight: "100%",
      backgroundColor: token.colorBgContainer,
    },
    header: {
      display: "flex",
      alignItems: "center",
      height: ROW_HEIGHT,
      minWidth: TABLE_MIN_WIDTH,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      backgroundColor: token.colorBgContainer,
      fontSize: 12,
      fontWeight: 500,
      color: token.colorTextSecondary,
      position: "sticky",
      top: 0,
      zIndex: 2,
      flexShrink: 0,
    },
    headerCell: {
      padding: "0 8px",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap",
    },
    virtualList: {
      width: "100%",
      minWidth: TABLE_MIN_WIDTH,
      position: "relative",
      willChange: "transform",
      contain: "strict",
      backgroundColor: token.colorBgContainer,
    },
    row: {
      display: "flex",
      alignItems: "center",
      height: ROW_HEIGHT,
      maxHeight: ROW_HEIGHT,
      minHeight: ROW_HEIGHT,
      minWidth: TABLE_MIN_WIDTH,
      boxSizing: "border-box",
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      cursor: "pointer",
      position: "absolute",
      top: 0,
      left: 0,
      width: "100%",
      willChange: "transform",
      contain: "layout style",
    },
    cell: {
      padding: "0 8px",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap",
      display: "flex",
      alignItems: "center",
      height: "100%",
      maxHeight: ROW_HEIGHT,
      lineHeight: `${ROW_HEIGHT - 2}px`,
    },
    emptyState: {
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      height: "100%",
      color: token.colorTextSecondary,
    },
    newRecordsIndicator: {
      position: "absolute",
      bottom: 16,
      left: "50%",
      transform: "translateX(-50%)",
      display: "flex",
      alignItems: "center",
      gap: 8,
      padding: "8px 16px",
      backgroundColor: token.colorPrimary,
      color: "#fff",
      borderRadius: 20,
      cursor: "pointer",
      boxShadow: "0 2px 8px rgba(0, 0, 0, 0.15)",
      zIndex: 100,
      animation: "slideUp 0.3s ease-out",
      transition: "transform 0.2s, box-shadow 0.2s",
    },
  };

  const getColumnStyle = (col: ColumnDef): CSSProperties => {
    const width = typeof col.width === "number" ? col.width : undefined;
    const minWidth = col.minWidth ?? width;
    const isAutoWidth = col.width === "auto";
    return {
      width: width,
      minWidth: minWidth,
      flex: isAutoWidth ? 1 : `0 0 ${width}px`,
      justifyContent:
        col.align === "center"
          ? "center"
          : col.align === "right"
            ? "flex-end"
            : "flex-start",
    };
  };

  return (
    <div ref={containerRef} style={styles.container} tabIndex={0}>
      <style>
        {`
          @keyframes slideUp {
            from {
              opacity: 0;
              transform: translateX(-50%) translateY(20px);
            }
            to {
              opacity: 1;
              transform: translateX(-50%) translateY(0);
            }
          }
          @keyframes pulse {
            0%, 100% {
              transform: translateX(-50%) scale(1);
            }
            50% {
              transform: translateX(-50%) scale(1.05);
            }
          }
        `}
      </style>
      <div
        ref={parentRef}
        style={styles.scrollContainer}
        onScroll={handleScroll}
      >
        <div style={styles.tableInner}>
          <div style={styles.header}>
            {columns.map((col) => (
              <div
                key={col.key}
                style={{ ...styles.headerCell, ...getColumnStyle(col) }}
              >
                {col.title}
              </div>
            ))}
          </div>

          {data.length === 0 ? (
            <div style={styles.emptyState}>No traffic data</div>
          ) : (
            <div
              style={{
                ...styles.virtualList,
                height: `${rowVirtualizer.getTotalSize()}px`,
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const record = data[virtualRow.index];
                if (!record) return null;
                const isSelected = record.id === selectedId;
                return (
                  <div
                    key={virtualRow.key}
                    data-index={virtualRow.index}
                    style={{
                      ...styles.row,
                      height: ROW_HEIGHT,
                      transform: `translateY(${virtualRow.start}px)`,
                      backgroundColor: isSelected
                        ? token.colorPrimaryBg
                        : virtualRow.index % 2 === 0
                          ? token.colorBgContainer
                          : token.colorFillQuaternary,
                    }}
                    onClick={() => onSelect?.(record)}
                  >
                    {columns.map((col) => (
                      <div
                        key={col.key}
                        style={{ ...styles.cell, ...getColumnStyle(col) }}
                      >
                        {col.render(record)}
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {showNewIndicator && newRecordsCount > 0 && (
        <div
          style={{
            ...styles.newRecordsIndicator,
            animation: "slideUp 0.3s ease-out, pulse 2s ease-in-out infinite",
          }}
          onClick={handleScrollToBottomClick}
        >
          <Badge
            count={newRecordsCount}
            size="small"
            style={{ backgroundColor: "#fff", color: token.colorPrimary }}
          >
            <span style={{ color: "#fff", fontSize: 13 }}>New Traffic</span>
          </Badge>
          <ArrowDownOutlined style={{ fontSize: 14 }} />
        </div>
      )}
    </div>
  );
}
