import { useEffect, useState, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
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
  Segmented,
  theme,
  InputNumber,
  Slider,
  Select,
  Table,
} from "antd";
import type { ColumnsType } from "antd/es/table";
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
  BgColorsOutlined,
  ThunderboltOutlined,
  FolderOutlined,
  FileOutlined,
  DeleteOutlined,
  SafetyOutlined,
  LaptopOutlined,
  ClockCircleOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useMetricsStore } from "../../stores/useMetricsStore";
import {
  getPendingAuthorizations,
  approvePending,
  rejectPending,
  clearPendingAuthorizations,
} from "../../api/whitelist";
import { getAppMetrics } from "../../api/metrics";
import {
  getSystemProxyStatus,
  setSystemProxy,
  getProxyAddressInfo,
  getProxyQRCodeUrl,
  type SystemProxyStatus,
  type ProxyAddressInfo,
} from "../../api/proxy";
import {
  getTlsConfig,
  updateTlsConfig,
  getPerformanceConfig,
  updatePerformanceConfig,
  clearBodyCache,
  type TlsConfig,
  type PerformanceConfig,
} from "../../api/config";
import {
  getCertInfo,
  getCertDownloadUrl,
  getCertQRCodeUrl,
  type CertInfo,
} from "../../api/cert";
import type {
  PendingAuth,
  TrafficTypeMetrics,
  AccessMode,
  AppMetrics,
} from "../../types";
import { useThemeStore, type ThemeMode } from "../../stores/useThemeStore";
import { useWhitelistStore } from "../../stores/useWhitelistStore";
import MetricsChart from "../../components/MetricsChart";

const { Text, Paragraph } = Typography;

const TAB_PARAM = "tab";
const DEFAULT_TAB = "proxy";
const VALID_TABS = [
  "proxy",
  "appearance",
  "certificate",
  "metrics",
  "system",
  "access",
];

