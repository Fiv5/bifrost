import { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { Table, Typography, Tag, theme, Button, Space, Tooltip, Empty } from 'antd';
import type { TableProps } from 'antd';
import {
  ArrowUpOutlined,
  ArrowDownOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import dayjs from 'dayjs';
import type { WebSocketFrame, FrameDirection, FrameType, SessionTargetSearchState } from '../../../../types';
import { useMarkSearch } from '../../hooks/useMarkSearch';

const { Text } = Typography;

interface MessagesProps {
  recordId: string;
  isWebSocket: boolean;
  frameCount: number;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

const formatSize = (bytes: number) => {
  if (bytes === 0) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
};

const getFrameTypeColor = (type: FrameType): string => {
  switch (type) {
    case 'text':
      return 'blue';
    case 'binary':
      return 'purple';
    case 'ping':
      return 'cyan';
    case 'pong':
      return 'geekblue';
    case 'close':
      return 'red';
    case 'sse':
      return 'green';
    default:
      return 'default';
  }
};

const DirectionIcon = ({ direction }: { direction: FrameDirection }) => {
  return direction === 'send' ? (
    <ArrowUpOutlined style={{ color: '#52c41a' }} />
  ) : (
    <ArrowDownOutlined style={{ color: '#1890ff' }} />
  );
};

export const Messages = ({
  recordId,
  isWebSocket,
  frameCount,
  searchValue,
  onSearch,
}: MessagesProps) => {
  const { token } = theme.useToken();
  const [frames, setFrames] = useState<WebSocketFrame[]>([]);
  const [loading, setLoading] = useState(false);
  const [lastFrameId, setLastFrameId] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const tableRef = useRef<HTMLDivElement>(null);

  const fetchFrames = useCallback(async (after?: number) => {
    setLoading(true);
    try {
      const params = new URLSearchParams();
      if (after !== undefined) {
        params.set('after', String(after));
      }
      params.set('limit', '100');

      const response = await fetch(
        `/api/traffic/${recordId}/frames?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error('Failed to fetch frames');
      }
      const data = await response.json();

      if (after !== undefined) {
        setFrames((prev) => [...prev, ...data.frames]);
      } else {
        setFrames(data.frames);
      }
      setLastFrameId(data.last_frame_id);
      setHasMore(data.has_more);
    } catch (error) {
      console.error('Failed to fetch frames:', error);
    } finally {
      setLoading(false);
    }
  }, [recordId]);

  useEffect(() => {
    if (frameCount > 0) {
      fetchFrames();
    }
  }, [recordId, frameCount, fetchFrames]);

  useMarkSearch(searchValue, () => tableRef.current, onSearch);

  const filteredFrames = useMemo(() => {
    if (!searchValue.value) return frames;
    const searchLower = searchValue.value.toLowerCase();
    return frames.filter(
      (frame) =>
        frame.payload_preview?.toLowerCase().includes(searchLower) ||
        frame.frame_type.toLowerCase().includes(searchLower)
    );
  }, [frames, searchValue.value]);

  const columns: TableProps<WebSocketFrame>['columns'] = [
    {
      title: '#',
      dataIndex: 'frame_id',
      key: 'frame_id',
      width: 60,
      render: (id: number) => <Text type="secondary">{id}</Text>,
    },
    {
      title: '',
      dataIndex: 'direction',
      key: 'direction',
      width: 40,
      render: (direction: FrameDirection) => (
        <DirectionIcon direction={direction} />
      ),
    },
    {
      title: 'Type',
      dataIndex: 'frame_type',
      key: 'frame_type',
      width: 80,
      render: (type: FrameType) => (
        <Tag color={getFrameTypeColor(type)}>{type.toUpperCase()}</Tag>
      ),
    },
    {
      title: 'Size',
      dataIndex: 'payload_size',
      key: 'payload_size',
      width: 80,
      render: (size: number) => formatSize(size),
    },
    {
      title: 'Time',
      dataIndex: 'timestamp',
      key: 'timestamp',
      width: 100,
      render: (ts: number) => dayjs(ts).format('HH:mm:ss.SSS'),
    },
    {
      title: 'Preview',
      dataIndex: 'payload_preview',
      key: 'payload_preview',
      ellipsis: true,
      render: (preview: string | undefined) =>
        preview ? (
          <Text
            style={{ fontFamily: 'monospace', fontSize: 12 }}
            ellipsis={{ tooltip: preview }}
          >
            {preview}
          </Text>
        ) : (
          <Text type="secondary">-</Text>
        ),
    },
  ];

  if (frameCount === 0) {
    return (
      <div
        style={{
          padding: 24,
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          height: 200,
        }}
      >
        <Empty
          description={`No ${isWebSocket ? 'WebSocket' : 'SSE'} messages yet`}
        />
      </div>
    );
  }

  return (
    <div ref={tableRef}>
      <div
        style={{
          marginBottom: 4,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <Text type="secondary">
          {filteredFrames.length} of {frames.length} frames
          {hasMore && ' (more available)'}
        </Text>
        <Space>
          {hasMore && (
            <Button
              size="small"
              onClick={() => fetchFrames(lastFrameId)}
              loading={loading}
            >
              Load More
            </Button>
          )}
          <Tooltip title="Refresh">
            <Button
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => fetchFrames()}
              loading={loading}
            />
          </Tooltip>
        </Space>
      </div>

      <div className="compact-messages-table">
        <Table<WebSocketFrame>
          dataSource={filteredFrames}
          columns={columns}
          rowKey="frame_id"
          pagination={false}
          size="small"
          loading={loading}
          style={{
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
          rowClassName={(record) =>
            record.direction === 'send' ? 'frame-send' : 'frame-receive'
          }
        />
      </div>

      <style>{`
        .compact-messages-table .ant-table-small .ant-table-tbody > tr > td {
          padding: 4px 8px;
        }
        .compact-messages-table .ant-table-small .ant-table-thead > tr > th {
          padding: 4px 8px;
        }
        .frame-send td:first-child {
          border-left: 3px solid #52c41a;
        }
        .frame-receive td:first-child {
          border-left: 3px solid #1890ff;
        }
      `}</style>
    </div>
  );
};
