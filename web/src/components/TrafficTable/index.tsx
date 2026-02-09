import { Table, Tag, Typography, Tooltip } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import dayjs from 'dayjs';
import type { TrafficSummary } from '../../types';

const { Text } = Typography;

interface TrafficTableProps {
  data: TrafficSummary[];
  loading?: boolean;
  onSelect?: (record: TrafficSummary) => void;
  selectedId?: string;
}

export default function TrafficTable({ data, loading, onSelect, selectedId }: TrafficTableProps) {
  const getStatusColor = (status: number) => {
    if (status >= 500) return 'error';
    if (status >= 400) return 'warning';
    if (status >= 300) return 'processing';
    if (status >= 200) return 'success';
    return 'default';
  };

  const getMethodColor = (method: string) => {
    const colors: Record<string, string> = {
      GET: 'green',
      POST: 'blue',
      PUT: 'orange',
      DELETE: 'red',
      PATCH: 'purple',
      OPTIONS: 'default',
      HEAD: 'cyan',
    };
    return colors[method.toUpperCase()] || 'default';
  };

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '-';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  };

  const columns: ColumnsType<TrafficSummary> = [
    {
      title: 'Time',
      dataIndex: 'timestamp',
      key: 'timestamp',
      width: 90,
      render: (ts: number) => (
        <Text type="secondary" style={{ fontSize: 12 }}>
          {dayjs(ts).format('HH:mm:ss')}
        </Text>
      ),
    },
    {
      title: 'Method',
      dataIndex: 'method',
      key: 'method',
      width: 80,
      render: (method: string) => (
        <Tag color={getMethodColor(method)} style={{ margin: 0 }}>
          {method}
        </Tag>
      ),
    },
    {
      title: 'Status',
      dataIndex: 'status',
      key: 'status',
      width: 70,
      align: 'center',
      render: (status: number) =>
        status > 0 ? (
          <Tag color={getStatusColor(status)} style={{ margin: 0 }}>
            {status}
          </Tag>
        ) : (
          <Text type="secondary">-</Text>
        ),
    },
    {
      title: 'Host',
      dataIndex: 'host',
      key: 'host',
      width: 180,
      ellipsis: true,
      render: (host: string) => (
        <Tooltip title={host}>
          <Text style={{ fontSize: 12 }}>{host}</Text>
        </Tooltip>
      ),
    },
    {
      title: 'Path',
      dataIndex: 'path',
      key: 'path',
      ellipsis: true,
      render: (path: string) => (
        <Tooltip title={path}>
          <Text style={{ fontSize: 12 }}>{path}</Text>
        </Tooltip>
      ),
    },
    {
      title: 'Type',
      dataIndex: 'content_type',
      key: 'content_type',
      width: 120,
      ellipsis: true,
      render: (ct: string | null) => (
        <Text type="secondary" style={{ fontSize: 11 }}>
          {ct?.split(';')[0] || '-'}
        </Text>
      ),
    },
    {
      title: 'Size',
      dataIndex: 'response_size',
      key: 'response_size',
      width: 80,
      align: 'right',
      render: (size: number) => (
        <Text type="secondary" style={{ fontSize: 12 }}>
          {formatSize(size)}
        </Text>
      ),
    },
    {
      title: 'Time',
      dataIndex: 'duration_ms',
      key: 'duration_ms',
      width: 70,
      align: 'right',
      render: (ms: number) => (
        <Text type={ms > 1000 ? 'warning' : 'secondary'} style={{ fontSize: 12 }}>
          {ms > 0 ? `${ms}ms` : '-'}
        </Text>
      ),
    },
  ];

  return (
    <Table
      columns={columns}
      dataSource={data}
      rowKey="id"
      loading={loading}
      pagination={false}
      size="small"
      scroll={{ y: 'calc(100vh - 350px)' }}
      onRow={(record) => ({
        onClick: () => onSelect?.(record),
        style: {
          cursor: 'pointer',
          background: record.id === selectedId ? '#e6f7ff' : undefined,
        },
      })}
    />
  );
}
