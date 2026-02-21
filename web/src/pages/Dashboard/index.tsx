import { useEffect } from 'react';
import { Row, Col, Card, Statistic, Spin, Alert, Typography } from 'antd';
import {
  CloudServerOutlined,
  ClockCircleOutlined,
  ApiOutlined,
  SwapOutlined,
  FileTextOutlined,
  CloudUploadOutlined,
} from '@ant-design/icons';
import { useMetricsStore } from '../../stores/useMetricsStore';
import MetricsChart from '../../components/MetricsChart';

const { Text } = Typography;

export default function Dashboard() {
  const {
    overview,
    history,
    loading,
    error,
    fetchOverview,
    fetchHistory,
    enablePush,
    disablePush,
    usePush,
  } = useMetricsStore();

  useEffect(() => {
    fetchOverview();
    fetchHistory(60);

    if (usePush) {
      enablePush({
        needOverview: true,
        needMetrics: true,
        needHistory: true,
        historyLimit: 60,
      });
      return () => {
        disablePush();
      };
    } else {
      const interval = setInterval(() => {
        fetchOverview();
        fetchHistory(60);
      }, 1000);
      return () => clearInterval(interval);
    }
  }, [fetchOverview, fetchHistory, enablePush, disablePush, usePush]);

  if (loading && !overview) {
    return <Spin size="large" style={{ display: 'block', margin: '100px auto' }} />;
  }

  if (error) {
    return <Alert type="error" message="Failed to load metrics" description={error} />;
  }

  const formatUptime = (secs: number) => {
    const days = Math.floor(secs / 86400);
    const hours = Math.floor((secs % 86400) / 3600);
    const mins = Math.floor((secs % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h ${mins}m`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  return (
    <div>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Status"
              value="Running"
              prefix={<CloudServerOutlined style={{ color: '#52c41a' }} />}
              valueStyle={{ color: '#52c41a' }}
            />
            {overview && (
              <Text type="secondary">
                v{overview.system.version} · Port {overview.server.port}
              </Text>
            )}
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Uptime"
              value={overview ? formatUptime(overview.system.uptime_secs) : '-'}
              prefix={<ClockCircleOutlined />}
            />
            <Text type="secondary">
              PID: {overview?.system.pid}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Total Requests"
              value={overview?.metrics.total_requests || 0}
              prefix={<ApiOutlined />}
            />
            <Text type="secondary">
              QPS: {overview?.metrics.qps.toFixed(2) || 0}
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Total Traffic"
              value={formatBytes((overview?.metrics.bytes_sent || 0) + (overview?.metrics.bytes_received || 0))}
              prefix={<CloudUploadOutlined />}
            />
            <Text type="secondary">
              ↑{formatBytes(overview?.metrics.bytes_sent_rate || 0)}/s ↓{formatBytes(overview?.metrics.bytes_received_rate || 0)}/s
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Active Connections"
              value={overview?.metrics.active_connections || 0}
              prefix={<SwapOutlined />}
            />
            <Text type="secondary">
              {overview?.traffic.recorded || 0} recorded
            </Text>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={4}>
          <Card>
            <Statistic
              title="Rules"
              value={overview?.rules.enabled || 0}
              suffix={`/ ${overview?.rules.total || 0}`}
              prefix={<FileTextOutlined />}
            />
            <Text type="secondary">enabled / total</Text>
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={12}>
          <Card title="CPU Usage" size="small">
            {history.length > 0 ? (
              <MetricsChart data={history} type="cpu" />
            ) : (
              <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Text type="secondary">No data</Text>
              </div>
            )}
          </Card>
        </Col>
        <Col xs={24} lg={12}>
          <Card title="Memory Usage" size="small">
            {history.length > 0 ? (
              <MetricsChart data={history} type="memory" />
            ) : (
              <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Text type="secondary">No data</Text>
              </div>
            )}
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={12}>
          <Card title="QPS" size="small">
            {history.length > 0 ? (
              <MetricsChart data={history} type="qps" />
            ) : (
              <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Text type="secondary">No data</Text>
              </div>
            )}
          </Card>
        </Col>
        <Col xs={24} lg={12}>
          <Card title="Bandwidth" size="small">
            {history.length > 0 ? (
              <MetricsChart data={history} type="bandwidth" />
            ) : (
              <div style={{ height: 200, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <Text type="secondary">No data</Text>
              </div>
            )}
          </Card>
        </Col>
      </Row>


    </div>
  );
}
