import {
  useRef,
  useEffect,
  useCallback,
  useState,
  memo,
  useMemo,
  type CSSProperties,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Tag, Typography, Tooltip, Badge, theme } from "antd";
import { ThunderboltOutlined, ArrowDownOutlined } from "@ant-design/icons";
import type { TrafficSummary } from "../../types";
import AppIcon from "../AppIcon";
import TrafficContextMenu from "./TrafficContextMenu";

const { Text } = Typography;

interface VirtualTrafficTableProps {
  data: TrafficSummary[];
  onSelect?: (record: TrafficSummary) => void;
  onDoubleClick?: (record: TrafficSummary) => void;
  selectedId?: string;
  selectedIds?: string[];
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
const TABLE_MIN_WIDTH = 1440;

const STATUS_DOT_COLORS: Record<string, string> = {
  pending: "#d9d9d9",
  info: "#73d13d",
  success: "#52c41a",
  redirect: "#faad14",
  clientError: "#fa8c16",
  serverError: "#f5222d",
};

const getStatusDotColor = (status: number): string => {
  if (status === 0) return STATUS_DOT_COLORS.pending;
  if (status >= 100 && status < 200) return STATUS_DOT_COLORS.info;
  if (status >= 200 && status < 300) return STATUS_DOT_COLORS.success;
  if (status >= 300 && status < 400) return STATUS_DOT_COLORS.redirect;
  if (status >= 400 && status < 500) return STATUS_DOT_COLORS.clientError;
  if (status >= 500) return STATUS_DOT_COLORS.serverError;
  return STATUS_DOT_COLORS.pending;
};

const getStatusColor = (status: number) => {
  if (status >= 500) return "error";
  if (status >= 400) return "warning";
  if (status >= 300) return "processing";
  if (status >= 200) return "success";
  return "default";
};

const METHOD_COLORS: Record<string, string> = {
  GET: "green",
  POST: "blue",
  PUT: "orange",
  DELETE: "red",
  PATCH: "purple",
  OPTIONS: "default",
  HEAD: "cyan",
  CONNECT: "magenta",
};

const getMethodColor = (method: string) => {
  return METHOD_COLORS[method.toUpperCase()] || "default";
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

const columnStyles: CSSProperties[] = [];

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
      const size =
        (record.is_websocket || record.is_sse || record.is_tunnel) &&
        record.socket_status
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

for (const col of columns) {
  columnStyles.push(getColumnStyle(col));
}

const baseRowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  height: ROW_HEIGHT,
  maxHeight: ROW_HEIGHT,
  minHeight: ROW_HEIGHT,
  minWidth: TABLE_MIN_WIDTH,
  boxSizing: "border-box",
  cursor: "pointer",
  position: "absolute",
  top: 0,
  left: 0,
  width: "100%",
  willChange: "transform",
  contain: "layout style",
};

const baseCellStyle: CSSProperties = {
  padding: "0 8px",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  display: "flex",
  alignItems: "center",
  height: "100%",
  maxHeight: ROW_HEIGHT,
  lineHeight: `${ROW_HEIGHT - 2}px`,
};

interface TableRowProps {
  record: TrafficSummary;
  isSelected: boolean;
  translateY: number;
  rowIndex: number;
  borderColor: string;
  selectedBg: string;
  evenBg: string;
  oddBg: string;
  onRowClick: () => void;
  onRowDoubleClick: () => void;
  onRowContextMenu: (e: React.MouseEvent) => void;
}

const areRowPropsEqual = (prev: TableRowProps, next: TableRowProps): boolean => {
  if (prev.isSelected !== next.isSelected) return false;
  if (prev.translateY !== next.translateY) return false;
  if (prev.rowIndex !== next.rowIndex) return false;
  
  const prevRecord = prev.record;
  const nextRecord = next.record;
  if (prevRecord.id !== nextRecord.id) return false;
  if (prevRecord.status !== nextRecord.status) return false;
  if (prevRecord.sequence !== nextRecord.sequence) return false;
  if (prevRecord.duration_ms !== nextRecord.duration_ms) return false;
  if (prevRecord.response_size !== nextRecord.response_size) return false;
  if (prevRecord.end_time !== nextRecord.end_time) return false;
  if (prevRecord.socket_status?.send_bytes !== nextRecord.socket_status?.send_bytes) return false;
  if (prevRecord.socket_status?.receive_bytes !== nextRecord.socket_status?.receive_bytes) return false;
  
  return true;
};

const TableRow = memo(function TableRow({
  record,
  isSelected,
  translateY,
  rowIndex,
  borderColor,
  selectedBg,
  evenBg,
  oddBg,
  onRowClick,
  onRowDoubleClick,
  onRowContextMenu,
}: TableRowProps) {
  const rowStyle: CSSProperties = {
    ...baseRowStyle,
    borderBottom: `1px solid ${borderColor}`,
    transform: `translateY(${translateY}px)`,
    backgroundColor: isSelected
      ? selectedBg
      : rowIndex % 2 === 0
        ? evenBg
        : oddBg,
  };

  return (
    <div
      data-index={rowIndex}
      style={rowStyle}
      onClick={onRowClick}
      onDoubleClick={onRowDoubleClick}
      onContextMenu={onRowContextMenu}
    >
      {columns.map((col, colIndex) => (
        <div
          key={col.key}
          style={{ ...baseCellStyle, ...columnStyles[colIndex] }}
        >
          {col.render(record)}
        </div>
      ))}
    </div>
  );
}, areRowPropsEqual);

const keyframesStyle = `
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
`;

export default function VirtualTrafficTable({
  data,
  onSelect,
  onDoubleClick,
  selectedId,
  selectedIds = [],
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

  const [contextMenu, setContextMenu] = useState<{
    visible: boolean;
    record: TrafficSummary | null;
    position: { x: number; y: number };
  }>({
    visible: false,
    record: null,
    position: { x: 0, y: 0 },
  });

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, record: TrafficSummary) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({
        visible: true,
        record,
        position: { x: e.clientX, y: e.clientY },
      });
    },
    [],
  );

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }));
  }, []);

  const selectedRecords =
    selectedIds.length > 0
      ? data.filter((r) => selectedIds.includes(r.id))
      : contextMenu.record
        ? [contextMenu.record]
        : [];

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
  }, [
    checkIsAtBottom,
    onScrollPositionChange,
    onLoadMore,
    hasMore,
    onScrollTopChange,
  ]);

  useEffect(() => {
    if (
      !initialScrollRestoredRef.current &&
      data.length > 0 &&
      parentRef.current
    ) {
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
        rowVirtualizer.scrollToIndex(currLength - 1, {
          align: "end",
          behavior: "auto",
        });
      } else if (!isAtBottomRef.current) {
        setShowNewIndicator(true);
      }
    }

    prevDataLengthRef.current = currLength;
  }, [
    data.length,
    autoScroll,
    rowVirtualizer,
    initialScrollTop,
    checkIsAtBottom,
  ]);

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

  const styles = useMemo(
    () => ({
      container: {
        display: "flex",
        flexDirection: "column" as const,
        height: "100%",
        width: "100%",
        overflow: "hidden",
        position: "relative" as const,
        backgroundColor: token.colorBgContainer,
        outline: "none",
      },
      scrollContainer: {
        flex: 1,
        overflow: "auto",
        position: "relative" as const,
        minWidth: 0,
        backgroundColor: token.colorBgContainer,
      },
      tableInner: {
        minWidth: TABLE_MIN_WIDTH,
        display: "flex",
        flexDirection: "column" as const,
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
        position: "sticky" as const,
        top: 0,
        zIndex: 2,
        flexShrink: 0,
      },
      headerCell: {
        padding: "0 8px",
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap" as const,
      },
      virtualList: {
        width: "100%",
        minWidth: TABLE_MIN_WIDTH,
        position: "relative" as const,
        willChange: "transform",
        contain: "strict",
        backgroundColor: token.colorBgContainer,
      },
      emptyState: {
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        color: token.colorTextSecondary,
      },
      newRecordsIndicator: {
        position: "absolute" as const,
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
    }),
    [token],
  );

  const headerCells = useMemo(
    () =>
      columns.map((col, index) => (
        <div
          key={col.key}
          style={{ ...styles.headerCell, ...columnStyles[index] }}
        >
          {col.title}
        </div>
      )),
    [styles.headerCell],
  );

  const virtualItems = rowVirtualizer.getVirtualItems();

  return (
    <div ref={containerRef} style={styles.container} tabIndex={0}>
      <style>{keyframesStyle}</style>
      <div
        ref={parentRef}
        style={styles.scrollContainer}
        onScroll={handleScroll}
      >
        <div style={styles.tableInner}>
          <div style={styles.header}>{headerCells}</div>

          {data.length === 0 ? (
            <div style={styles.emptyState}>No traffic data</div>
          ) : (
            <div
              style={{
                ...styles.virtualList,
                height: `${rowVirtualizer.getTotalSize()}px`,
              }}
            >
              {virtualItems.map((virtualRow) => {
                const record = data[virtualRow.index];
                if (!record) return null;
                return (
                  <TableRow
                    key={virtualRow.key}
                    record={record}
                    isSelected={record.id === selectedId}
                    translateY={virtualRow.start}
                    rowIndex={virtualRow.index}
                    borderColor={token.colorBorderSecondary}
                    selectedBg={token.colorPrimaryBg}
                    evenBg={token.colorBgContainer}
                    oddBg={token.colorFillQuaternary}
                    onRowClick={() => onSelect?.(record)}
                    onRowDoubleClick={() => onDoubleClick?.(record)}
                    onRowContextMenu={(e) => handleContextMenu(e, record)}
                  />
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

      <TrafficContextMenu
        record={contextMenu.record}
        visible={contextMenu.visible}
        position={contextMenu.position}
        onClose={handleCloseContextMenu}
        selectedRecords={selectedRecords}
      />
    </div>
  );
}
