import { useEffect } from "react";
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
} from "antd";
import { CopyOutlined } from "@ant-design/icons";
import { useMetricsStore } from "../../stores/useMetricsStore";

const { Text, Paragraph } = Typography;

export default function Settings() {
  const { overview, loading, error, fetchOverview } = useMetricsStore();

  useEffect(() => {
    fetchOverview();
  }, [fetchOverview]);

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

  return (
    <div>
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
