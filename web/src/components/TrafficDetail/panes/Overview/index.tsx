import { useMemo, useRef } from "react";
import {
  Descriptions,
  Typography,
  theme,
  Tag,
  Collapse,
  ConfigProvider,
} from "antd";
import dayjs from "dayjs";
import type {
  TrafficRecord,
  SessionTargetSearchState,
  MatchedRule,
  RequestTiming,
  SocketStatus,
} from "../../../../types";
import { useMarkSearch } from "../../hooks/useMarkSearch";
import AppIcon from "../../../AppIcon";

const { Text, Paragraph } = Typography;

interface OverviewProps {
  record: TrafficRecord;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

const STATUS_CODES: Record<number, string> = {
  100: "Continue",
  101: "Switching Protocols",
  200: "OK",
  201: "Created",
  202: "Accepted",
  204: "No Content",
  206: "Partial Content",
  301: "Moved Permanently",
  302: "Found",
  303: "See Other",
  304: "Not Modified",
  307: "Temporary Redirect",
  308: "Permanent Redirect",
  400: "Bad Request",
  401: "Unauthorized",
  403: "Forbidden",
  404: "Not Found",
  405: "Method Not Allowed",
  408: "Request Timeout",
  409: "Conflict",
  413: "Payload Too Large",
  414: "URI Too Long",
  415: "Unsupported Media Type",
  429: "Too Many Requests",
  500: "Internal Server Error",
  501: "Not Implemented",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
};

const formatSize = (bytes: number) => {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
};

const RuleCard = ({ rule, index }: { rule: MatchedRule; index: number }) => {
  const { token } = theme.useToken();
  const source = rule.rule_name
    ? `${rule.rule_name}${rule.line ? `:${rule.line}` : ""}`
    : "Unknown";

  return (
    <div
      style={{
        padding: 8,
        marginBottom: 6,
        backgroundColor: token.colorBgLayout,
        borderRadius: 4,
        border: `1px solid ${token.colorBorderSecondary}`,
      }}
    >
      <div style={{ marginBottom: 4 }}>
        <Tag color="blue">#{index + 1}</Tag>
        <Text strong>{source}</Text>
      </div>
      <div style={{ marginBottom: 2 }}>
        <Text type="secondary">Protocol: </Text>
        <Tag color="green">{rule.protocol}</Tag>
      </div>
      <div style={{ marginBottom: 2 }}>
        <Text type="secondary">Pattern: </Text>
        <Text code>{rule.pattern}</Text>
      </div>
      <div style={{ marginBottom: 2 }}>
        <Text type="secondary">Value: </Text>
        <Text code>{rule.value || "(empty)"}</Text>
      </div>
      {rule.raw && (
        <div>
          <Text type="secondary">Raw Rule:</Text>
          <pre
            style={{
              fontFamily: "monospace",
              fontSize: 12,
              padding: "4px 8px",
              borderRadius: 4,
              margin: "4px 0 0 0",
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
              backgroundColor: token.colorBgContainer,
            }}
          >
            {rule.raw}
          </pre>
        </div>
      )}
    </div>
  );
};

const TimingBar = ({ timing }: { timing: RequestTiming }) => {
  const { token } = theme.useToken();
  const total = timing.total_ms || 1;

  const phases = [
    { key: "dns", label: "DNS", value: timing.dns_ms, color: "#8884d8" },
    {
      key: "connect",
      label: "Connect",
      value: timing.connect_ms,
      color: "#82ca9d",
    },
    { key: "tls", label: "TLS", value: timing.tls_ms, color: "#ffc658" },
    { key: "send", label: "Send", value: timing.send_ms, color: "#ff7300" },
    { key: "wait", label: "Wait", value: timing.wait_ms, color: "#00C49F" },
    {
      key: "receive",
      label: "Receive",
      value: timing.receive_ms,
      color: "#0088FE",
    },
  ].filter((p) => p.value !== undefined && p.value > 0);

  return (
    <div style={{ marginTop: 4 }}>
      <div
        style={{
          display: "flex",
          height: 16,
          borderRadius: 4,
          overflow: "hidden",
          backgroundColor: token.colorBgLayout,
        }}
      >
        {phases.map((phase) => (
          <div
            key={phase.key}
            style={{
              width: `${((phase.value || 0) / total) * 100}%`,
              backgroundColor: phase.color,
              minWidth: phase.value ? 2 : 0,
            }}
            title={`${phase.label}: ${phase.value}ms`}
          />
        ))}
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 4 }}>
        {phases.map((phase) => (
          <div
            key={phase.key}
            style={{ display: "flex", alignItems: "center", gap: 4 }}
          >
            <div
              style={{
                width: 10,
                height: 10,
                borderRadius: 2,
                backgroundColor: phase.color,
              }}
            />
            <Text type="secondary" style={{ fontSize: 11 }}>
              {phase.label}: {phase.value}ms
            </Text>
          </div>
        ))}
      </div>
    </div>
  );
};

