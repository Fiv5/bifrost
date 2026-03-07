import { useCallback, useState, useRef, useEffect, memo } from "react";
import { Typography, Tooltip, Dropdown, message } from "antd";
import type { MenuProps } from "antd";
import {
  CopyOutlined,
  ExpandAltOutlined,
  ShrinkOutlined,
} from "@ant-design/icons";
import type { TrafficRecord } from "../../../types";

const { Text } = Typography;

interface HeaderProps {
  record: TrafficRecord;
}

const STATUS_CODES: Record<number, string> = {
  100: "Continue",
  101: "Switching Protocols",
  200: "OK",
  201: "Created",
  202: "Accepted",
  204: "No Content",
  301: "Moved Permanently",
  302: "Found",
  304: "Not Modified",
  400: "Bad Request",
  401: "Unauthorized",
  403: "Forbidden",
  404: "Not Found",
  500: "Internal Server Error",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
};

const getStatusColor = (status: number): string => {
  if (status >= 500) return "#f5222d";
  if (status >= 400) return "#fa8c16";
  if (status >= 300) return "#faad14";
  if (status >= 200) return "#52c41a";
  return "#1890ff";
};

const HeaderContent = memo(function HeaderContent({
  record,
}: {
  record: TrafficRecord;
}) {
  const { method, status, url } = record;
  const statusText = STATUS_CODES[status] || "";
  const statusLabel = statusText ? `${status} ${statusText}` : String(status);
  const statusColor = getStatusColor(status);

  const [expanded, setExpanded] = useState(false);
  const [isOverflow, setIsOverflow] = useState(false);
  const urlRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const el = urlRef.current;
    if (el) {
      setIsOverflow(el.scrollWidth > el.clientWidth);
    }
  }, [url]);

  const handleCopyUrl = useCallback(() => {
    navigator.clipboard.writeText(url);
    message.success("URL copied");
  }, [url]);

  const handleCopyCurl = useCallback(() => {
    let curl = `curl '${url}'`;
    if (record.request_headers) {
      record.request_headers.forEach(([key, value]) => {
        curl += ` \\\n  -H '${key}: ${value}'`;
      });
    }
    if (method !== "GET") {
      curl += ` \\\n  -X ${method}`;
    }
    navigator.clipboard.writeText(curl);
    message.success("cURL copied");
  }, [url, method, record.request_headers]);

  const copyMenuItems: MenuProps["items"] = [
    { key: "url", label: "Copy URL", onClick: handleCopyUrl },
    { key: "curl", label: "Copy as cURL", onClick: handleCopyCurl },
  ];

  const toggleExpand = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  return (
    <div
      style={{
        display: "flex",
        alignItems: "flex-start",
        padding: "4px 8px",
        borderBottom: "1px solid #f0f0f0",
        gap: 8,
      }}
      data-testid="traffic-detail-header"
      data-url={url}
    >
      <div
        style={{
          display: "flex",
          alignItems: expanded ? "flex-start" : "center",
          flex: 1,
          overflow: "hidden",
          gap: 8,
        }}
      >
        <Text
          strong
          style={{
            whiteSpace: "nowrap",
            userSelect: "none",
            flexShrink: 0,
          }}
        >
          {method}
        </Text>
        <Text
          style={{
            whiteSpace: "nowrap",
            userSelect: "none",
            color: statusColor,
            fontWeight: 500,
            flexShrink: 0,
          }}
        >
          {statusLabel}
        </Text>
        <Text
          ref={urlRef}
          style={{
            overflow: "hidden",
            textOverflow: expanded ? "unset" : "ellipsis",
            whiteSpace: expanded ? "normal" : "nowrap",
            wordBreak: expanded ? "break-all" : "normal",
            color: "#666",
            flex: 1,
          }}
        >
          {url}
        </Text>
      </div>

      <div
        style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}
      >
        {isOverflow && (
          <Tooltip title={expanded ? "Collapse" : "Expand"}>
            {expanded ? (
              <ShrinkOutlined
                onClick={toggleExpand}
                style={{
                  fontSize: 14,
                  padding: 4,
                  cursor: "pointer",
                  color: "#666",
                }}
              />
            ) : (
              <ExpandAltOutlined
                onClick={toggleExpand}
                style={{
                  fontSize: 14,
                  padding: 4,
                  cursor: "pointer",
                  color: "#666",
                }}
              />
            )}
          </Tooltip>
        )}
        <Dropdown menu={{ items: copyMenuItems }} trigger={["click"]}>
          <Tooltip title="Copy">
            <CopyOutlined
              style={{
                fontSize: 14,
                padding: 4,
                cursor: "pointer",
                color: "#666",
              }}
            />
          </Tooltip>
        </Dropdown>
      </div>
    </div>
  );
});

export const Header = ({ record }: HeaderProps) => {
  return <HeaderContent key={record.id} record={record} />;
};
