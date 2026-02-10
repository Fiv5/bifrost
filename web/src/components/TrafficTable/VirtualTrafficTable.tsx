import { useRef, type CSSProperties } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Tag, Typography, Tooltip, Badge, Spin, theme } from "antd";
import { ThunderboltOutlined } from "@ant-design/icons";
import type { TrafficSummary } from "../../types";

const { Text } = Typography;

interface VirtualTrafficTableProps {
  data: TrafficSummary[];
  loading?: boolean;
  onSelect?: (record: TrafficSummary) => void;
  selectedId?: string;
  onLoadMore?: () => void;
  hasMore?: boolean;
}

const ROW_HEIGHT = 36;

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

interface ColumnDef {
  key: string;
  title: string;
  width: number | string;
  align?: "left" | "center" | "right";
  render: (record: TrafficSummary) => React.ReactNode;
}

const columns: ColumnDef[] = [
  {
    key: "has_rule_hit",
    title: "",
    width: 28,
    align: "center",
    render: (record) => (
      <Tooltip
        title={
          record.has_rule_hit
            ? `${record.matched_rule_count} rule(s): ${record.matched_protocols?.join(", ") || ""}`
            : "No rules matched"
        }
      >
        <Badge
          status={record.has_rule_hit ? "success" : "default"}
          style={{ cursor: "pointer" }}
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
      <Tag color={getMethodColor(record.method)} style={{ margin: 0, fontSize: 11 }}>
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
        <Tag color={getStatusColor(record.status)} style={{ margin: 0, fontSize: 11 }}>
          {record.status}
        </Tag>
      ) : (
        <Text type="secondary">-</Text>
      ),
  },
  {
    key: "client_ip",
    title: "Client",
    width: 90,
    render: (record) => (
      <Text type="secondary" style={{ fontSize: 11 }} ellipsis>
        {record.client_ip || "-"}
      </Text>
    ),
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
    render: (record) => (
      <Text type="secondary" style={{ fontSize: 11 }}>
        {formatSize(record.response_size)}
      </Text>
    ),
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
  loading,
  onSelect,
  selectedId,
  onLoadMore,
  hasMore,
}: VirtualTrafficTableProps) {
  const { token } = theme.useToken();
  const parentRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: data.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  const handleScroll = () => {
    if (!parentRef.current || !onLoadMore || !hasMore) return;
    const { scrollTop, scrollHeight, clientHeight } = parentRef.current;
    if (scrollHeight - scrollTop - clientHeight < 200) {
      onLoadMore();
    }
  };

  const styles: Record<string, CSSProperties> = {
    container: {
      display: "flex",
      flexDirection: "column",
      height: "100%",
      width: "100%",
      overflow: "hidden",
    },
    header: {
      display: "flex",
      alignItems: "center",
      height: ROW_HEIGHT,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      backgroundColor: token.colorBgContainer,
      fontSize: 12,
      fontWeight: 500,
      color: token.colorTextSecondary,
      position: "sticky",
      top: 0,
      zIndex: 1,
      flexShrink: 0,
    },
    headerCell: {
      padding: "0 8px",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap",
    },
    scrollContainer: {
      flex: 1,
      overflow: "auto",
      position: "relative",
    },
    virtualList: {
      width: "100%",
      position: "relative",
    },
    row: {
      display: "flex",
      alignItems: "center",
      height: ROW_HEIGHT,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      cursor: "pointer",
      position: "absolute",
      top: 0,
      left: 0,
      width: "100%",
    },
    cell: {
      padding: "0 8px",
      overflow: "hidden",
      textOverflow: "ellipsis",
      whiteSpace: "nowrap",
      display: "flex",
      alignItems: "center",
    },
    loadingOverlay: {
      position: "absolute",
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      backgroundColor: "rgba(255, 255, 255, 0.7)",
      zIndex: 10,
    },
    emptyState: {
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      height: "100%",
      color: token.colorTextSecondary,
    },
  };

  const getColumnStyle = (col: ColumnDef): CSSProperties => {
    const width = typeof col.width === "number" ? col.width : undefined;
    return {
      width: width,
      minWidth: width,
      flex: col.width === "auto" ? 1 : undefined,
      justifyContent:
        col.align === "center"
          ? "center"
          : col.align === "right"
            ? "flex-end"
            : "flex-start",
    };
  };

  return (
    <div style={styles.container}>
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

      <div ref={parentRef} style={styles.scrollContainer} onScroll={handleScroll}>
        {data.length === 0 ? (
          <div style={styles.emptyState}>
            {loading ? <Spin /> : "No traffic data"}
          </div>
        ) : (
          <div
            style={{
              ...styles.virtualList,
              height: `${rowVirtualizer.getTotalSize()}px`,
            }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const record = data[virtualRow.index];
              const isSelected = record.id === selectedId;
              return (
                <div
                  key={record.id}
                  style={{
                    ...styles.row,
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

        {loading && data.length > 0 && (
          <div style={styles.loadingOverlay}>
            <Spin />
          </div>
        )}
      </div>
    </div>
  );
}