const SocketStatusCard = ({
  status,
  isWebSocket,
}: {
  status: SocketStatus;
  isWebSocket: boolean;
}) => {
  const { token } = theme.useToken();

  return (
    <div
      className="compact-socket-status"
      style={{
        padding: 8,
        backgroundColor: token.colorBgLayout,
        borderRadius: 4,
        border: `1px solid ${token.colorBorderSecondary}`,
      }}
    >
      <div style={{ marginBottom: 4 }}>
        <Tag color={status.is_open ? "green" : "default"}>
          {status.is_open ? "Connected" : "Closed"}
        </Tag>
        <Tag color="blue">{isWebSocket ? "WebSocket" : "SSE"}</Tag>
      </div>
      <ConfigProvider
        theme={{
          components: {
            Descriptions: {
              itemPaddingBottom: 2,
              padding: 4,
              paddingSM: 4,
            },
          },
        }}
      >
        <Descriptions column={2} size="small" bordered>
          <Descriptions.Item label="Send Count">
            {status.send_count}
          </Descriptions.Item>
          <Descriptions.Item label="Receive Count">
            {status.receive_count}
          </Descriptions.Item>
          <Descriptions.Item label="Send Bytes">
            {formatSize(status.send_bytes)}
          </Descriptions.Item>
          <Descriptions.Item label="Receive Bytes">
            {formatSize(status.receive_bytes)}
          </Descriptions.Item>
          <Descriptions.Item label="Frame Count">
            {status.frame_count}
          </Descriptions.Item>
          {status.close_code !== undefined && (
            <Descriptions.Item label="Close Code">
              {status.close_code}
            </Descriptions.Item>
          )}
          {status.close_reason && (
            <Descriptions.Item label="Close Reason" span={2}>
              {status.close_reason}
            </Descriptions.Item>
          )}
        </Descriptions>
      </ConfigProvider>
    </div>
  );
};

