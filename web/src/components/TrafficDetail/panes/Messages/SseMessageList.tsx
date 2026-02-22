import { useRef } from 'react';
import { Typography, Button, Space, Tooltip, Empty, theme } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import type { WebSocketFrame, SessionTargetSearchState } from '../../../../types';
import { useMarkSearch } from '../../hooks/useMarkSearch';
import { SseEventCard } from './SseEventCard';

const { Text } = Typography;

interface SseMessageListProps {
  frames: WebSocketFrame[];
  filteredFrames: WebSocketFrame[];
  loading: boolean;
  hasMore: boolean;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  onLoadMore: () => void;
  onRefresh: () => void;
}

export const SseMessageList = ({
  frames,
  filteredFrames,
  loading,
  hasMore,
  searchValue,
  onSearch,
  onLoadMore,
  onRefresh,
}: SseMessageListProps) => {
  const { token } = theme.useToken();
  const containerRef = useRef<HTMLDivElement>(null);

  useMarkSearch(searchValue, () => containerRef.current, onSearch);

  if (frames.length === 0) {
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
        <Empty description="No SSE events yet" />
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          marginBottom: 8,
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          flexShrink: 0,
        }}
      >
        <Text type="secondary" style={{ fontSize: 12 }}>
          {filteredFrames.length} of {frames.length} events
          {hasMore && ' (more available)'}
        </Text>
        <Space>
          {hasMore && (
            <Button size="small" onClick={onLoadMore} loading={loading}>
              Load More
            </Button>
          )}
          <Tooltip title="Refresh">
            <Button
              size="small"
              icon={<ReloadOutlined />}
              onClick={onRefresh}
              loading={loading}
            />
          </Tooltip>
        </Space>
      </div>

      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          minHeight: 0,
        }}
      >
        {filteredFrames.map((frame) => (
          <SseEventCard
            key={frame.frame_id}
            frame={frame}
            searchValue={searchValue.value}
          />
        ))}

        {filteredFrames.length === 0 && frames.length > 0 && (
          <div
            style={{
              padding: 24,
              textAlign: 'center',
              color: token.colorTextSecondary,
            }}
          >
            No events match your search
          </div>
        )}
      </div>
    </div>
  );
};
