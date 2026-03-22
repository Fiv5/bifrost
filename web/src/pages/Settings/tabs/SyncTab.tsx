import {
  Alert,
  Button,
  Card,
  Descriptions,
  Input,
  Space,
  Switch,
  Tag,
  Typography,
} from "antd";
import {
  CloudOutlined,
  LinkOutlined,
  LoginOutlined,
  LogoutOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import type { SyncStatus } from "../../../api/sync";

const { Text } = Typography;

interface SyncTabProps {
  syncStatus: SyncStatus | null;
  syncLoading: boolean;
  remoteBaseUrlDraft: string;
  setRemoteBaseUrlDraft: (value: string) => void;
  onToggleEnabled: (enabled: boolean) => void;
  onToggleAutoSync: (enabled: boolean) => void;
  onSaveRemoteBaseUrl: () => void;
  onSignIn: () => void;
  onSignOut: () => void;
  onRunSync: () => void;
}

function renderStatusTag(syncStatus: SyncStatus | null) {
  if (!syncStatus) {
    return <Tag>Unknown</Tag>;
  }
  switch (syncStatus.reason) {
    case "ready":
      return <Tag color="green">Ready</Tag>;
    case "syncing":
      return <Tag color="processing">Syncing</Tag>;
    case "unreachable":
      return <Tag color="orange">Offline</Tag>;
    case "unauthorized":
      return <Tag color="gold">Sign in required</Tag>;
    case "error":
      return <Tag color="red">Error</Tag>;
    default:
      return <Tag>{syncStatus.reason}</Tag>;
  }
}

function renderLastAction(syncStatus: SyncStatus | null) {
  switch (syncStatus?.last_sync_action) {
    case "local_pushed":
      return "Local changes pushed to remote";
    case "remote_pulled":
      return "Newer remote changes pulled into local";
    case "bidirectional":
      return "Local and remote changes exchanged";
    case "no_change":
      return "No changes detected";
    default:
      return "No sync result yet";
  }
}

export default function SyncTab({
  syncStatus,
  syncLoading,
  remoteBaseUrlDraft,
  setRemoteBaseUrlDraft,
  onToggleEnabled,
  onToggleAutoSync,
  onSaveRemoteBaseUrl,
  onSignIn,
  onSignOut,
  onRunSync,
}: SyncTabProps) {
  return (
    <Space direction="vertical" size={16} style={{ width: "100%" }}>
      <Alert
        showIcon
        type={syncStatus?.reachable ? "info" : "warning"}
        message="Remote sync is a pluggable capability"
        description="Rules continue to work locally at all times. Sync only activates when the configured Bifrost service is reachable and a valid login session exists."
      />

      <Card
        title={
          <Space>
            <CloudOutlined />
            <span>Remote Sync</span>
          </Space>
        }
        size="small"
      >
        <Space direction="vertical" size={16} style={{ width: "100%" }}>
          <Descriptions
            size="small"
            column={1}
            items={[
              {
                key: "status",
                label: "Status",
                children: renderStatusTag(syncStatus),
              },
              {
                key: "reachability",
                label: "Connectivity",
                children: syncStatus?.reachable ? "Reachable" : "Local only",
              },
              {
                key: "session",
                label: "Session",
                children: syncStatus?.has_session
                  ? syncStatus.user?.user_id || "Signed in on this device"
                  : "Not signed in",
              },
              {
                key: "last-sync",
                label: "Last Sync",
                children: syncStatus?.last_sync_at || "Never",
              },
              {
                key: "last-sync-action",
                label: "Last Result",
                children: (
                  <span data-testid="settings-sync-last-action">
                    {renderLastAction(syncStatus)}
                  </span>
                ),
              },
            ]}
          />

          {syncStatus?.last_error ? (
            <Alert
              showIcon
              type="error"
              message="Last sync error"
              description={syncStatus.last_error}
            />
          ) : null}

          <Space align="center">
            <Text strong>Enable sync</Text>
            <Switch
              checked={syncStatus?.enabled ?? false}
              loading={syncLoading}
              onChange={onToggleEnabled}
              data-testid="settings-sync-enable-switch"
            />
            <Text type="secondary">Optional and environment-aware</Text>
          </Space>

          <Space align="center">
            <Text strong>Auto sync</Text>
            <Switch
              checked={syncStatus?.auto_sync ?? false}
              loading={syncLoading}
              onChange={onToggleAutoSync}
              disabled={!(syncStatus?.enabled ?? false)}
              data-testid="settings-sync-auto-switch"
            />
            <Text type="secondary">Automatically resync after reconnection</Text>
          </Space>

          <Space.Compact style={{ width: "100%" }}>
            <Input
              value={remoteBaseUrlDraft}
              onChange={(event) => setRemoteBaseUrlDraft(event.target.value)}
              placeholder="https://bifrost.bytedance.net"
              prefix={<LinkOutlined />}
              data-testid="settings-sync-remote-url-input"
            />
            <Button
              type="primary"
              onClick={onSaveRemoteBaseUrl}
              loading={syncLoading}
              data-testid="settings-sync-remote-url-save"
            >
              Save
            </Button>
          </Space.Compact>

          <Space wrap>
            <Button
              type="primary"
              icon={<LoginOutlined />}
              onClick={onSignIn}
              disabled={!(syncStatus?.enabled ?? false)}
              loading={syncLoading}
              data-testid="settings-sync-sign-in"
            >
              Sign In
            </Button>
            <Button
              icon={<LogoutOutlined />}
              onClick={onSignOut}
              disabled={!syncStatus?.has_session}
              loading={syncLoading}
              data-testid="settings-sync-sign-out"
            >
              Sign Out
            </Button>
            <Button
              icon={<ReloadOutlined />}
              onClick={onRunSync}
              disabled={!syncStatus?.authorized}
              loading={syncLoading || syncStatus?.syncing}
              data-testid="settings-sync-run-now"
            >
              Sync Now
            </Button>
          </Space>
        </Space>
      </Card>
    </Space>
  );
}