export const Overview = ({ record, searchValue, onSearch }: OverviewProps) => {
  const wrapperRef = useRef<HTMLDivElement>(null);

  useMarkSearch(searchValue, () => wrapperRef.current, onSearch);

  const collapseItems = useMemo(() => {
    const items = [];

    if (record.timing) {
      items.push({
        key: "timing",
        label: <Text strong>Timing Details</Text>,
        children: <TimingBar timing={record.timing} />,
      });
    }

    if ((record.is_websocket || record.is_sse) && record.socket_status) {
      items.push({
        key: "socket",
        label: (
          <Text strong>
            {record.is_websocket ? "WebSocket" : "SSE"} Status
            <Tag
              color={record.socket_status.is_open ? "green" : "default"}
              style={{ marginLeft: 8 }}
            >
              {record.socket_status.is_open ? "Connected" : "Closed"}
            </Tag>
          </Text>
        ),
        children: (
          <SocketStatusCard
            status={record.socket_status}
            isWebSocket={record.is_websocket || false}
          />
        ),
      });
    }

    if (record.matched_rules && record.matched_rules.length > 0) {
      items.push({
        key: "rules",
        label: (
          <Text strong>
            Matched Rules <Tag color="blue">{record.matched_rules.length}</Tag>
          </Text>
        ),
        children: (
          <div>
            {record.matched_rules.map((rule, index) => (
              <RuleCard key={index} rule={rule} index={index} />
            ))}
          </div>
        ),
      });
    }

    return items;
  }, [record]);

  const connectionType = useMemo(() => {
    if (record.is_websocket) return "WebSocket";
    if (record.is_sse) return "SSE";
    if (record.is_tunnel) return "Tunnel";
    return null;
  }, [record.is_websocket, record.is_sse, record.is_tunnel]);

  return (
    <div ref={wrapperRef}>
      <ConfigProvider
        theme={{
          components: {
            Descriptions: {
              itemPaddingBottom: 2,
              padding: 4,
              paddingSM: 4,
            },
          },
        }}
      >
        <Descriptions
          column={1}
          size="small"
          bordered
          style={{ marginBottom: 8 }}
          labelStyle={{ width: 140, fontWeight: 500 }}
        >
          <Descriptions.Item label="URL">
            <Paragraph
              style={{ margin: 0, maxWidth: "100%" }}
              ellipsis={{ rows: 2, expandable: true }}
              copyable
            >
              {record.url}
            </Paragraph>
          </Descriptions.Item>
          <Descriptions.Item label="Method">
            <Tag color="blue">{record.method}</Tag>
            {connectionType && (
              <Tag color="purple" style={{ marginLeft: 4 }}>
                {connectionType}
              </Tag>
            )}
          </Descriptions.Item>
          <Descriptions.Item label="Status">
            <Tag
              color={
                record.status >= 400
                  ? "red"
                  : record.status >= 300
                    ? "orange"
                    : "green"
              }
            >
              {record.status} {STATUS_CODES[record.status] || ""}
            </Tag>
          </Descriptions.Item>
          <Descriptions.Item label="Protocol">
            {record.protocol}
          </Descriptions.Item>
          <Descriptions.Item label="Host">{record.host}</Descriptions.Item>
          <Descriptions.Item label="Path">{record.path}</Descriptions.Item>
          <Descriptions.Item label="Content Type">
            {record.content_type || "-"}
          </Descriptions.Item>
          <Descriptions.Item label="Client">
            {record.client_app ? (
              <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
                <AppIcon appName={record.client_app} size={18} />
                <Tag color="cyan" style={{ margin: 0 }}>{record.client_app}</Tag>
                {record.client_pid && (
                  <Text type="secondary">
                    PID: {record.client_pid}
                  </Text>
                )}
                {record.client_ip && (
                  <Text type="secondary">
                    IP: {record.client_ip}
                  </Text>
                )}
              </div>
            ) : (
              record.client_ip || "-"
            )}
          </Descriptions.Item>
        </Descriptions>

        <Descriptions
          title="Size"
          column={2}
          size="small"
          bordered
          style={{ marginBottom: 8 }}
          labelStyle={{ width: 140, fontWeight: 500 }}
        >
          <Descriptions.Item label="Request Size">
            {formatSize(record.request_size)}
          </Descriptions.Item>
          <Descriptions.Item label="Response Size">
            {formatSize(record.response_size)}
          </Descriptions.Item>
          <Descriptions.Item label="Total Size">
            {formatSize(record.request_size + record.response_size)}
          </Descriptions.Item>
          <Descriptions.Item label="Duration">
            {record.duration_ms ? `${record.duration_ms}ms` : "-"}
          </Descriptions.Item>
        </Descriptions>

        <Descriptions
          title="Timeline"
          column={2}
          size="small"
          bordered
          style={{ marginBottom: 8 }}
          labelStyle={{ width: 140, fontWeight: 500 }}
        >
          <Descriptions.Item label="Timestamp" span={2}>
            {dayjs(record.timestamp).format("YYYY-MM-DD HH:mm:ss.SSS")}
          </Descriptions.Item>
          {record.timing && (
            <>
              {record.timing.dns_ms !== undefined && (
                <Descriptions.Item label="DNS">
                  {record.timing.dns_ms}ms
                </Descriptions.Item>
              )}
              {record.timing.connect_ms !== undefined && (
                <Descriptions.Item label="Connect">
                  {record.timing.connect_ms}ms
                </Descriptions.Item>
              )}
              {record.timing.tls_ms !== undefined && (
                <Descriptions.Item label="TLS">
                  {record.timing.tls_ms}ms
                </Descriptions.Item>
              )}
              {record.timing.send_ms !== undefined && (
                <Descriptions.Item label="Send">
                  {record.timing.send_ms}ms
                </Descriptions.Item>
              )}
              {record.timing.wait_ms !== undefined && (
                <Descriptions.Item label="Wait (TTFB)">
                  {record.timing.wait_ms}ms
                </Descriptions.Item>
              )}
              {record.timing.receive_ms !== undefined && (
                <Descriptions.Item label="Receive">
                  {record.timing.receive_ms}ms
                </Descriptions.Item>
              )}
            </>
          )}
        </Descriptions>

        {collapseItems.length > 0 && (
          <Collapse
            items={collapseItems}
            defaultActiveKey={["timing", "socket", "rules"]}
            size="small"
            style={{ marginTop: 8 }}
          />
        )}
      </ConfigProvider>
    </div>
  );
};
