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
  Tag,
  Typography,
  message,
} from "antd";
import {
  KeyOutlined,
  LockOutlined,
  SafetyOutlined,
  StopOutlined,
  UserOutlined,
} from "@ant-design/icons";
import {
  changeAdminPassword,
  fetchAdminAuthStatus,
  revokeAllSessions,
  setRemoteAccess,
  type AdminAuthStatus,
} from "../../../services/adminAuth";

const { Text } = Typography;

export default function RemoteAccessTab() {
  const [status, setStatus] = useState<AdminAuthStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

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

  useEffect(() => {
    void refresh();
  }, [refresh]);

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
            </Descriptions>

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
            size="small"
          >
            <Space direction="vertical" style={{ width: "100%" }}>
              <Text type="secondary">
                Revoke all existing admin login sessions. This will force all
                currently logged-in users to re-authenticate.
              </Text>
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
