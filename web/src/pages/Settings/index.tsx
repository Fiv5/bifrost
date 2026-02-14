import { useEffect, useState, useCallback } from "react";
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
  Tabs,
  Input,
  Tag,
  Divider,
  Image,
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
  SafetyCertificateOutlined,
  LockOutlined,
  DownloadOutlined,
  QrcodeOutlined,
  ExclamationCircleOutlined,
  PlusOutlined,
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
import {
  getTlsConfig,
  updateTlsConfig,
  type TlsConfig,
} from "../../api/config";
import {
  getCertInfo,
  getCertDownloadUrl,
  getCertQRCodeUrl,
  type CertInfo,
} from "../../api/cert";
import type { PendingAuth, TrafficTypeMetrics } from "../../types";

const { Text, Paragraph } = Typography;

export default function Settings() {
  const { overview, loading, error, fetchOverview } = useMetricsStore();
  const [pendingList, setPendingList] = useState<PendingAuth[]>([]);
  const [pendingLoading, setPendingLoading] = useState(false);
  const [systemProxy, setSystemProxyState] = useState<SystemProxyStatus | null>(
    null,
  );
  const [systemProxyLoading, setSystemProxyLoading] = useState(false);
  const [tlsConfig, setTlsConfig] = useState<TlsConfig | null>(null);
  const [tlsLoading, setTlsLoading] = useState(false);
  const [certInfo, setCertInfo] = useState<CertInfo | null>(null);
  const [newExcludePattern, setNewExcludePattern] = useState("");
  const [newIncludePattern, setNewIncludePattern] = useState("");

  const fetchSystemProxy = async () => {
    try {
      const status = await getSystemProxyStatus();
      setSystemProxyState(status);
    } catch {
      console.error("Failed to fetch system proxy status");
    }
  };

  const fetchTlsConfig = useCallback(async () => {
    setTlsLoading(true);
    try {
      const config = await getTlsConfig();
      setTlsConfig(config);
    } catch {
      console.error("Failed to fetch TLS config");
    } finally {
      setTlsLoading(false);
    }
  }, []);

  const fetchCertInfo = useCallback(async () => {
    try {
      const info = await getCertInfo();
      setCertInfo(info);
    } catch {
      console.error("Failed to fetch cert info");
    }
  }, []);

  const handleSystemProxyToggle = async (enabled: boolean) => {
    setSystemProxyLoading(true);
    try {
      const result = await setSystemProxy({ enabled });
      setSystemProxyState(result);
      message.success(
        enabled ? "System proxy enabled" : "System proxy disabled",
      );
    } catch {
      message.error("Failed to toggle system proxy");
    } finally {
      setSystemProxyLoading(false);
    }
  };

  const handleTlsInterceptionToggle = async (enabled: boolean) => {
    setTlsLoading(true);
    try {
      const result = await updateTlsConfig({
        enable_tls_interception: enabled,
      });
      setTlsConfig(result);
      message.success(
        enabled ? "HTTPS interception enabled" : "HTTPS interception disabled",
      );
    } catch {
      message.error("Failed to update TLS config");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleUnsafeSslToggle = async (enabled: boolean) => {
    setTlsLoading(true);
    try {
      const result = await updateTlsConfig({ unsafe_ssl: enabled });
      setTlsConfig(result);
      message.success(
        enabled
          ? "Certificate verification disabled"
          : "Certificate verification enabled",
      );
    } catch {
      message.error("Failed to update TLS config");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleDisconnectOnConfigChangeToggle = async (enabled: boolean) => {
    setTlsLoading(true);
    try {
      const result = await updateTlsConfig({
        disconnect_on_config_change: enabled,
      });
      setTlsConfig(result);
      message.success(
        enabled
          ? "Auto-disconnect on config change enabled"
          : "Auto-disconnect on config change disabled",
      );
    } catch {
      message.error("Failed to update TLS config");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleAddExcludePattern = async () => {
    if (!newExcludePattern.trim()) {
      message.warning("Please enter a pattern");
      return;
    }

    const pattern = newExcludePattern.trim();
    if (tlsConfig?.intercept_exclude.includes(pattern)) {
      message.warning("Pattern already exists");
      return;
    }

    setTlsLoading(true);
    try {
      const newList = [...(tlsConfig?.intercept_exclude || []), pattern];
      const result = await updateTlsConfig({ intercept_exclude: newList });
      setTlsConfig(result);
      setNewExcludePattern("");
      message.success(`Added ${pattern} to exclude list`);
    } catch {
      message.error("Failed to add pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleRemoveExcludePattern = async (pattern: string) => {
    setTlsLoading(true);
    try {
      const newList = (tlsConfig?.intercept_exclude || []).filter(
        (p) => p !== pattern,
      );
      const result = await updateTlsConfig({ intercept_exclude: newList });
      setTlsConfig(result);
      message.success(`Removed ${pattern} from exclude list`);
    } catch {
      message.error("Failed to remove pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleAddIncludePattern = async () => {
    if (!newIncludePattern.trim()) {
      message.warning("Please enter a pattern");
      return;
    }

    const pattern = newIncludePattern.trim();
    if (tlsConfig?.intercept_include.includes(pattern)) {
      message.warning("Pattern already exists");
      return;
    }

    setTlsLoading(true);
    try {
      const newList = [...(tlsConfig?.intercept_include || []), pattern];
      const result = await updateTlsConfig({ intercept_include: newList });
      setTlsConfig(result);
      setNewIncludePattern("");
      message.success(`Added ${pattern} to include list`);
    } catch {
      message.error("Failed to add pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleRemoveIncludePattern = async (pattern: string) => {
    setTlsLoading(true);
    try {
      const newList = (tlsConfig?.intercept_include || []).filter(
        (p) => p !== pattern,
      );
      const result = await updateTlsConfig({ intercept_include: newList });
      setTlsConfig(result);
      message.success(`Removed ${pattern} from include list`);
    } catch {
      message.error("Failed to remove pattern");
    } finally {
      setTlsLoading(false);
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
    fetchTlsConfig();
    fetchCertInfo();
    const interval = setInterval(fetchOverview, 1000);
    return () => clearInterval(interval);
  }, [fetchOverview, fetchTlsConfig, fetchCertInfo]);

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
    const config = `HTTP Proxy: 127.0.0.1:${overview?.server.port || 9900}
HTTPS Proxy: 127.0.0.1:${overview?.server.port || 9900}`;
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

  const tabItems = [
    {
      key: "proxy",
      label: (
        <span>
          <GlobalOutlined /> Proxy
        </span>
      ),
      children: (
        <div>
          <Row gutter={[16, 16]}>
            <Col xs={24} lg={12}>
              <Card
                title={
                  <Space>
                    <GlobalOutlined />
                    <span>System Proxy</span>
                  </Space>
                }
                size="small"
              >
                <Space direction="vertical" style={{ width: "100%" }}>
                  <Row justify="space-between" align="middle">
                    <Col>
                      <Text>Enable System Proxy</Text>
                    </Col>
                    <Col>
                      {systemProxy?.supported ? (
                        <Switch
                          checked={systemProxy?.enabled}
                          loading={systemProxyLoading}
                          onChange={handleSystemProxyToggle}
                        />
                      ) : (
                        <Tooltip title="System proxy is not supported on this platform">
                          <Text type="secondary">Not Supported</Text>
                        </Tooltip>
                      )}
                    </Col>
                  </Row>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    Route all system traffic through this proxy
                  </Text>
                </Space>
              </Card>
            </Col>

            <Col xs={24} lg={12}>
              <Card
                title={
                  <Space>
                    <ApiOutlined />
                    <span>Proxy Address</span>
                  </Space>
                }
                size="small"
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
                    <Text code>{overview?.server.port || 9900}</Text>
                  </Descriptions.Item>
                  <Descriptions.Item label="HTTP/HTTPS Proxy">
                    <Text code>127.0.0.1:{overview?.server.port || 9900}</Text>
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
                </Descriptions>
              </Card>
            </Col>
          </Row>

          <Divider />

          <Row gutter={[16, 16]}>
            <Col xs={24} lg={12}>
              <Card
                title={
                  <Space>
                    <LockOutlined />
                    <span>TLS/HTTPS Settings</span>
                  </Space>
                }
                size="small"
                loading={tlsLoading && !tlsConfig}
              >
                <Space
                  direction="vertical"
                  style={{ width: "100%" }}
                  size="middle"
                >
                  <Row justify="space-between" align="middle">
                    <Col>
                      <Text>Enable HTTPS Interception</Text>
                    </Col>
                    <Col>
                      <Switch
                        checked={tlsConfig?.enable_tls_interception}
                        loading={tlsLoading}
                        onChange={handleTlsInterceptionToggle}
                      />
                    </Col>
                  </Row>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    Intercept and inspect HTTPS traffic. Requires CA certificate
                    installed.
                  </Text>

                  <Divider style={{ margin: "12px 0" }} />

                  <Row justify="space-between" align="middle">
                    <Col>
                      <Space>
                        <Text>Skip Certificate Verification</Text>
                        <Tooltip title="Warning: This makes connections insecure">
                          <ExclamationCircleOutlined
                            style={{ color: "#faad14" }}
                          />
                        </Tooltip>
                      </Space>
                    </Col>
                    <Col>
                      <Switch
                        checked={tlsConfig?.unsafe_ssl}
                        loading={tlsLoading}
                        onChange={handleUnsafeSslToggle}
                      />
                    </Col>
                  </Row>
                  {tlsConfig?.unsafe_ssl && (
                    <Alert
                      type="warning"
                      message="Certificate verification is disabled"
                      description="Only use this in development environments"
                      showIcon
                      style={{ marginTop: 8 }}
                    />
                  )}

                  <Divider style={{ margin: "12px 0" }} />

                  <Row justify="space-between" align="middle">
                    <Col>
                      <Tooltip title="Automatically disconnect affected connections when TLS config changes">
                        <Text>Auto-disconnect on Config Change</Text>
                      </Tooltip>
                    </Col>
                    <Col>
                      <Switch
                        checked={tlsConfig?.disconnect_on_config_change}
                        loading={tlsLoading}
                        onChange={handleDisconnectOnConfigChangeToggle}
                      />
                    </Col>
                  </Row>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    When enabled, existing connections will be closed when TLS
                    settings change.
                  </Text>
                </Space>
              </Card>
            </Col>

            <Col xs={24} lg={12}>
              <Card
                title={
                  <Space>
                    <SafetyCertificateOutlined />
                    <span>Exclude Patterns</span>
                    <Tag>{tlsConfig?.intercept_exclude.length || 0}</Tag>
                  </Space>
                }
                size="small"
                extra={
                  <Space.Compact>
                    <Input
                      placeholder="*.example.com"
                      value={newExcludePattern}
                      onChange={(e) => setNewExcludePattern(e.target.value)}
                      onPressEnter={handleAddExcludePattern}
                      style={{ width: 150 }}
                      size="small"
                    />
                    <Button
                      type="primary"
                      icon={<PlusOutlined />}
                      onClick={handleAddExcludePattern}
                      size="small"
                      loading={tlsLoading}
                    >
                      Add
                    </Button>
                  </Space.Compact>
                }
              >
                <Text
                  type="secondary"
                  style={{ display: "block", marginBottom: 8, fontSize: 12 }}
                >
                  Domains matching these patterns will NOT be intercepted
                  (passthrough mode). Has higher priority than global switch.
                  Useful for certificate pinning sites.
                </Text>
                <div style={{ maxHeight: 200, overflowY: "auto" }}>
                  {tlsConfig?.intercept_exclude.length === 0 ? (
                    <Text type="secondary">No exclude patterns configured</Text>
                  ) : (
                    <Space wrap>
                      {tlsConfig?.intercept_exclude.map((pattern) => (
                        <Tag
                          key={pattern}
                          closable
                          onClose={() => handleRemoveExcludePattern(pattern)}
                        >
                          {pattern}
                        </Tag>
                      ))}
                    </Space>
                  )}
                </div>
              </Card>
            </Col>

            <Col xs={24} lg={12}>
              <Card
                title={
                  <Space>
                    <LockOutlined />
                    <span>Force Intercept Patterns</span>
                    <Tag color="orange">
                      {tlsConfig?.intercept_include.length || 0}
                    </Tag>
                  </Space>
                }
                size="small"
                extra={
                  <Space.Compact>
                    <Input
                      placeholder="*.api.example.com"
                      value={newIncludePattern}
                      onChange={(e) => setNewIncludePattern(e.target.value)}
                      onPressEnter={handleAddIncludePattern}
                      style={{ width: 150 }}
                      size="small"
                    />
                    <Button
                      type="primary"
                      icon={<PlusOutlined />}
                      onClick={handleAddIncludePattern}
                      size="small"
                      loading={tlsLoading}
                    >
                      Add
                    </Button>
                  </Space.Compact>
                }
              >
                <Text
                  type="secondary"
                  style={{ display: "block", marginBottom: 8, fontSize: 12 }}
                >
                  Domains matching these patterns will ALWAYS be intercepted,
                  even when global interception is disabled. Has highest
                  priority.
                </Text>
                <div style={{ maxHeight: 200, overflowY: "auto" }}>
                  {tlsConfig?.intercept_include.length === 0 ? (
                    <Text type="secondary">
                      No force intercept patterns configured
                    </Text>
                  ) : (
                    <Space wrap>
                      {tlsConfig?.intercept_include.map((pattern) => (
                        <Tag
                          key={pattern}
                          color="orange"
                          closable
                          onClose={() => handleRemoveIncludePattern(pattern)}
                        >
                          {pattern}
                        </Tag>
                      ))}
                    </Space>
                  )}
                </div>
              </Card>
            </Col>
          </Row>
        </div>
      ),
    },
    {
      key: "certificate",
      label: (
        <span>
          <SafetyCertificateOutlined /> Certificate
        </span>
      ),
      children: (
        <Row gutter={[16, 16]}>
          <Col xs={24} lg={12}>
            <Card
              title={
                <Space>
                  <SafetyCertificateOutlined />
                  <span>CA Certificate</span>
                </Space>
              }
              size="small"
            >
              <Space
                direction="vertical"
                style={{ width: "100%" }}
                size="middle"
              >
                <Row justify="space-between" align="middle">
                  <Col>
                    <Text>Certificate Status</Text>
                  </Col>
                  <Col>
                    {certInfo?.available ? (
                      <Tag color="green" icon={<CheckOutlined />}>
                        Available
                      </Tag>
                    ) : (
                      <Tag color="red" icon={<CloseOutlined />}>
                        Not Found
                      </Tag>
                    )}
                  </Col>
                </Row>

                <Divider style={{ margin: "8px 0" }} />

                <Button
                  type="primary"
                  icon={<DownloadOutlined />}
                  href={getCertDownloadUrl()}
                  download="bifrost-ca.crt"
                  disabled={!certInfo?.available}
                  block
                >
                  Download CA Certificate
                </Button>

                <Text type="secondary" style={{ fontSize: 12 }}>
                  Install this certificate as a trusted root CA on your device
                  to enable HTTPS inspection.
                </Text>
              </Space>
            </Card>
          </Col>

          <Col xs={24} lg={12}>
            <Card
              title={
                <Space>
                  <QrcodeOutlined />
                  <span>Mobile Installation</span>
                </Space>
              }
              size="small"
            >
              <Space
                direction="vertical"
                style={{ width: "100%", alignItems: "center" }}
                size="middle"
              >
                {certInfo?.available ? (
                  <>
                    <Image
                      src={getCertQRCodeUrl()}
                      alt="Certificate QR Code"
                      width={180}
                      height={180}
                      fallback="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mN8/+F9PQAJpAN4pokyXwAAAABJRU5ErkJggg=="
                    />
                    <Text
                      type="secondary"
                      style={{ fontSize: 12, textAlign: "center" }}
                    >
                      Scan with your mobile device to download and install the
                      CA certificate
                    </Text>
                  </>
                ) : (
                  <Text type="secondary">QR code not available</Text>
                )}
              </Space>
            </Card>
          </Col>

          <Col xs={24}>
            <Card
              title={
                <Space>
                  <GlobalOutlined />
                  <span>Available Download URLs</span>
                </Space>
              }
              size="small"
            >
              {certInfo?.download_urls && certInfo.download_urls.length > 0 ? (
                <List
                  size="small"
                  dataSource={certInfo.download_urls}
                  renderItem={(url) => (
                    <List.Item>
                      <a href={url} target="_blank" rel="noreferrer">
                        {url}
                      </a>
                    </List.Item>
                  )}
                />
              ) : (
                <Text type="secondary">No download URLs available</Text>
              )}
            </Card>
          </Col>
        </Row>
      ),
    },
    {
      key: "metrics",
      label: (
        <span>
          <DashboardOutlined /> Metrics
        </span>
      ),
      children: (
        <div>
          <Card title="Performance Metrics" size="small">
            <Tabs
              defaultActiveKey="overview"
              size="small"
              items={[
                {
                  key: "overview",
                  label: "Overview",
                  children: (
                    <MetricsContent
                      activeConnections={
                        overview?.metrics.active_connections || 0
                      }
                      totalRequests={overview?.metrics.total_requests || 0}
                      qps={overview?.metrics.qps || 0}
                      maxQps={overview?.metrics.max_qps || 0}
                      recordedTraffic={overview?.traffic.recorded || 0}
                      bytesSentRate={overview?.metrics.bytes_sent_rate || 0}
                      bytesReceivedRate={
                        overview?.metrics.bytes_received_rate || 0
                      }
                      maxBytesSentRate={
                        overview?.metrics.max_bytes_sent_rate || 0
                      }
                      maxBytesReceivedRate={
                        overview?.metrics.max_bytes_received_rate || 0
                      }
                      bytesSent={overview?.metrics.bytes_sent || 0}
                      bytesReceived={overview?.metrics.bytes_received || 0}
                      formatBytes={formatBytes}
                      formatBytesRate={formatBytesRate}
                    />
                  ),
                },
                {
                  key: "http",
                  label: "HTTP",
                  children: (
                    <TrafficTypeContent
                      metrics={overview?.metrics.http}
                      formatBytes={formatBytes}
                    />
                  ),
                },
                {
                  key: "https",
                  label: "HTTPS",
                  children: (
                    <TrafficTypeContent
                      metrics={overview?.metrics.https}
                      formatBytes={formatBytes}
                    />
                  ),
                },
                {
                  key: "tunnel",
                  label: "Tunnel",
                  children: (
                    <TrafficTypeContent
                      metrics={overview?.metrics.tunnel}
                      formatBytes={formatBytes}
                    />
                  ),
                },
                {
                  key: "ws",
                  label: "WS",
                  children: (
                    <TrafficTypeContent
                      metrics={overview?.metrics.ws}
                      formatBytes={formatBytes}
                    />
                  ),
                },
                {
                  key: "wss",
                  label: "WSS",
                  children: (
                    <TrafficTypeContent
                      metrics={overview?.metrics.wss}
                      formatBytes={formatBytes}
                    />
                  ),
                },
              ]}
            />

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

          <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
            <Col xs={24} sm={12} lg={6}>
              <Card size="small">
                <Statistic
                  title="Total Rules"
                  value={overview?.rules.total || 0}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card size="small">
                <Statistic
                  title="Enabled Rules"
                  value={overview?.rules.enabled || 0}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card size="small">
                <Statistic
                  title="Recorded Traffic"
                  value={overview?.traffic.recorded || 0}
                />
              </Card>
            </Col>
            <Col xs={24} sm={12} lg={6}>
              <Card size="small">
                <Statistic
                  title="Total Requests"
                  value={overview?.metrics.total_requests || 0}
                />
              </Card>
            </Col>
          </Row>
        </div>
      ),
    },
    {
      key: "system",
      label: (
        <span>
          <DatabaseOutlined /> System
        </span>
      ),
      children: (
        <Row gutter={[16, 16]}>
          <Col xs={24} lg={12}>
            <Card title="System Information" size="small">
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
            <Card title="Usage Guide" size="small">
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
      ),
    },
  ];

  return (
    <div style={{ padding: 16 }}>
      {pendingCount > 0 && (
        <Alert
          type="warning"
          showIcon
          icon={<WarningOutlined />}
          style={{ marginBottom: 16 }}
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
      )}

      <Tabs defaultActiveKey="proxy" items={tabItems} />
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

interface MetricsContentProps {
  activeConnections: number;
  totalRequests: number;
  qps: number;
  maxQps: number;
  recordedTraffic: number;
  bytesSentRate: number;
  bytesReceivedRate: number;
  maxBytesSentRate: number;
  maxBytesReceivedRate: number;
  bytesSent: number;
  bytesReceived: number;
  formatBytes: (bytes: number) => string;
  formatBytesRate: (bytesPerSec: number) => string;
}

function MetricsContent({
  activeConnections,
  totalRequests,
  qps,
  maxQps,
  recordedTraffic,
  bytesSentRate,
  bytesReceivedRate,
  maxBytesSentRate,
  maxBytesReceivedRate,
  bytesSent,
  bytesReceived,
  formatBytes,
  formatBytesRate,
}: MetricsContentProps) {
  return (
    <>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Active Connections"
              value={activeConnections}
              prefix={<SwapOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Total Requests"
              value={totalRequests}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Current QPS"
              value={qps.toFixed(2)}
              prefix={<DashboardOutlined />}
              suffix={
                <Text type="secondary" style={{ fontSize: 12 }}>
                  max: {maxQps.toFixed(2)}
                </Text>
              }
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Recorded Traffic"
              value={recordedTraffic}
              prefix={<DatabaseOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Upload Rate"
              value={formatBytesRate(bytesSentRate)}
              prefix={<CloudUploadOutlined style={{ color: "#52c41a" }} />}
            />
            <Text type="secondary" style={{ fontSize: 12 }}>
              Max: {formatBytesRate(maxBytesSentRate)}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Download Rate"
              value={formatBytesRate(bytesReceivedRate)}
              prefix={<CloudDownloadOutlined style={{ color: "#1890ff" }} />}
            />
            <Text type="secondary" style={{ fontSize: 12 }}>
              Max: {formatBytesRate(maxBytesReceivedRate)}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Total Upload"
              value={formatBytes(bytesSent)}
              prefix={<CloudUploadOutlined style={{ color: "#52c41a" }} />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
            <Statistic
              title="Total Download"
              value={formatBytes(bytesReceived)}
              prefix={<CloudDownloadOutlined style={{ color: "#1890ff" }} />}
            />
          </Card>
        </Col>
      </Row>
    </>
  );
}

interface TrafficTypeContentProps {
  metrics?: TrafficTypeMetrics;
  formatBytes: (bytes: number) => string;
}

function TrafficTypeContent({ metrics, formatBytes }: TrafficTypeContentProps) {
  const data = metrics || {
    requests: 0,
    bytes_sent: 0,
    bytes_received: 0,
    active_connections: 0,
  };

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} sm={12} lg={6}>
        <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
          <Statistic
            title="Active Connections"
            value={data.active_connections}
            prefix={<SwapOutlined />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
          <Statistic
            title="Total Requests"
            value={data.requests}
            prefix={<ApiOutlined />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
          <Statistic
            title="Total Upload"
            value={formatBytes(data.bytes_sent)}
            prefix={<CloudUploadOutlined style={{ color: "#52c41a" }} />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card size="small" bordered={false} style={{ background: "#fafafa" }}>
          <Statistic
            title="Total Download"
            value={formatBytes(data.bytes_received)}
            prefix={<CloudDownloadOutlined style={{ color: "#1890ff" }} />}
          />
        </Card>
      </Col>
    </Row>
  );
}
