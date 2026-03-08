import { useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Col,
  Divider,
  Input,
  Popconfirm,
  Row,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
  Tooltip,
  message,
} from "antd";
import type { ColumnsType } from "antd/es/table";
import {
  ClockCircleOutlined,
  DeleteOutlined,
  GlobalOutlined,
  LaptopOutlined,
  PlusOutlined,
  ReloadOutlined,
  SafetyOutlined,
} from "@ant-design/icons";
import { useWhitelistStore } from "../../../stores/useWhitelistStore";
import type { AccessMode } from "../../../types";

const { Text } = Typography;

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

export default function AccessControlTab() {
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
            <Tag color={getModeColor(status.mode)} icon={getModeIcon(status.mode)}>
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
                    {accessModeOptions.find((o) => o.value === status.mode)
                      ?.description}
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
              size="small"
              pagination={{ pageSize: 10 }}
            />
          </Card>
        </Col>

        <Col xs={24}>
          <Card
            title={
              <Space>
                <SafetyOutlined />
                <span>
                  Temporary Whitelist ({status.temporary_whitelist.length})
                </span>
              </Space>
            }
            size="small"
            extra={
              <Space.Compact>
                <Input
                  placeholder="Temporary IP"
                  value={newTempIp}
                  onChange={(e) => setNewTempIp(e.target.value)}
                  onPressEnter={handleAddTemp}
                  style={{ width: 200 }}
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
            <Table
              columns={tempColumns}
              dataSource={status.temporary_whitelist.map((ip) => ({ ip }))}
              rowKey="ip"
              loading={loading}
              size="small"
              pagination={{ pageSize: 10 }}
            />
          </Card>
        </Col>
      </Row>
    </div>
  );
}
