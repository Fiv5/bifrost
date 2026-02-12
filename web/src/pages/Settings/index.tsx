import { useEffect, useState } from "react";
import {
  Card,
  Descriptions,
  Spin,
  Alert,
  Typography,
  Row,
  Col,
  Statistic,
  Button,
  message,
  Progress,
  List,
  Badge,
  Space,
  Popconfirm,
  Switch,
  Tooltip,
} from "antd";
import {
  CopyOutlined,
  ApiOutlined,
  SwapOutlined,
  CloudUploadOutlined,
  CloudDownloadOutlined,
  DashboardOutlined,
  DatabaseOutlined,
  CheckOutlined,
  CloseOutlined,
  ClearOutlined,
  WarningOutlined,
  GlobalOutlined,
} from "@ant-design/icons";
import { useMetricsStore } from "../../stores/useMetricsStore";
import {
  getPendingAuthorizations,
  approvePending,
  rejectPending,
  clearPendingAuthorizations,
} from "../../api/whitelist";
import {
  getSystemProxyStatus,
  setSystemProxy,
  type SystemProxyStatus,
} from "../../api/proxy";
import type { PendingAuth } from "../../types";

const { Text, Paragraph } = Typography;

export default function Settings() {
  const { overview, loading, error, fetchOverview } = useMetricsStore();
  const [pendingList, setPendingList] = useState<PendingAuth[]>([]);
  const [pendingLoading, setPendingLoading] = useState(false);
  const [systemProxy, setSystemProxyState] = useState<SystemProxyStatus | null>(null);
  const [systemProxyLoading, setSystemProxyLoading] = useState(false);

  const fetchSystemProxy = async () => {
    try {
      const status = await getSystemProxyStatus();
      setSystemProxyState(status);
    } catch {
      console.error("Failed to fetch system proxy status");
    }
  };

  const handleSystemProxyToggle = async (enabled: boolean) => {
    setSystemProxyLoading(true);
    try {
      const result = await setSystemProxy({ enabled });
      setSystemProxyState(result);
      message.success(enabled ? "System proxy enabled" : "System proxy disabled");
    } catch {
      message.error("Failed to toggle system proxy");
    } finally {
      setSystemProxyLoading(false);
    }
  };

  const fetchPending = async () => {
    if (overview && overview.pending_authorizations > 0) {
      setPendingLoading(true);
      try {
        const list = await getPendingAuthorizations();
        setPendingList(list);
      } catch {
        console.error("Failed to fetch pending authorizations");
      } finally {
        setPendingLoading(false);
      }
    } else {
      setPendingList([]);
    }
  };

  useEffect(() => {
    fetchOverview();
    fetchSystemProxy();
    const interval = setInterval(fetchOverview, 1000);
    return () => clearInterval(interval);
  }, [fetchOverview]);

  useEffect(() => {
    fetchPending();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [overview?.pending_authorizations]);

  const handleApprove = async (ip: string) => {
    try {
      await approvePending(ip);
      message.success(`Approved ${ip}`);
      fetchOverview();
      fetchPending();
    } catch {
      message.error(`Failed to approve ${ip}`);
    }
  };

  const handleReject = async (ip: string) => {
    try {
      await rejectPending(ip);
      message.success(`Rejected ${ip}`);
      fetchOverview();
      fetchPending();
    } catch {
      message.error(`Failed to reject ${ip}`);
    }
  };

  const handleClearAll = async () => {
    try {
      await clearPendingAuthorizations();
      message.success("Cleared all pending authorizations");
      fetchOverview();
      fetchPending();
    } catch {
      message.error("Failed to clear pending authorizations");
    }
  };

  const copyProxyConfig = () => {
    const config = `HTTP Proxy: 127.0.0.1:${overview?.server.port || 8899}
HTTPS Proxy: 127.0.0.1:${overview?.server.port || 8899}`;
    navigator.clipboard.writeText(config);
    message.success("Proxy config copied to clipboard");
  };

  if (loading && !overview) {
    return (
      <Spin size="large" style={{ display: "block", margin: "100px auto" }} />
    );
  }

  if (error) {
    return (
      <Alert
        type="error"
        message="Failed to load system info"
        description={error}
      />
    );
  }

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const formatBytesRate = (bytesPerSec: number) => {
    return `${formatBytes(bytesPerSec)}/s`;
  };

  const formatTimeAgo = (timestamp: number) => {
    const now = Math.floor(Date.now() / 1000);
    const diff = now - timestamp;
    if (diff < 60) return `${diff}s ago`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  };

  const memoryPercent = overview
    ? (overview.metrics.memory_used / overview.metrics.memory_total) * 100
    : 0;

  const pendingCount = overview?.pending_authorizations || 0;

  return (
    <div style={{ padding: 16 }}>
      {pendingCount > 0 && (
        <Row gutter={[16, 16]} style={{ marginBottom: 16 }}>
          <Col xs={24}>
            <Alert
              type="warning"
              showIcon
              icon={<WarningOutlined />}
              message={
                <Space>
                  <Badge
                    count={pendingCount}
                    style={{ backgroundColor: "#faad14" }}
                  />
                  <span>Pending Authorization Requests</span>
                </Space>
              }
              description={
                <div style={{ marginTop: 8 }}>
                  <List
                    loading={pendingLoading}
                    size="small"
                    dataSource={pendingList}
                    locale={{ emptyText: "Loading..." }}
                    renderItem={(item) => (
                      <List.Item
                        actions={[
                          <Button
                            key="approve"
                            type="primary"
                            size="small"
                            icon={<CheckOutlined />}
                            onClick={() => handleApprove(item.ip)}
                          >
                            Allow
                          </Button>,
                          <Button
                            key="reject"
                            danger
                            size="small"
                            icon={<CloseOutlined />}
                            onClick={() => handleReject(item.ip)}
                          >
                            Deny
                          </Button>,
                        ]}
                      >
                        <List.Item.Meta
                          title={<Text code>{item.ip}</Text>}
                          description={
                            <Text type="secondary">
                              First seen: {formatTimeAgo(item.first_seen)} ·
                              Attempts: {item.attempt_count}
                            </Text>
                          }
                        />
                      </List.Item>
                    )}
                  />
                  {pendingList.length > 0 && (
                    <div style={{ marginTop: 8, textAlign: "right" }}>
                      <Popconfirm
                        title="Clear all pending authorizations?"
                        description="This will reject all pending requests."
                        onConfirm={handleClearAll}
                        okText="Yes"
                        cancelText="No"
                      >
                        <Button size="small" icon={<ClearOutlined />}>
                          Clear All
                        </Button>
                      </Popconfirm>
                    </div>
                  )}
                </div>
              }
            />
          </Col>
        </Row>
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24} lg={12}>
          <Card title="System Information">
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Version">
                <Text code>v{overview?.system.version}</Text>
              </Descriptions.Item>
              <Descriptions.Item label="Rust Version">
                {overview?.system.rust_version || "Unknown"}
              </Descriptions.Item>
              <Descriptions.Item label="OS">
                {overview?.system.os} ({overview?.system.arch})
              </Descriptions.Item>
              <Descriptions.Item label="PID">
                {overview?.system.pid}
              </Descriptions.Item>
              <Descriptions.Item label="Uptime">
                {overview ? formatUptime(overview.system.uptime_secs) : "-"}
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>

        <Col xs={24} lg={12}>
          <Card
            title="Proxy Configuration"
            extra={
              <Button
                icon={<CopyOutlined />}
                size="small"
                onClick={copyProxyConfig}
              >
                Copy
              </Button>
            }
          >
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Port">
                <Text code>{overview?.server.port || 8899}</Text>
              </Descriptions.Item>
              <Descriptions.Item label="HTTP Proxy">
                <Text code>127.0.0.1:{overview?.server.port || 8899}</Text>
              </Descriptions.Item>
              <Descriptions.Item label="Admin URL">
                <a
                  href={overview?.server.admin_url}
                  target="_blank"
                  rel="noreferrer"
                >
                  {overview?.server.admin_url}
                </a>
              </Descriptions.Item>
              <Descriptions.Item label="System Proxy">
                {systemProxy?.supported ? (
                  <Space>
                    <Switch
                      checked={systemProxy?.enabled}
                      loading={systemProxyLoading}
                      onChange={handleSystemProxyToggle}
                      checkedChildren={<GlobalOutlined />}
                      unCheckedChildren={<GlobalOutlined />}
                    />
                    <Text type={systemProxy?.enabled ? "success" : "secondary"}>
                      {systemProxy?.enabled ? "Enabled" : "Disabled"}
                    </Text>
                  </Space>
                ) : (
                  <Tooltip title="System proxy is not supported on this platform">
                    <Text type="secondary">Not Supported</Text>
                  </Tooltip>
                )}
              </Descriptions.Item>
            </Descriptions>
            <Paragraph
              type="secondary"
              style={{ marginTop: 16, marginBottom: 0 }}
            >
              Configure your browser or system proxy settings to use the above
              address.
            </Paragraph>
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24}>
          <Card title="Performance Metrics">
            <Row gutter={[16, 16]}>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Active Connections"
                    value={overview?.metrics.active_connections || 0}
                    prefix={<SwapOutlined />}
                  />
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Total Requests"
                    value={overview?.metrics.total_requests || 0}
                    prefix={<ApiOutlined />}
                  />
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Current QPS"
                    value={overview?.metrics.qps.toFixed(2) || 0}
                    prefix={<DashboardOutlined />}
                    suffix={
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        max: {overview?.metrics.max_qps.toFixed(2) || 0}
                      </Text>
                    }
                  />
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Recorded Traffic"
                    value={overview?.traffic.recorded || 0}
                    prefix={<DatabaseOutlined />}
                  />
                </Card>
              </Col>
            </Row>

            <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Upload Rate"
                    value={formatBytesRate(
                      overview?.metrics.bytes_sent_rate || 0,
                    )}
                    prefix={
                      <CloudUploadOutlined style={{ color: "#52c41a" }} />
                    }
                  />
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    Max:{" "}
                    {formatBytesRate(
                      overview?.metrics.max_bytes_sent_rate || 0,
                    )}
                  </Text>
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Download Rate"
                    value={formatBytesRate(
                      overview?.metrics.bytes_received_rate || 0,
                    )}
                    prefix={
                      <CloudDownloadOutlined style={{ color: "#1890ff" }} />
                    }
                  />
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    Max:{" "}
                    {formatBytesRate(
                      overview?.metrics.max_bytes_received_rate || 0,
                    )}
                  </Text>
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Total Upload"
                    value={formatBytes(overview?.metrics.bytes_sent || 0)}
                    prefix={
                      <CloudUploadOutlined style={{ color: "#52c41a" }} />
                    }
                  />
                </Card>
              </Col>
              <Col xs={24} sm={12} lg={6}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Statistic
                    title="Total Download"
                    value={formatBytes(overview?.metrics.bytes_received || 0)}
                    prefix={
                      <CloudDownloadOutlined style={{ color: "#1890ff" }} />
                    }
                  />
                </Card>
              </Col>
            </Row>

            <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
              <Col xs={24} sm={12}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Text strong>CPU Usage</Text>
                  <Progress
                    percent={Number(
                      (overview?.metrics.cpu_usage || 0).toFixed(1),
                    )}
                    status={
                      (overview?.metrics.cpu_usage || 0) > 80
                        ? "exception"
                        : "normal"
                    }
                    strokeColor={{
                      "0%": "#108ee9",
                      "100%":
                        (overview?.metrics.cpu_usage || 0) > 80
                          ? "#ff4d4f"
                          : "#87d068",
                    }}
                  />
                </Card>
              </Col>
              <Col xs={24} sm={12}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: "#fafafa" }}
                >
                  <Text strong>Memory Usage</Text>
                  <Progress
                    percent={Number(memoryPercent.toFixed(1))}
                    status={memoryPercent > 80 ? "exception" : "normal"}
                    strokeColor={{
                      "0%": "#108ee9",
                      "100%": memoryPercent > 80 ? "#ff4d4f" : "#87d068",
                    }}
                    format={() =>
                      `${formatBytes(overview?.metrics.memory_used || 0)} / ${formatBytes(overview?.metrics.memory_total || 0)}`
                    }
                  />
                </Card>
              </Col>
            </Row>
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24}>
          <Card title="Usage Guide">
            <Descriptions column={1} size="small" bordered>
              <Descriptions.Item label="Rule Syntax">
                <Paragraph style={{ margin: 0 }}>
                  <Text code>pattern protocol://value</Text>
                  <br />
                  Example: <Text code>*.example.com host://127.0.0.1</Text>
                </Paragraph>
              </Descriptions.Item>
              <Descriptions.Item label="Supported Protocols">
                <Text>
                  host, proxy, pac, reqHeaders, reqBody, resHeaders, resBody,
                  htmlAppend, jsAppend, cssAppend, and more...
                </Text>
              </Descriptions.Item>
              <Descriptions.Item label="Pattern Matching">
                <Text>
                  Supports glob patterns (* and **) and regular expressions
                  (/pattern/)
                </Text>
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic title="Total Rules" value={overview?.rules.total || 0} />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Enabled Rules"
              value={overview?.rules.enabled || 0}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Recorded Traffic"
              value={overview?.traffic.recorded || 0}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Total Requests"
              value={overview?.metrics.total_requests || 0}
            />
          </Card>
        </Col>
      </Row>
    </div>
  );
}

function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${mins}m`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m ${secs % 60}s`;
}
