import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  Badge,
  Button,
  List,
  Modal,
  Popconfirm,
  Space,
  Typography,
  message,
} from "antd";
import {
  CheckOutlined,
  StopOutlined,
  ClearOutlined,
  SafetyCertificateOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import { usePendingIpTlsStore } from "../../stores/usePendingIpTlsStore";

const { Text } = Typography;

function formatTimeAgo(timestamp: number) {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

export default function PendingIpTlsModal() {
  const pendingList = usePendingIpTlsStore((s) => s.pendingList);
  const pendingCount = usePendingIpTlsStore((s) => s.pendingCount);
  const approvePending = usePendingIpTlsStore((s) => s.approvePending);
  const skipPending = usePendingIpTlsStore((s) => s.skipPending);
  const clearPending = usePendingIpTlsStore((s) => s.clearPending);
  const navigate = useNavigate();

  const handleApprove = useCallback(
    async (ip: string) => {
      const ok = await approvePending(ip);
      if (ok) {
        message.success(`Enabled TLS interception for ${ip}`);
      } else {
        message.error(`Failed to enable TLS interception for ${ip}`);
      }
    },
    [approvePending],
  );

  const handleSkip = useCallback(
    async (ip: string) => {
      const ok = await skipPending(ip);
      if (ok) {
        message.success(`Skipped TLS interception for ${ip}`);
      } else {
        message.error(`Failed to skip TLS interception for ${ip}`);
      }
    },
    [skipPending],
  );

  const handleClearAll = useCallback(async () => {
    const ok = await clearPending();
    if (ok) {
      message.success("Cleared all pending IP TLS decisions");
    } else {
      message.error("Failed to clear pending IP TLS decisions");
    }
  }, [clearPending]);

  const handleGoToSettings = useCallback(() => {
    navigate("/settings?tab=tls");
  }, [navigate]);

  return (
    <Modal
      open={pendingCount > 0}
      title={
        <Space>
          <SafetyCertificateOutlined style={{ color: "#1890ff" }} />
          <span>IP TLS Interception Requests</span>
          <Badge
            count={pendingCount}
            style={{ backgroundColor: "#1890ff" }}
          />
        </Space>
      }
      footer={
        <Space>
          {pendingList.length > 0 && (
            <Popconfirm
              title="Clear all pending IP TLS decisions?"
              description="Pending IPs will need to reconnect to trigger a new prompt."
              onConfirm={handleClearAll}
              okText="Yes"
              cancelText="No"
            >
              <Button icon={<ClearOutlined />}>
                Clear All
              </Button>
            </Popconfirm>
          )}
          <Button
            icon={<SettingOutlined />}
            onClick={handleGoToSettings}
          >
            TLS Settings
          </Button>
        </Space>
      }
      closable={false}
      maskClosable={false}
      keyboard={false}
      centered
      width={560}
      zIndex={999}
    >
      <div>
        <Text type="secondary" style={{ display: "block", marginBottom: 12 }}>
          New IP addresses are connecting through this proxy. Choose whether to
          enable TLS interception (decrypt HTTPS traffic) or skip for each IP.
        </Text>
        <List
          size="small"
          dataSource={pendingList}
          locale={{ emptyText: "No pending requests" }}
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
                  Enable TLS
                </Button>,
                <Button
                  key="skip"
                  size="small"
                  icon={<StopOutlined />}
                  onClick={() => handleSkip(item.ip)}
                >
                  Skip
                </Button>,
              ]}
            >
              <List.Item.Meta
                title={<Text code>{item.ip}</Text>}
                description={
                  <Text type="secondary">
                    First seen: {formatTimeAgo(item.first_seen)} · Attempts:{" "}
                    {item.attempt_count}
                  </Text>
                }
              />
            </List.Item>
          )}
        />
      </div>
    </Modal>
  );
}
