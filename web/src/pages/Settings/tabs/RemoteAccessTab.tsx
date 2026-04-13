import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Col,
  Descriptions,
  Divider,
  Input,
  Popconfirm,
  Row,
  Space,
  Switch,
  Table,
  Tag,
  Tooltip,
  Typography,
  message,
} from "antd";
import type { ColumnsType } from "antd/es/table";
import {
  CheckCircleOutlined,
  CloseCircleOutlined,
  KeyOutlined,
  LockOutlined,
  ReloadOutlined,
  SafetyOutlined,
  StopOutlined,
  UserOutlined,
  WarningOutlined,
} from "@ant-design/icons";
import {
  changeAdminPassword,
  fetchAdminAuthStatus,
  fetchLoginAudit,
  revokeAllSessions,
  setRemoteAccess,
  type AdminAuthStatus,
  type LoginAuditEntry,
} from "../../../services/adminAuth";

const { Text } = Typography;

const auditColumns: ColumnsType<LoginAuditEntry> = [
  {
    title: "Status",
    dataIndex: "success",
    key: "success",
    width: 80,
    render: (success: boolean) =>
      success ? (
        <Tag icon={<CheckCircleOutlined />} color="success">OK</Tag>
      ) : (
        <Tag icon={<CloseCircleOutlined />} color="error">Failed</Tag>
      ),
  },
  {
    title: "Time",
    dataIndex: "ts",
    key: "ts",
    width: 180,
    render: (ts: number) => {
      const d = new Date(ts * 1000);
      return (
        <Tooltip title={d.toISOString()}>
          {d.toLocaleString()}
        </Tooltip>
      );
    },
  },
  {
    title: "IP",
    dataIndex: "ip",
    key: "ip",
    width: 160,
  },
  {
    title: "Username",
    dataIndex: "username",
    key: "username",
    width: 120,
  },
  {
    title: "User Agent",
    dataIndex: "ua",
    key: "ua",
    ellipsis: { showTitle: false },
    render: (ua: string) => (
      <Tooltip title={ua}>
        <Text type="secondary" style={{ fontSize: 12 }}>
          {ua || "-"}
        </Text>
      </Tooltip>
    ),
  },
];

