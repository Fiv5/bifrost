import { useMemo } from "react";
import {
  Alert,
  AutoComplete,
  Button,
  Card,
  Col,
  Descriptions,
  Divider,
  Image,
  Input,
  Row,
  Space,
  Switch,
  Tag,
  Tooltip,
  Typography,
  theme,
} from "antd";
import {
  ApiOutlined,
  CopyOutlined,
  ExclamationCircleOutlined,
  GlobalOutlined,
  LockOutlined,
  PlusOutlined,
  QrcodeOutlined,
  SafetyCertificateOutlined,
  SwapOutlined,
} from "@ant-design/icons";
import type { SystemOverview } from "../../../types";
import { getProxyQRCodeUrl } from "../../../api/proxy";
import type { ProxyAddressInfo, SystemProxyStatus } from "../../../api/proxy";
import type { TlsConfig } from "../../../api/config";

const { Text } = Typography;

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
  appSuggestions: string[];
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
  appSuggestions,
}: TlsInterceptionPatternsCardProps) {
  const { token } = theme.useToken();

  const appOptions = useMemo(() => {
    return appSuggestions.map((app) => ({
      value: app,
      label: app,
    }));
  }, [appSuggestions]);

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
            <div>
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
                <SafetyCertificateOutlined style={{ color: token.colorWarning }} />
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
            <div>
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
              <AutoComplete
                placeholder="Chrome*, *Browser, Postman"
                value={newAppIncludePattern}
                options={appOptions}
                onChange={(value) => setNewAppIncludePattern(value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleAddAppIncludePattern();
                  }
                }}
                size="small"
                style={{ flex: 1 }}
                filterOption={(inputValue, option) =>
                  option?.value
                    .toLowerCase()
                    .includes(inputValue.toLowerCase()) ?? false
                }
                allowClear
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
            <div>
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
                <SafetyCertificateOutlined style={{ color: token.colorWarning }} />
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
              <AutoComplete
                placeholder="System*, *Agent, curl"
                value={newAppExcludePattern}
                options={appOptions}
                onChange={(value) => setNewAppExcludePattern(value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    handleAddAppExcludePattern();
                  }
                }}
                size="small"
                style={{ flex: 1 }}
                filterOption={(inputValue, option) =>
                  option?.value
                    .toLowerCase()
                    .includes(inputValue.toLowerCase()) ?? false
                }
                allowClear
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
            <div>
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

const formatUptime = (secs: number): string => {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const mins = Math.floor((secs % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${mins}m`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m ${secs % 60}s`;
};

export interface ProxyTabProps {
  systemProxy: SystemProxyStatus | null;
  systemProxyLoading: boolean;
  onToggleSystemProxy: (enabled: boolean) => void;
  copyProxyConfig: () => void;
  overview: SystemOverview | null;
  proxyAddressInfo: ProxyAddressInfo | null;
  selectedProxyIp: string;
  setSelectedProxyIp: (value: string) => void;
  tlsConfig: TlsConfig | null;
  tlsLoading: boolean;
  onToggleTlsInterception: (enabled: boolean) => void;
  onToggleUnsafeSsl: (enabled: boolean) => void;
  onToggleDisconnectOnConfigChange: (enabled: boolean) => void;
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
  appSuggestions: string[];
}

export default function ProxyTab({
  systemProxy,
  systemProxyLoading,
  onToggleSystemProxy,
  copyProxyConfig,
  overview,
  proxyAddressInfo,
  selectedProxyIp,
  setSelectedProxyIp,
  tlsConfig,
  tlsLoading,
  onToggleTlsInterception,
  onToggleUnsafeSsl,
  onToggleDisconnectOnConfigChange,
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
  appSuggestions,
}: ProxyTabProps) {
  return (
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
                      onChange={onToggleSystemProxy}
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
              <Button icon={<CopyOutlined />} size="small" onClick={copyProxyConfig}>
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
                {proxyAddressInfo && proxyAddressInfo.addresses.length > 0 && (
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
                            selectedProxyIp === addr.ip ? "blue" : "default"
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
                    onChange={onToggleTlsInterception}
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
                      <ExclamationCircleOutlined style={{ color: "#faad14" }} />
                    </Tooltip>
                  </Space>
                </Col>
                <Col>
                  <Switch
                    checked={tlsConfig?.unsafe_ssl}
                    loading={tlsLoading}
                    onChange={onToggleUnsafeSsl}
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
                    onChange={onToggleDisconnectOnConfigChange}
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
            appSuggestions={appSuggestions}
          />
        </Col>

        <Col xs={24}>
          <Card title="System Information" size="small">
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Version">
                <Text code>v{overview?.system.version}</Text>
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
      </Row>
    </div>
  );
}
