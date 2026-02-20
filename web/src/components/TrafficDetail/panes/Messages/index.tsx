import { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { Table, Typography, Tag, theme, Button, Space, Tooltip, Empty, ConfigProvider, Modal } from 'antd';
import type { TableProps } from 'antd';
import {
  ArrowUpOutlined,
  ArrowDownOutlined,
  ReloadOutlined,
  CopyOutlined,
  ExpandOutlined,
} from '@ant-design/icons';
import dayjs from 'dayjs';
import hljs from 'highlight.js/lib/core';
import json from 'highlight.js/lib/languages/json';
import plaintext from 'highlight.js/lib/languages/plaintext';
import 'highlight.js/styles/github.css';
import type { WebSocketFrame, FrameDirection, FrameType, SessionTargetSearchState } from '../../../../types';
import { useMarkSearch } from '../../hooks/useMarkSearch';

hljs.registerLanguage('json', json);
hljs.registerLanguage('plaintext', plaintext);

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

const formatJson = (text: string): { formatted: string; isJson: boolean } => {
  try {
    const parsed = JSON.parse(text);
    return { formatted: JSON.stringify(parsed, null, 2), isJson: true };
  } catch {
    return { formatted: text, isJson: false };
  }
};

const highlightContent = (text: string): string => {
  const { formatted, isJson } = formatJson(text);
  try {
    const result = hljs.highlight(formatted, { language: isJson ? 'json' : 'plaintext' });
    return result.value;
  } catch {
    return formatted;
  }
};

const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
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
  const [selectedFrame, setSelectedFrame] = useState<WebSocketFrame | null>(null);
  const [detailModalOpen, setDetailModalOpen] = useState(false);

  const fetchFrames = useCallback(async (after?: number) => {
    setLoading(true);
    try {
      const params = new URLSearchParams();
      if (after !== undefined) {
        params.set('after', String(after));
      }
      params.set('limit', '100');

      const response = await fetch(
        `/_bifrost/api/traffic/${recordId}/frames?${params.toString()}`
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
    {
      title: '',
      key: 'actions',
      width: 70,
      render: (_: unknown, record: WebSocketFrame) => (
        <Space size={4}>
          <Tooltip title="Copy">
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={(e) => {
                e.stopPropagation();
                if (record.payload_preview) {
                  copyToClipboard(record.payload_preview);
                }
              }}
              disabled={!record.payload_preview}
            />
          </Tooltip>
          <Tooltip title="Expand">
            <Button
              type="text"
              size="small"
              icon={<ExpandOutlined />}
              onClick={(e) => {
                e.stopPropagation();
                setSelectedFrame(record);
                setDetailModalOpen(true);
              }}
              disabled={!record.payload_preview}
            />
          </Tooltip>
        </Space>
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

      <ConfigProvider
        theme={{
          components: {
            Table: {
              cellPaddingBlockSM: 2,
              cellPaddingInlineSM: 4,
            },
          },
        }}
      >
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
            `${record.direction === 'send' ? 'frame-send' : 'frame-receive'} message-row`
          }
          onRow={(record) => ({
            onClick: () => {
              if (record.payload_preview) {
                setSelectedFrame(record);
                setDetailModalOpen(true);
              }
            },
          })}
        />
      </ConfigProvider>

      <style>{`
        .frame-send td:first-child {
          border-left: 3px solid #52c41a;
        }
        .frame-receive td:first-child {
          border-left: 3px solid #1890ff;
        }
        .message-row {
          cursor: pointer;
        }
        .message-row:hover {
          background-color: ${token.colorBgTextHover};
        }
      `}</style>

      <Modal
        title={
          <Space>
            <DirectionIcon direction={selectedFrame?.direction ?? 'receive'} />
            <Tag color={getFrameTypeColor(selectedFrame?.frame_type ?? 'text')}>
              {selectedFrame?.frame_type?.toUpperCase()}
            </Tag>
            <Text type="secondary">
              #{selectedFrame?.frame_id} - {dayjs(selectedFrame?.timestamp).format('YYYY-MM-DD HH:mm:ss.SSS')}
            </Text>
          </Space>
        }
        open={detailModalOpen}
        onCancel={() => {
          setDetailModalOpen(false);
          setSelectedFrame(null);
        }}
        footer={
          <Space>
            <Button
              icon={<CopyOutlined />}
              onClick={() => {
                if (selectedFrame?.payload_preview) {
                  const { formatted } = formatJson(selectedFrame.payload_preview);
                  copyToClipboard(formatted);
                }
              }}
            >
              Copy
            </Button>
            <Button onClick={() => setDetailModalOpen(false)}>Close</Button>
          </Space>
        }
        width={700}
        styles={{
          body: {
            maxHeight: '60vh',
            overflow: 'auto',
          },
        }}
      >
        {selectedFrame?.payload_preview && (
          <pre
            style={{
              margin: 0,
              padding: 12,
              fontSize: 12,
              fontFamily: 'monospace',
              backgroundColor: token.colorBgLayout,
              borderRadius: 4,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              lineHeight: 1.5,
            }}
          >
            <code
              dangerouslySetInnerHTML={{
                __html: highlightContent(selectedFrame.payload_preview),
              }}
            />
          </pre>
        )}
      </Modal>
    </div>
  );
};