export default function RemoteAccessTab() {
  const [status, setStatus] = useState<AdminAuthStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [auditItems, setAuditItems] = useState<LoginAuditEntry[]>([]);
  const [auditTotal, setAuditTotal] = useState(0);
  const [auditPage, setAuditPage] = useState(1);
  const [auditLoading, setAuditLoading] = useState(false);
  const auditPageSize = 10;

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const s = await fetchAdminAuthStatus();
      setStatus(s);
      setUsername(s.username || "admin");
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshAudit = useCallback(async (page = 1) => {
    setAuditLoading(true);
    try {
      const res = await fetchLoginAudit(auditPageSize, (page - 1) * auditPageSize);
      setAuditItems(res.items);
      setAuditTotal(res.total);
      setAuditPage(page);
    } catch {
      // ignore
    } finally {
      setAuditLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    void refreshAudit(1);
  }, [refresh, refreshAudit]);

  const handleToggleRemoteAccess = async (enabled: boolean) => {
    setLoading(true);
    try {
      const s = await setRemoteAccess(enabled);
      setStatus(s);
      message.success(
        enabled ? "Remote access enabled" : "Remote access disabled",
      );
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : "Failed to toggle remote access",
      );
    } finally {
      setLoading(false);
    }
  };

  const handleChangePassword = async () => {
    if (!password.trim()) {
      message.warning("Please enter a password");
      return;
    }
    const minLen = status?.min_password_length ?? 6;
    if (password.length < minLen) {
      message.warning(`Password must be at least ${minLen} characters`);
      return;
    }
    const hasLetter = /[a-zA-Z]/.test(password);
    const hasDigit = /\d/.test(password);
    if (!hasLetter || !hasDigit) {
      message.warning("Password must contain both letters and digits");
      return;
    }
    if (password !== confirmPassword) {
      message.warning("Passwords do not match");
      return;
    }
    setLoading(true);
    try {
      await changeAdminPassword(password, username);
      message.success("Password updated");
      setPassword("");
      setConfirmPassword("");
      await refresh();
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : "Failed to change password",
      );
    } finally {
      setLoading(false);
    }
  };

  const handleRevokeAll = async () => {
    setLoading(true);
    try {
      await revokeAllSessions();
      message.success("All sessions revoked");
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : "Failed to revoke sessions",
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <div data-testid="settings-remote-access-tab">
      <Row gutter={[16, 16]}>
        <Col xs={24}>
          <Card
            title={
              <Space>
                <SafetyOutlined />
                <span>Remote Access Status</span>
              </Space>
            }
            size="small"
          >
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Remote Access">
                {status?.remote_access_enabled ? (
                  <Tag color="green">Enabled</Tag>
                ) : (
                  <Tag color="default">Disabled</Tag>
                )}
              </Descriptions.Item>
              <Descriptions.Item label="Admin Username">
                <Text code>{status?.username || "admin"}</Text>
              </Descriptions.Item>
              <Descriptions.Item label="Password">
                {status?.has_password ? (
                  <Tag color="green">Set</Tag>
                ) : (
                  <Tag color="orange">Not Set</Tag>
                )}
              </Descriptions.Item>
              <Descriptions.Item label="Failed Attempts">
                {(status?.failed_attempts ?? 0) > 0 ? (
                  <Tag color="red">{status?.failed_attempts}/{status?.max_attempts ?? 5}</Tag>
                ) : (
                  <Tag color="green">0/{status?.max_attempts ?? 5}</Tag>
                )}
              </Descriptions.Item>
            </Descriptions>

            {status?.locked_out ? (
              <Alert
                type="error"
                icon={<WarningOutlined />}
                message="Brute-Force Lockout Active"
                description="Remote access was automatically disabled due to too many failed login attempts. The password has been cleared. Set a new password below to re-enable."
                showIcon
                style={{ margin: "12px 0" }}
              />
            ) : null}

            <Divider style={{ margin: "12px 0" }} />

            <Space>
              <Switch
                checked={status?.remote_access_enabled ?? false}
                onChange={handleToggleRemoteAccess}
                loading={loading}
                data-testid="settings-remote-access-toggle"
              />
              <Text>Enable Remote Admin Access</Text>
            </Space>
            <br />
            <Text type="secondary" style={{ fontSize: 12 }}>
              When enabled, the admin interface can be accessed from remote IPs
              (e.g. LAN devices). A login with username and password is required.
            </Text>
          </Card>
        </Col>

        <Col xs={24}>
          <Card
            title={
              <Space>
                <KeyOutlined />
                <span>Admin Credentials</span>
              </Space>
            }
            size="small"
          >
            <Space direction="vertical" style={{ width: "100%" }}>
              <div>
                <Text type="secondary" style={{ marginBottom: 4, display: "block" }}>
                  Username
                </Text>
                <Input
                  prefix={<UserOutlined />}
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="admin"
                  data-testid="settings-remote-username"
                />
              </div>
              <div>
                <Text type="secondary" style={{ marginBottom: 4, display: "block" }}>
                  New Password
                </Text>
                <Input.Password
                  prefix={<LockOutlined />}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Enter new password"
                  data-testid="settings-remote-password"
                />
                <Text type="secondary" style={{ fontSize: 11 }}>
                  Min {status?.min_password_length ?? 6} chars, must include letters and digits
                </Text>
              </div>
              <div>
                <Text type="secondary" style={{ marginBottom: 4, display: "block" }}>
                  Confirm Password
                </Text>
                <Input.Password
                  prefix={<LockOutlined />}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Confirm new password"
                  onPressEnter={handleChangePassword}
                  data-testid="settings-remote-confirm-password"
                />
              </div>
              <Button
                type="primary"
                onClick={handleChangePassword}
                loading={loading}
                data-testid="settings-remote-save-password"
              >
                Save Credentials
              </Button>
            </Space>

            {!status?.has_password && (
              <Alert
                type="info"
                message="Set a password to enable remote admin access"
                showIcon
                style={{ marginTop: 12 }}
              />
            )}
          </Card>
        </Col>

        <Col xs={24}>
          <Card
            title={
              <Space>
                <StopOutlined />
                <span>Session Management</span>
              </Space>
            }
            extra={
              <Button
                size="small"
                icon={<ReloadOutlined />}
                onClick={() => void refreshAudit(auditPage)}
                loading={auditLoading}
              >
                Refresh
              </Button>
            }
            size="small"
          >
            <Space direction="vertical" style={{ width: "100%" }}>
              <Text type="secondary">
                Recent admin login sessions. You can revoke all sessions to
                force re-authentication.
              </Text>
              <Table<LoginAuditEntry>
                columns={auditColumns}
                dataSource={auditItems}
                rowKey="id"
                size="small"
                loading={auditLoading}
                pagination={{
                  current: auditPage,
                  pageSize: auditPageSize,
                  total: auditTotal,
                  showTotal: (total) => `${total} records`,
                  showSizeChanger: false,
                  onChange: (page) => void refreshAudit(page),
                }}
                data-testid="settings-remote-audit-table"
              />
              <Divider style={{ margin: "4px 0" }} />
              <Popconfirm
                title="Revoke all sessions?"
                description="All logged-in admin sessions will be invalidated."
                onConfirm={handleRevokeAll}
                okText="Revoke"
                cancelText="Cancel"
              >
                <Button
                  danger
                  loading={loading}
                  data-testid="settings-remote-revoke-all"
                >
                  Revoke All Sessions
                </Button>
              </Popconfirm>
            </Space>
          </Card>
        </Col>
      </Row>
    </div>
  );
}