export default function Settings() {
  const { overview, history, loading, error, fetchOverview, fetchHistory } =
    useMetricsStore();
  const { mode: themeMode, setMode: setThemeMode } = useThemeStore();
  const { token } = theme.useToken();
  const [searchParams, setSearchParams] = useSearchParams();

  const tabFromUrl = searchParams.get(TAB_PARAM);
  const activeTab =
    tabFromUrl && VALID_TABS.includes(tabFromUrl) ? tabFromUrl : DEFAULT_TAB;

  const handleTabChange = useCallback(
    (key: string) => {
      setSearchParams(
        (prev) => {
          prev.set(TAB_PARAM, key);
          return prev;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

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
  const [newAppExcludePattern, setNewAppExcludePattern] = useState("");
  const [newAppIncludePattern, setNewAppIncludePattern] = useState("");
  const [performanceConfig, setPerformanceConfig] =
    useState<PerformanceConfig | null>(null);
  const [perfLoading, setPerfLoading] = useState(false);
  const [appMetrics, setAppMetrics] = useState<AppMetrics[]>([]);
  const [appMetricsLoading, setAppMetricsLoading] = useState(false);
  const [proxyAddressInfo, setProxyAddressInfo] =
    useState<ProxyAddressInfo | null>(null);
  const [selectedProxyIp, setSelectedProxyIp] = useState<string>("");

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

  const fetchPerformanceConfig = useCallback(async () => {
    setPerfLoading(true);
    try {
      const config = await getPerformanceConfig();
      setPerformanceConfig(config);
    } catch {
      console.error("Failed to fetch performance config");
    } finally {
      setPerfLoading(false);
    }
  }, []);

  const fetchAppMetricsData = useCallback(async () => {
    setAppMetricsLoading(true);
    try {
      const metrics = await getAppMetrics();
      setAppMetrics(metrics);
    } catch {
      console.error("Failed to fetch app metrics");
    } finally {
      setAppMetricsLoading(false);
    }
  }, []);

  const fetchProxyAddressInfo = useCallback(async () => {
    try {
      const info = await getProxyAddressInfo();
      setProxyAddressInfo(info);
      if (info.addresses.length > 0 && !selectedProxyIp) {
        setSelectedProxyIp(info.addresses[0].ip);
      }
    } catch {
      console.error("Failed to fetch proxy address info");
    }
  }, [selectedProxyIp]);

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

  const handleAddAppExcludePattern = async () => {
    if (!newAppExcludePattern.trim()) {
      message.warning("Please enter a pattern");
      return;
    }

    const pattern = newAppExcludePattern.trim();
    if (tlsConfig?.app_intercept_exclude.includes(pattern)) {
      message.warning("Pattern already exists");
      return;
    }

    setTlsLoading(true);
    try {
      const newList = [...(tlsConfig?.app_intercept_exclude || []), pattern];
      const result = await updateTlsConfig({ app_intercept_exclude: newList });
      setTlsConfig(result);
      setNewAppExcludePattern("");
      message.success(`Added ${pattern} to app exclude list`);
    } catch {
      message.error("Failed to add pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleRemoveAppExcludePattern = async (pattern: string) => {
    setTlsLoading(true);
    try {
      const newList = (tlsConfig?.app_intercept_exclude || []).filter(
        (p) => p !== pattern,
      );
      const result = await updateTlsConfig({ app_intercept_exclude: newList });
      setTlsConfig(result);
      message.success(`Removed ${pattern} from app exclude list`);
    } catch {
      message.error("Failed to remove pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleAddAppIncludePattern = async () => {
    if (!newAppIncludePattern.trim()) {
      message.warning("Please enter a pattern");
      return;
    }

    const pattern = newAppIncludePattern.trim();
    if (tlsConfig?.app_intercept_include.includes(pattern)) {
      message.warning("Pattern already exists");
      return;
    }

    setTlsLoading(true);
    try {
      const newList = [...(tlsConfig?.app_intercept_include || []), pattern];
      const result = await updateTlsConfig({ app_intercept_include: newList });
      setTlsConfig(result);
      setNewAppIncludePattern("");
      message.success(`Added ${pattern} to app include list`);
    } catch {
      message.error("Failed to add pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleRemoveAppIncludePattern = async (pattern: string) => {
    setTlsLoading(true);
    try {
      const newList = (tlsConfig?.app_intercept_include || []).filter(
        (p) => p !== pattern,
      );
      const result = await updateTlsConfig({ app_intercept_include: newList });
      setTlsConfig(result);
      message.success(`Removed ${pattern} from app include list`);
    } catch {
      message.error("Failed to remove pattern");
    } finally {
      setTlsLoading(false);
    }
  };

  const handleUpdateMaxRecords = async (value: number) => {
    setPerfLoading(true);
    try {
      const result = await updatePerformanceConfig({ max_records: value });
      setPerformanceConfig(result);
      message.success(`Max records updated to ${value}`);
    } catch {
      message.error("Failed to update max records");
    } finally {
      setPerfLoading(false);
    }
  };

  const handleUpdateMaxBodyMemorySize = async (value: number) => {
    setPerfLoading(true);
    try {
      const result = await updatePerformanceConfig({
        max_body_memory_size: value,
      });
      setPerformanceConfig(result);
      message.success("Max body memory size updated");
    } catch {
      message.error("Failed to update max body memory size");
    } finally {
      setPerfLoading(false);
    }
  };

  const handleUpdateMaxBodyBufferSize = async (value: number) => {
    setPerfLoading(true);
    try {
      const result = await updatePerformanceConfig({
        max_body_buffer_size: value,
      });
      setPerformanceConfig(result);
      message.success("Max body buffer size updated");
    } catch {
      message.error("Failed to update max body buffer size");
    } finally {
      setPerfLoading(false);
    }
  };

  const handleUpdateFileRetentionDays = async (value: number) => {
    setPerfLoading(true);
    try {
      const result = await updatePerformanceConfig({
        file_retention_days: value,
      });
      setPerformanceConfig(result);
      message.success(`File retention updated to ${value} days`);
    } catch {
      message.error("Failed to update file retention days");
    } finally {
      setPerfLoading(false);
    }
  };

  const handleClearBodyCache = async () => {
    setPerfLoading(true);
    try {
      const result = await clearBodyCache();
      message.success(result.message);
      fetchPerformanceConfig();
    } catch {
      message.error("Failed to clear body cache");
    } finally {
      setPerfLoading(false);
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
    fetchHistory(3600);
    fetchSystemProxy();
    fetchTlsConfig();
    fetchCertInfo();
    fetchPerformanceConfig();
    fetchAppMetricsData();
    fetchProxyAddressInfo();
    const interval = setInterval(fetchOverview, 1000);
    const historyInterval = setInterval(() => fetchHistory(3600), 5000);
    const appMetricsInterval = setInterval(fetchAppMetricsData, 5000);
    return () => {
      clearInterval(interval);
      clearInterval(historyInterval);
      clearInterval(appMetricsInterval);
    };
  }, [
    fetchOverview,
    fetchHistory,
    fetchTlsConfig,
    fetchCertInfo,
    fetchPerformanceConfig,
    fetchAppMetricsData,
    fetchProxyAddressInfo,
  ]);

  useEffect(() => {
    fetchPending();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [overview?.pending_authorizations]);

  const { fetchStatus: fetchWhitelistStatus } = useWhitelistStore();

  const handleApprove = async (ip: string) => {
    try {
      await approvePending(ip);
      message.success(`Approved ${ip}`);
      fetchOverview();
      fetchPending();
      fetchWhitelistStatus();
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
            <Col xs={24}>
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

            <Col xs={24}>
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
                <Row gutter={16}>
                  <Col flex="auto">
                    <Descriptions column={1} size="small">
                      <Descriptions.Item label="Port">
                        <Text code>{overview?.server.port || 9900}</Text>
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
                    {proxyAddressInfo &&
                      proxyAddressInfo.addresses.length > 0 && (
                        <>
                          <Divider style={{ margin: "12px 0" }} />
                          <Text
                            type="secondary"
                            style={{
                              fontSize: 12,
                              display: "block",
                              marginBottom: 8,
                            }}
                          >
                            Available Network Addresses (select for QR code)
                          </Text>
                          <Space wrap size={[8, 8]}>
                            {proxyAddressInfo.addresses.map((addr) => (
                              <Tag
                                key={addr.ip}
                                color={
                                  selectedProxyIp === addr.ip
                                    ? "blue"
                                    : "default"
                                }
                                style={{ cursor: "pointer" }}
                                onClick={() => setSelectedProxyIp(addr.ip)}
                              >
                                <Text
                                  code
                                  style={{
                                    color:
                                      selectedProxyIp === addr.ip
                                        ? "#1890ff"
                                        : undefined,
                                  }}
                                >
                                  {addr.address}
                                </Text>
                              </Tag>
                            ))}
                          </Space>
                        </>
                      )}
                  </Col>
                  <Col>
                    <Tooltip
                      title={
                        selectedProxyIp
                          ? `Scan to connect: ${selectedProxyIp}:${overview?.server.port || 9900}`
                          : "Scan with mobile device to configure proxy"
                      }
                    >
                      <div style={{ textAlign: "center" }}>
                        <Image
                          src={getProxyQRCodeUrl(selectedProxyIp || undefined)}
                          alt="Proxy QR Code"
                          width={100}
                          height={100}
                          preview={{
                            mask: <QrcodeOutlined style={{ fontSize: 20 }} />,
                          }}
                          fallback="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mN8/+F9PQAJpAN4pokyXwAAAABJRU5ErkJggg=="
                        />
                        <div style={{ marginTop: 4 }}>
                          <Text type="secondary" style={{ fontSize: 11 }}>
                            <QrcodeOutlined /> Scan to connect
                          </Text>
                        </div>
                      </div>
                    </Tooltip>
                  </Col>
                </Row>
              </Card>
            </Col>

            <Col xs={24}>
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

            <Col xs={24}>
              <TlsInterceptionPatternsCard
                tlsConfig={tlsConfig}
                tlsLoading={tlsLoading}
                newIncludePattern={newIncludePattern}
                newExcludePattern={newExcludePattern}
                newAppIncludePattern={newAppIncludePattern}
                newAppExcludePattern={newAppExcludePattern}
                setNewIncludePattern={setNewIncludePattern}
                setNewExcludePattern={setNewExcludePattern}
                setNewAppIncludePattern={setNewAppIncludePattern}
                setNewAppExcludePattern={setNewAppExcludePattern}
                handleAddIncludePattern={handleAddIncludePattern}
                handleRemoveIncludePattern={handleRemoveIncludePattern}
                handleAddExcludePattern={handleAddExcludePattern}
                handleRemoveExcludePattern={handleRemoveExcludePattern}
                handleAddAppIncludePattern={handleAddAppIncludePattern}
                handleRemoveAppIncludePattern={handleRemoveAppIncludePattern}
                handleAddAppExcludePattern={handleAddAppExcludePattern}
                handleRemoveAppExcludePattern={handleRemoveAppExcludePattern}
              />
            </Col>

            <Col xs={24}>
              <Card
                title={
                  <Space>
                    <ThunderboltOutlined />
                    <span>Performance</span>
                  </Space>
                }
                size="small"
                loading={perfLoading && !performanceConfig}
              >
                <Space
                  direction="vertical"
                  style={{ width: "100%" }}
                  size="middle"
                >
                  <Row justify="space-between" align="middle">
                    <Col>
                      <Space direction="vertical" size={0}>
                        <Text>Max Records</Text>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          Maximum number of traffic records to keep in memory
                        </Text>
                      </Space>
                    </Col>
                    <Col>
                      <InputNumber
                        min={100}
                        max={100000}
                        value={performanceConfig?.traffic.max_records}
                        onChange={(value) =>
                          value && handleUpdateMaxRecords(value)
                        }
                        style={{ width: 120 }}
                      />
                    </Col>
                  </Row>

                  <Divider style={{ margin: "12px 0" }} />

                  <Row justify="space-between" align="middle">
                    <Col flex="1" style={{ marginRight: 16 }}>
                      <Space
                        direction="vertical"
                        size={0}
                        style={{ width: "100%" }}
                      >
                        <Text>Max Body Memory Size</Text>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          Bodies larger than this will be stored to disk
                        </Text>
                        <Slider
                          min={64 * 1024}
                          max={10 * 1024 * 1024}
                          step={64 * 1024}
                          value={
                            performanceConfig?.traffic.max_body_memory_size
                          }
                          onChange={(value) =>
                            handleUpdateMaxBodyMemorySize(value)
                          }
                          tooltip={{
                            formatter: (value) =>
                              value ? formatBytes(value) : "",
                          }}
                        />
                      </Space>
                    </Col>
                    <Col>
                      <Text code>
                        {formatBytes(
                          performanceConfig?.traffic.max_body_memory_size || 0,
                        )}
                      </Text>
                    </Col>
                  </Row>

                  <Divider style={{ margin: "12px 0" }} />

                  <Row justify="space-between" align="middle">
                    <Col flex="1" style={{ marginRight: 16 }}>
                      <Space
                        direction="vertical"
                        size={0}
                        style={{ width: "100%" }}
                      >
                        <Text>Max Body Buffer Size</Text>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          Maximum size of body to capture (larger bodies will be
                          truncated)
                        </Text>
                        <Slider
                          min={1 * 1024 * 1024}
                          max={64 * 1024 * 1024}
                          step={1 * 1024 * 1024}
                          value={
                            performanceConfig?.traffic.max_body_buffer_size
                          }
                          onChange={(value) =>
                            handleUpdateMaxBodyBufferSize(value)
                          }
                          tooltip={{
                            formatter: (value) =>
                              value ? formatBytes(value) : "",
                          }}
                        />
                      </Space>
                    </Col>
                    <Col>
                      <Text code>
                        {formatBytes(
                          performanceConfig?.traffic.max_body_buffer_size || 0,
                        )}
                      </Text>
                    </Col>
                  </Row>

                  <Divider style={{ margin: "12px 0" }} />

                  <Row justify="space-between" align="middle">
                    <Col flex="1" style={{ marginRight: 16 }}>
                      <Space
                        direction="vertical"
                        size={0}
                        style={{ width: "100%" }}
                      >
                        <Text>File Retention Days</Text>
                        <Text type="secondary" style={{ fontSize: 12 }}>
                          Number of days to keep body files on disk
                        </Text>
                        <Slider
                          min={1}
                          max={7}
                          step={1}
                          value={performanceConfig?.traffic.file_retention_days}
                          onChange={(value) =>
                            handleUpdateFileRetentionDays(value)
                          }
                          marks={{ 1: "1d", 3: "3d", 5: "5d", 7: "7d" }}
                        />
                      </Space>
                    </Col>
                    <Col>
                      <Text code>
                        {performanceConfig?.traffic.file_retention_days || 0}{" "}
                        days
                      </Text>
                    </Col>
                  </Row>

                  {(performanceConfig?.body_store_stats ||
                    performanceConfig?.traffic_store_stats ||
                    performanceConfig?.frame_store_stats) && (
                    <>
                      <Divider style={{ margin: "12px 0" }} />
                      <Card
                        size="small"
                        bordered={false}
                        style={{ background: token.colorBgLayout }}
                      >
                        <Row gutter={[16, 8]} align="middle">
                          <Col flex="auto">
                            <Space>
                              <FolderOutlined />
                              <Text strong>File Storage Statistics</Text>
                            </Space>
                          </Col>
                          <Col>
                            <Popconfirm
                              title="Clear all cache files?"
                              description="This will delete all cached data including body files, traffic records, and WebSocket frames."
                              onConfirm={handleClearBodyCache}
                              okText="Clear"
                              cancelText="Cancel"
                              okButtonProps={{ danger: true }}
                            >
                              <Button
                                size="small"
                                danger
                                icon={<DeleteOutlined />}
                                loading={perfLoading}
                              >
                                Clear Cache
                              </Button>
                            </Popconfirm>
                          </Col>
                        </Row>
                        <Row gutter={[16, 8]} style={{ marginTop: 12 }}>
                          <Col xs={8}>
                            <Space direction="vertical" size={0}>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                Body Cache
                              </Text>
                              <Space>
                                <FileOutlined />
                                <Text>
                                  {performanceConfig.body_store_stats
                                    ?.file_count ?? 0}{" "}
                                  files
                                </Text>
                              </Space>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                {formatBytes(
                                  performanceConfig.body_store_stats
                                    ?.total_size ?? 0,
                                )}
                              </Text>
                            </Space>
                          </Col>
                          <Col xs={8}>
                            <Space direction="vertical" size={0}>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                Traffic Records
                              </Text>
                              <Space>
                                <DatabaseOutlined />
                                <Text>
                                  {performanceConfig.traffic_store_stats
                                    ?.record_count ?? 0}{" "}
                                  records
                                </Text>
                              </Space>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                {formatBytes(
                                  performanceConfig.traffic_store_stats
                                    ?.file_size ?? 0,
                                )}
                              </Text>
                            </Space>
                          </Col>
                          <Col xs={8}>
                            <Space direction="vertical" size={0}>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                WebSocket Frames
                              </Text>
                              <Space>
                                <SwapOutlined />
                                <Text>
                                  {performanceConfig.frame_store_stats
                                    ?.connection_count ?? 0}{" "}
                                  connections
                                </Text>
                              </Space>
                              <Text type="secondary" style={{ fontSize: 12 }}>
                                {formatBytes(
                                  performanceConfig.frame_store_stats
                                    ?.total_size ?? 0,
                                )}
                              </Text>
                            </Space>
                          </Col>
                        </Row>
                        <Divider style={{ margin: "8px 0" }} />
                        <Row>
                          <Col>
                            <Space>
                              <Text type="secondary">Total Storage:</Text>
                              <Text strong>
                                {formatBytes(
                                  (performanceConfig.body_store_stats
                                    ?.total_size ?? 0) +
                                    (performanceConfig.traffic_store_stats
                                      ?.file_size ?? 0) +
                                    (performanceConfig.frame_store_stats
                                      ?.total_size ?? 0),
                                )}
                              </Text>
                            </Space>
                          </Col>
                        </Row>
                      </Card>
                    </>
                  )}
                </Space>
              </Card>
            </Col>
          </Row>
        </div>
      ),
    },
    {
      key: "appearance",
      label: (
        <span>
          <BgColorsOutlined /> Appearance
        </span>
      ),
      children: (
        <Row gutter={[16, 16]}>
          <Col xs={24}>
            <Card
              title={
                <Space>
                  <BgColorsOutlined />
                  <span>Theme</span>
                </Space>
              }
              size="small"
            >
              <Space direction="vertical" style={{ width: "100%" }}>
                <Row justify="space-between" align="middle">
                  <Col>
                    <Text>Color Mode</Text>
                  </Col>
                  <Col>
                    <Segmented
                      value={themeMode}
                      onChange={(value) => setThemeMode(value as ThemeMode)}
                      options={[
                        { label: "Light", value: "light" },
                        { label: "Dark", value: "dark" },
                        { label: "System", value: "system" },
                      ]}
                    />
                  </Col>
                </Row>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  Choose your preferred color theme. System mode will
                  automatically follow your operating system settings.
                </Text>
              </Space>
            </Card>
          </Col>
        </Row>
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
          <Col xs={24}>
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

          <Col xs={24}>
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
                      src={getCertQRCodeUrl(selectedProxyIp || undefined)}
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
                      {selectedProxyIp && (
                        <>
                          <br />
                          <Text code style={{ fontSize: 11 }}>
                            {selectedProxyIp}
                          </Text>
                        </>
                      )}
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
                {
                  key: "apps",
                  label: "Applications",
                  children: (
                    <AppMetricsContent
                      appMetrics={appMetrics}
                      loading={appMetricsLoading}
                      formatBytes={formatBytes}
                      onRefresh={fetchAppMetricsData}
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
                  style={{ background: token.colorBgLayout }}
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
                  {history.length > 0 && (
                    <div style={{ marginTop: 12 }}>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        Last Hour
                      </Text>
                      <MetricsChart data={history} type="cpu" height={120} />
                    </div>
                  )}
                </Card>
              </Col>
              <Col xs={24} sm={12}>
                <Card
                  size="small"
                  bordered={false}
                  style={{ background: token.colorBgLayout }}
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
                  {history.length > 0 && (
                    <div style={{ marginTop: 12 }}>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        Last Hour
                      </Text>
                      <MetricsChart data={history} type="memory" height={120} />
                    </div>
                  )}
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
          <Col xs={24}>
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

          <Col xs={24}>
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
    {
      key: "access",
      label: (
        <span>
          <SafetyOutlined /> Access Control
        </span>
      ),
      children: <AccessControlTab />,
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

      <Tabs activeKey={activeTab} onChange={handleTabChange} items={tabItems} />
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
  const { token } = theme.useToken();

  return (
    <>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Active Connections"
              value={activeConnections}
              prefix={<SwapOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Requests"
              value={totalRequests}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
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
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
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
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Upload Rate"
              value={formatBytesRate(bytesSentRate)}
              prefix={
                <CloudUploadOutlined style={{ color: token.colorSuccess }} />
              }
            />
            <Text type="secondary" style={{ fontSize: 12 }}>
              Max: {formatBytesRate(maxBytesSentRate)}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Download Rate"
              value={formatBytesRate(bytesReceivedRate)}
              prefix={
                <CloudDownloadOutlined style={{ color: token.colorInfo }} />
              }
            />
            <Text type="secondary" style={{ fontSize: 12 }}>
              Max: {formatBytesRate(maxBytesReceivedRate)}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Upload"
              value={formatBytes(bytesSent)}
              prefix={
                <CloudUploadOutlined style={{ color: token.colorSuccess }} />
              }
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Download"
              value={formatBytes(bytesReceived)}
              prefix={
                <CloudDownloadOutlined style={{ color: token.colorInfo }} />
              }
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
  const { token } = theme.useToken();
  const data = metrics || {
    requests: 0,
    bytes_sent: 0,
    bytes_received: 0,
    active_connections: 0,
  };

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} sm={12} lg={6}>
        <Card
          size="small"
          bordered={false}
          style={{ background: token.colorBgLayout }}
        >
          <Statistic
            title="Active Connections"
            value={data.active_connections}
            prefix={<SwapOutlined />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card
          size="small"
          bordered={false}
          style={{ background: token.colorBgLayout }}
        >
          <Statistic
            title="Total Requests"
            value={data.requests}
            prefix={<ApiOutlined />}
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card
          size="small"
          bordered={false}
          style={{ background: token.colorBgLayout }}
        >
          <Statistic
            title="Total Upload"
            value={formatBytes(data.bytes_sent)}
            prefix={
              <CloudUploadOutlined style={{ color: token.colorSuccess }} />
            }
          />
        </Card>
      </Col>
      <Col xs={24} sm={12} lg={6}>
        <Card
          size="small"
          bordered={false}
          style={{ background: token.colorBgLayout }}
        >
          <Statistic
            title="Total Download"
            value={formatBytes(data.bytes_received)}
            prefix={
              <CloudDownloadOutlined style={{ color: token.colorInfo }} />
            }
          />
        </Card>
      </Col>
    </Row>
  );
}

interface AppMetricsContentProps {
  appMetrics: AppMetrics[];
  loading: boolean;
  formatBytes: (bytes: number) => string;
  onRefresh: () => void;
}

function AppMetricsContent({
  appMetrics,
  loading,
  formatBytes,
  onRefresh,
}: AppMetricsContentProps) {
  const { token } = theme.useToken();

  const columns: ColumnsType<AppMetrics> = [
    {
      title: "Application",
      dataIndex: "app_name",
      key: "app_name",
      fixed: "left" as const,
      width: 200,
      render: (name: string) => (
        <Space>
          <LaptopOutlined style={{ color: token.colorPrimary }} />
          <Text strong>{name}</Text>
        </Space>
      ),
    },
    {
      title: "Requests",
      dataIndex: "requests",
      key: "requests",
      width: 100,
      sorter: (a, b) => a.requests - b.requests,
      render: (val: number) => val.toLocaleString(),
    },
    {
      title: "Upload",
      dataIndex: "bytes_sent",
      key: "bytes_sent",
      width: 100,
      sorter: (a, b) => a.bytes_sent - b.bytes_sent,
      render: (val: number) => formatBytes(val),
    },
    {
      title: "Download",
      dataIndex: "bytes_received",
      key: "bytes_received",
      width: 100,
      sorter: (a, b) => a.bytes_received - b.bytes_received,
      render: (val: number) => formatBytes(val),
    },
    {
      title: "HTTP",
      dataIndex: "http_requests",
      key: "http_requests",
      width: 80,
      render: (val: number) => val.toLocaleString(),
    },
    {
      title: "HTTPS",
      dataIndex: "https_requests",
      key: "https_requests",
      width: 80,
      render: (val: number) => val.toLocaleString(),
    },
    {
      title: "Tunnel",
      dataIndex: "tunnel_requests",
      key: "tunnel_requests",
      width: 80,
      render: (val: number) => val.toLocaleString(),
    },
    {
      title: "WS",
      dataIndex: "ws_requests",
      key: "ws_requests",
      width: 80,
      render: (val: number) => val.toLocaleString(),
    },
    {
      title: "WSS",
      dataIndex: "wss_requests",
      key: "wss_requests",
      width: 80,
      render: (val: number) => val.toLocaleString(),
    },
  ];

  const totalStats = appMetrics.reduce(
    (acc, app) => ({
      requests: acc.requests + app.requests,
      bytes_sent: acc.bytes_sent + app.bytes_sent,
      bytes_received: acc.bytes_received + app.bytes_received,
    }),
    { requests: 0, bytes_sent: 0, bytes_received: 0 },
  );

  return (
    <div>
      <Row gutter={[16, 16]} style={{ marginBottom: 16 }}>
        <Col xs={24} sm={8}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Applications"
              value={appMetrics.length}
              prefix={<LaptopOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={8}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Requests"
              value={totalStats.requests.toLocaleString()}
              prefix={<ApiOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={8}>
          <Card
            size="small"
            bordered={false}
            style={{ background: token.colorBgLayout }}
          >
            <Statistic
              title="Total Traffic"
              value={formatBytes(
                totalStats.bytes_sent + totalStats.bytes_received,
              )}
              prefix={<SwapOutlined />}
            />
          </Card>
        </Col>
      </Row>

      <div
        style={{
          marginBottom: 16,
          display: "flex",
          justifyContent: "flex-end",
        }}
      >
        <Button
          icon={<ReloadOutlined />}
          onClick={onRefresh}
          loading={loading}
          size="small"
        >
          Refresh
        </Button>
      </div>

      <Table
        columns={columns}
        dataSource={appMetrics}
        rowKey="app_name"
        loading={loading}
        size="small"
        scroll={{ x: 900 }}
        pagination={{
          pageSize: 10,
          showSizeChanger: true,
          showTotal: (total) => `Total ${total} applications`,
        }}
      />
    </div>
  );
}

interface TlsInterceptionPatternsCardProps {
  tlsConfig: TlsConfig | null;
  tlsLoading: boolean;
  newIncludePattern: string;
  newExcludePattern: string;
  newAppIncludePattern: string;
  newAppExcludePattern: string;
  setNewIncludePattern: (pattern: string) => void;
  setNewExcludePattern: (pattern: string) => void;
  setNewAppIncludePattern: (pattern: string) => void;
  setNewAppExcludePattern: (pattern: string) => void;
  handleAddIncludePattern: () => void;
  handleRemoveIncludePattern: (pattern: string) => void;
  handleAddExcludePattern: () => void;
  handleRemoveExcludePattern: (pattern: string) => void;
  handleAddAppIncludePattern: () => void;
  handleRemoveAppIncludePattern: (pattern: string) => void;
  handleAddAppExcludePattern: () => void;
  handleRemoveAppExcludePattern: (pattern: string) => void;
}

function TlsInterceptionPatternsCard({
  tlsConfig,
  tlsLoading,
  newIncludePattern,
  newExcludePattern,
  newAppIncludePattern,
  newAppExcludePattern,
  setNewIncludePattern,
  setNewExcludePattern,
  setNewAppIncludePattern,
  setNewAppExcludePattern,
  handleAddIncludePattern,
  handleRemoveIncludePattern,
  handleAddExcludePattern,
  handleRemoveExcludePattern,
  handleAddAppIncludePattern,
  handleRemoveAppIncludePattern,
  handleAddAppExcludePattern,
  handleRemoveAppExcludePattern,
}: TlsInterceptionPatternsCardProps) {
  const { token } = theme.useToken();

  return (
    <Card
      title={
        <Space>
          <SwapOutlined />
          <span>TLS Interception Patterns</span>
        </Space>
      }
      size="small"
    >
      <Text
        type="secondary"
        style={{ display: "block", marginBottom: 16, fontSize: 12 }}
      >
        Configure TLS interception behavior by domain or application. Priority:
        Rules &gt; App Include &gt; App Exclude &gt; Domain Include &gt; Domain
        Exclude &gt; Global.
      </Text>
      <Row gutter={[16, 16]}>
        <Col xs={24}>
          <div
            style={{
              padding: 16,
              background: token.colorSuccessBg,
              borderRadius: 8,
              border: `1px solid ${token.colorSuccessBorder}`,
            }}
          >
            <Space
              style={{
                width: "100%",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <Space>
                <LockOutlined style={{ color: token.colorSuccess }} />
                <Text strong style={{ color: token.colorSuccessText }}>
                  Force Intercept
                </Text>
                <Tag color="green">
                  {tlsConfig?.intercept_include.length || 0}
                </Tag>
              </Space>
            </Space>
            <Text
              type="secondary"
              style={{
                display: "block",
                marginBottom: 12,
                fontSize: 12,
              }}
            >
              Always intercept these domains, even when global interception is
              OFF. Highest priority.
            </Text>
            <Space.Compact style={{ width: "100%", marginBottom: 12 }}>
              <Input
                placeholder="*.api.example.com"
                value={newIncludePattern}
                onChange={(e) => setNewIncludePattern(e.target.value)}
                onPressEnter={handleAddIncludePattern}
                size="small"
              />
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={handleAddIncludePattern}
                size="small"
                loading={tlsLoading}
                style={{
                  background: token.colorSuccess,
                  borderColor: token.colorSuccess,
                }}
              >
                Add
              </Button>
            </Space.Compact>
            <div style={{ maxHeight: 150, overflowY: "auto" }}>
              {tlsConfig?.intercept_include.length === 0 ? (
                <Text type="secondary">No patterns configured</Text>
              ) : (
                <Space wrap>
                  {tlsConfig?.intercept_include.map((pattern) => (
                    <Tag
                      key={pattern}
                      color="green"
                      closable
                      onClose={() => handleRemoveIncludePattern(pattern)}
                    >
                      {pattern}
                    </Tag>
                  ))}
                </Space>
              )}
            </div>
          </div>
        </Col>
        <Col xs={24}>
          <div
            style={{
              padding: 16,
              background: token.colorWarningBg,
              borderRadius: 8,
              border: `1px solid ${token.colorWarningBorder}`,
            }}
          >
            <Space
              style={{
                width: "100%",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <Space>
                <SafetyCertificateOutlined
                  style={{ color: token.colorWarning }}
                />
                <Text strong style={{ color: token.colorWarningText }}>
                  Passthrough (No Intercept)
                </Text>
                <Tag color="orange">
                  {tlsConfig?.intercept_exclude.length || 0}
                </Tag>
              </Space>
            </Space>
            <Text
              type="secondary"
              style={{
                display: "block",
                marginBottom: 12,
                fontSize: 12,
              }}
            >
              Never intercept these domains, even when global interception is
              ON. For certificate pinning sites.
            </Text>
            <Space.Compact style={{ width: "100%", marginBottom: 12 }}>
              <Input
                placeholder="*.apple.com"
                value={newExcludePattern}
                onChange={(e) => setNewExcludePattern(e.target.value)}
                onPressEnter={handleAddExcludePattern}
                size="small"
              />
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={handleAddExcludePattern}
                size="small"
                loading={tlsLoading}
                style={{
                  background: token.colorWarning,
                  borderColor: token.colorWarning,
                }}
              >
                Add
              </Button>
            </Space.Compact>
            <div style={{ maxHeight: 150, overflowY: "auto" }}>
              {tlsConfig?.intercept_exclude.length === 0 ? (
                <Text type="secondary">No patterns configured</Text>
              ) : (
                <Space wrap>
                  {tlsConfig?.intercept_exclude.map((pattern) => (
                    <Tag
                      key={pattern}
                      color="orange"
                      closable
                      onClose={() => handleRemoveExcludePattern(pattern)}
                    >
                      {pattern}
                    </Tag>
                  ))}
                </Space>
              )}
            </div>
          </div>
        </Col>
      </Row>
      <Divider style={{ margin: "16px 0" }}>
        <Text type="secondary" style={{ fontSize: 12 }}>
          Application-based Filtering
        </Text>
      </Divider>
      <Row gutter={[16, 16]}>
        <Col xs={24}>
          <div
            style={{
              padding: 16,
              background: token.colorSuccessBg,
              borderRadius: 8,
              border: `1px solid ${token.colorSuccessBorder}`,
            }}
          >
            <Space
              style={{
                width: "100%",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <Space>
                <LockOutlined style={{ color: token.colorSuccess }} />
                <Text strong style={{ color: token.colorSuccessText }}>
                  App Force Intercept
                </Text>
                <Tag color="green">
                  {tlsConfig?.app_intercept_include.length || 0}
                </Tag>
              </Space>
            </Space>
            <Text
              type="secondary"
              style={{
                display: "block",
                marginBottom: 12,
                fontSize: 12,
              }}
            >
              Always intercept traffic from these apps. Supports: exact match,
              prefix* (starts with), *suffix (ends with).
            </Text>
            <Space.Compact style={{ width: "100%", marginBottom: 12 }}>
              <Input
                placeholder="Chrome*, *Browser, Postman"
                value={newAppIncludePattern}
                onChange={(e) => setNewAppIncludePattern(e.target.value)}
                onPressEnter={handleAddAppIncludePattern}
                size="small"
              />
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={handleAddAppIncludePattern}
                size="small"
                loading={tlsLoading}
                style={{
                  background: token.colorSuccess,
                  borderColor: token.colorSuccess,
                }}
              >
                Add
              </Button>
            </Space.Compact>
            <div style={{ maxHeight: 150, overflowY: "auto" }}>
              {tlsConfig?.app_intercept_include.length === 0 ? (
                <Text type="secondary">No patterns configured</Text>
              ) : (
                <Space wrap>
                  {tlsConfig?.app_intercept_include.map((pattern) => (
                    <Tag
                      key={pattern}
                      color="green"
                      closable
                      onClose={() => handleRemoveAppIncludePattern(pattern)}
                    >
                      {pattern}
                    </Tag>
                  ))}
                </Space>
              )}
            </div>
          </div>
        </Col>
        <Col xs={24}>
          <div
            style={{
              padding: 16,
              background: token.colorWarningBg,
              borderRadius: 8,
              border: `1px solid ${token.colorWarningBorder}`,
            }}
          >
            <Space
              style={{
                width: "100%",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <Space>
                <SafetyCertificateOutlined
                  style={{ color: token.colorWarning }}
                />
                <Text strong style={{ color: token.colorWarningText }}>
                  App Passthrough
                </Text>
                <Tag color="orange">
                  {tlsConfig?.app_intercept_exclude.length || 0}
                </Tag>
              </Space>
            </Space>
            <Text
              type="secondary"
              style={{
                display: "block",
                marginBottom: 12,
                fontSize: 12,
              }}
            >
              Never intercept traffic from these apps. Supports: exact match,
              prefix* (starts with), *suffix (ends with).
            </Text>
            <Space.Compact style={{ width: "100%", marginBottom: 12 }}>
              <Input
                placeholder="System*, *Agent, curl"
                value={newAppExcludePattern}
                onChange={(e) => setNewAppExcludePattern(e.target.value)}
                onPressEnter={handleAddAppExcludePattern}
                size="small"
              />
              <Button
                type="primary"
                icon={<PlusOutlined />}
                onClick={handleAddAppExcludePattern}
                size="small"
                loading={tlsLoading}
                style={{
                  background: token.colorWarning,
                  borderColor: token.colorWarning,
                }}
              >
                Add
              </Button>
            </Space.Compact>
            <div style={{ maxHeight: 150, overflowY: "auto" }}>
              {tlsConfig?.app_intercept_exclude.length === 0 ? (
                <Text type="secondary">No patterns configured</Text>
              ) : (
                <Space wrap>
                  {tlsConfig?.app_intercept_exclude.map((pattern) => (
                    <Tag
                      key={pattern}
                      color="orange"
                      closable
                      onClose={() => handleRemoveAppExcludePattern(pattern)}
                    >
                      {pattern}
                    </Tag>
                  ))}
                </Space>
              )}
            </div>
          </div>
        </Col>
      </Row>
    </Card>
  );
}

const accessModeOptions: {
  value: AccessMode;
  label: string;
  description: string;
}[] = [
  {
    value: "allow_all",
    label: "Allow All",
    description: "Allow all connections (no restriction)",
  },
  {
    value: "local_only",
    label: "Local Only",
    description: "Only allow localhost connections (127.0.0.1, ::1)",
  },
  {
    value: "whitelist",
    label: "Whitelist",
    description: "Only allow whitelisted IPs/CIDRs",
  },
  {
    value: "interactive",
    label: "Interactive",
    description: "Prompt for unknown IPs (not implemented)",
  },
];

const getModeColor = (mode: AccessMode) => {
  switch (mode) {
    case "allow_all":
      return "red";
    case "local_only":
      return "blue";
    case "whitelist":
      return "green";
    case "interactive":
      return "orange";
    default:
      return "default";
  }
};

const getModeIcon = (mode: AccessMode) => {
  switch (mode) {
    case "allow_all":
      return <GlobalOutlined />;
    case "local_only":
      return <LaptopOutlined />;
    case "whitelist":
      return <SafetyOutlined />;
    case "interactive":
      return <ClockCircleOutlined />;
    default:
      return null;
  }
};

function AccessControlTab() {
  const {
    status,
    loading,
    error,
    fetchStatus,
    addToWhitelist,
    removeFromWhitelist,
    setMode,
    setAllowLan,
    addTemporary,
    removeTemporary,
    clearError,
  } = useWhitelistStore();

  const [newIpOrCidr, setNewIpOrCidr] = useState("");
  const [newTempIp, setNewTempIp] = useState("");

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  useEffect(() => {
    if (error) {
      message.error(error);
      clearError();
    }
  }, [error, clearError]);

  const handleAdd = async () => {
    if (!newIpOrCidr.trim()) {
      message.warning("Please enter an IP address or CIDR");
      return;
    }
    const success = await addToWhitelist(newIpOrCidr.trim());
    if (success) {
      message.success(`Added ${newIpOrCidr} to whitelist`);
      setNewIpOrCidr("");
    }
  };

  const handleRemove = async (ipOrCidr: string) => {
    const success = await removeFromWhitelist(ipOrCidr);
    if (success) {
      message.success(`Removed ${ipOrCidr} from whitelist`);
    }
  };

  const handleAddTemp = async () => {
    if (!newTempIp.trim()) {
      message.warning("Please enter an IP address");
      return;
    }
    const success = await addTemporary(newTempIp.trim());
    if (success) {
      message.success(`Added ${newTempIp} to temporary whitelist`);
      setNewTempIp("");
    }
  };

  const handleRemoveTemp = async (ip: string) => {
    const success = await removeTemporary(ip);
    if (success) {
      message.success(`Removed ${ip} from temporary whitelist`);
    }
  };

  const handleModeChange = async (mode: AccessMode) => {
    const success = await setMode(mode);
    if (success) {
      message.success(`Access mode changed to ${mode}`);
    }
  };

  const handleAllowLanChange = async (allow: boolean) => {
    const success = await setAllowLan(allow);
    if (success) {
      message.success(`LAN access ${allow ? "enabled" : "disabled"}`);
    }
  };

  const whitelistColumns: ColumnsType<{ ip: string }> = [
    {
      title: "IP / CIDR",
      dataIndex: "ip",
      key: "ip",
      render: (ip: string) => <Text code>{ip}</Text>,
    },
    {
      title: "Actions",
      key: "actions",
      width: 100,
      align: "center",
      render: (_, record) => (
        <Popconfirm
          title="Remove from whitelist"
          description={`Remove ${record.ip}?`}
          onConfirm={() => handleRemove(record.ip)}
          okText="Yes"
          cancelText="No"
        >
          <Tooltip title="Remove">
            <Button type="text" size="small" danger icon={<DeleteOutlined />} />
          </Tooltip>
        </Popconfirm>
      ),
    },
  ];

  const tempColumns: ColumnsType<{ ip: string }> = [
    {
      title: "IP Address",
      dataIndex: "ip",
      key: "ip",
      render: (ip: string) => <Text code>{ip}</Text>,
    },
    {
      title: "Actions",
      key: "actions",
      width: 100,
      align: "center",
      render: (_, record) => (
        <Popconfirm
          title="Remove from temporary whitelist"
          description={`Remove ${record.ip}?`}
          onConfirm={() => handleRemoveTemp(record.ip)}
          okText="Yes"
          cancelText="No"
        >
          <Tooltip title="Remove">
            <Button type="text" size="small" danger icon={<DeleteOutlined />} />
          </Tooltip>
        </Popconfirm>
      ),
    },
  ];

  if (!status) {
    return (
      <Alert
        type="warning"
        message="Access Control Not Configured"
        description="The access control feature is not configured on the server. Start the proxy with --access-mode option to enable it."
        showIcon
      />
    );
  }

  return (
    <div>
      <Row justify="space-between" align="middle" style={{ marginBottom: 16 }}>
        <Col>
          <Space>
            <Tag
              color={getModeColor(status.mode)}
              icon={getModeIcon(status.mode)}
            >
              {accessModeOptions.find((o) => o.value === status.mode)?.label ||
                status.mode}
            </Tag>
            {status.allow_lan && <Tag color="cyan">LAN Allowed</Tag>}
          </Space>
        </Col>
        <Col>
          <Button icon={<ReloadOutlined />} onClick={() => fetchStatus()}>
            Refresh
          </Button>
        </Col>
      </Row>

      {error && (
        <Alert
          type="error"
          message={error}
          closable
          onClose={clearError}
          style={{ marginBottom: 16 }}
        />
      )}

      <Row gutter={[16, 16]}>
        <Col xs={24}>
          <Card
            title={
              <Space>
                <SafetyOutlined />
                <span>Access Settings</span>
              </Space>
            }
            size="small"
          >
            <Row gutter={[16, 16]}>
              <Col span={24}>
                <Space direction="vertical" style={{ width: "100%" }}>
                  <Text type="secondary">Access Mode</Text>
                  <Select
                    value={status.mode}
                    onChange={handleModeChange}
                    style={{ width: "100%" }}
                    options={accessModeOptions.map((o) => ({
                      value: o.value,
                      label: (
                        <Space>
                          {getModeIcon(o.value)}
                          <span>{o.label}</span>
                        </Space>
                      ),
                    }))}
                  />
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {
                      accessModeOptions.find((o) => o.value === status.mode)
                        ?.description
                    }
                  </Text>
                </Space>
              </Col>
              <Col span={24}>
                <Divider style={{ margin: "8px 0" }} />
                <Space>
                  <Switch
                    checked={status.allow_lan}
                    onChange={handleAllowLanChange}
                  />
                  <Text>Allow LAN Connections</Text>
                </Space>
                <br />
                <Text type="secondary" style={{ fontSize: 12 }}>
                  When enabled, private network IPs (192.168.x.x, 10.x.x.x,
                  172.16-31.x.x) are allowed
                </Text>
              </Col>
            </Row>
          </Card>
        </Col>

        <Col xs={24}>
          <Card
            title={
              <Space>
                <SafetyOutlined />
                <span>Permanent Whitelist ({status.whitelist.length})</span>
              </Space>
            }
            size="small"
            extra={
              <Space.Compact>
                <Input
                  placeholder="IP or CIDR (e.g., 192.168.1.0/24)"
                  value={newIpOrCidr}
                  onChange={(e) => setNewIpOrCidr(e.target.value)}
                  onPressEnter={handleAdd}
                  style={{ width: 200 }}
                  size="small"
                />
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={handleAdd}
                  size="small"
                >
                  Add
                </Button>
              </Space.Compact>
            }
          >
            <Table
              columns={whitelistColumns}
              dataSource={status.whitelist.map((ip) => ({ ip }))}
              rowKey="ip"
              loading={loading}
              pagination={false}
              size="small"
              locale={{ emptyText: "No entries in whitelist" }}
            />
          </Card>
        </Col>

        <Col xs={24}>
          <Card
            title={
              <Space>
                <ClockCircleOutlined />
                <span>
                  Temporary Whitelist ({status.temporary_whitelist.length})
                </span>
              </Space>
            }
            size="small"
            extra={
              <Space.Compact>
                <Input
                  placeholder="IP Address"
                  value={newTempIp}
                  onChange={(e) => setNewTempIp(e.target.value)}
                  onPressEnter={handleAddTemp}
                  style={{ width: 160 }}
                  size="small"
                />
                <Button
                  type="primary"
                  icon={<PlusOutlined />}
                  onClick={handleAddTemp}
                  size="small"
                >
                  Add
                </Button>
              </Space.Compact>
            }
          >
            <Text
              type="secondary"
              style={{ display: "block", marginBottom: 8 }}
            >
              Temporary entries are stored in memory and will be lost when the
              server restarts.
            </Text>
            <Table
              columns={tempColumns}
              dataSource={status.temporary_whitelist.map((ip) => ({ ip }))}
              rowKey="ip"
              loading={loading}
              pagination={false}
              size="small"
              locale={{ emptyText: "No temporary entries" }}
            />
          </Card>
        </Col>
      </Row>
    </div>
  );
}
