import { useMemo, useCallback } from 'react';
import { Typography, Button, Space, Tooltip, Empty, Input, Tag, theme } from 'antd';
import { 
  ReloadOutlined, 
  SearchOutlined, 
  FilterOutlined, 
  HighlightOutlined,
  ArrowUpOutlined,
  ArrowDownOutlined,
  FullscreenOutlined,
} from '@ant-design/icons';
import type { WebSocketFrame } from '../../../../types';
import {
  VirtualMessageList,
  MessageItemCard,
  useMessageSearch,
  normalizeWSFrame,
  type MessageItem,
} from '../../../VirtualMessageViewer';

const { Text } = Typography;
const { Search } = Input;

interface SseMessageListProps {
  frames: WebSocketFrame[];
  loading: boolean;
  hasMore: boolean;
  searchQuery?: string;
  searchMode?: 'highlight' | 'filter';
  onSearchChange?: (query: string) => void;
  onSearchModeChange?: (mode: 'highlight' | 'filter') => void;
  onLoadMore: () => void;
  onRefresh: () => void;
  onFullscreenOpen?: () => void;
}

export const SseMessageList = ({
  frames,
  loading,
  hasMore,
  searchQuery = '',
  searchMode = 'highlight',
  onSearchChange,
  onSearchModeChange,
  onLoadMore,
  onRefresh,
  onFullscreenOpen,
}: SseMessageListProps) => {
  const { token } = theme.useToken();

  const normalizedMessages = useMemo<MessageItem[]>(() => {
    return frames.map(normalizeWSFrame);
  }, [frames]);

  const {
    searchState,
    setQuery,
    setMatchMode,
    filteredItems,
    highlightedIndices,
    goToNext,
    goToPrev,
    matchTokens,
  } = useMessageSearch({
    items: normalizedMessages,
    initialQuery: searchQuery,
    initialMatchMode: searchMode,
  });

  const displayItems = searchMode === 'filter' && searchQuery ? filteredItems : normalizedMessages;

  const handleSearchChange = useCallback((value: string) => {
    setQuery(value);
    onSearchChange?.(value);
  }, [setQuery, onSearchChange]);

  const handleModeChange = useCallback((mode: 'highlight' | 'filter') => {
    setMatchMode(mode);
    onSearchModeChange?.(mode);
  }, [setMatchMode, onSearchModeChange]);

  const getItemKey = useCallback((item: MessageItem) => item.id, []);

  const renderItem = useCallback((item: MessageItem) => (
    <MessageItemCard
      message={item}
      searchTokens={searchMode === 'highlight' ? matchTokens : []}
    />
  ), [searchMode, matchTokens]);

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

  const matchInfo = searchState.total > 0 
    ? `${searchState.currentIndex >= 0 ? searchState.currentIndex + 1 : 0}/${searchState.total}`
    : null;

  return (
    <div
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
          gap: 8,
          flexShrink: 0,
          flexWrap: 'wrap',
        }}
      >
        <Space size="small">
          <Search
            placeholder="Search events..."
            allowClear
            value={searchQuery}
            onChange={(e) => handleSearchChange(e.target.value)}
            onSearch={handleSearchChange}
            style={{ width: 180 }}
            size="small"
            prefix={<SearchOutlined />}
          />

          <Button.Group size="small">
            <Tooltip title="Highlight matches">
              <Button
                type={searchMode === 'highlight' ? 'primary' : 'default'}
                icon={<HighlightOutlined />}
                onClick={() => handleModeChange('highlight')}
              />
            </Tooltip>
            <Tooltip title="Filter matches only">
              <Button
                type={searchMode === 'filter' ? 'primary' : 'default'}
                icon={<FilterOutlined />}
                onClick={() => handleModeChange('filter')}
              />
            </Tooltip>
          </Button.Group>

          {matchInfo && (
            <>
              <Tag color="blue" style={{ margin: 0 }}>{matchInfo}</Tag>
              <Button.Group size="small">
                <Button icon={<ArrowUpOutlined />} onClick={goToPrev} />
                <Button icon={<ArrowDownOutlined />} onClick={goToNext} />
              </Button.Group>
            </>
          )}

          <Text type="secondary" style={{ fontSize: 11 }}>
            {displayItems.length} of {frames.length} events
            {hasMore && ' (+)'}
          </Text>
        </Space>

        <Space size="small">
          {onFullscreenOpen && (
            <Tooltip title="Fullscreen">
              <Button
                size="small"
                icon={<FullscreenOutlined />}
                onClick={onFullscreenOpen}
              />
            </Tooltip>
          )}
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

      <div style={{ flex: 1, overflow: 'hidden', minHeight: 0 }}>
        <VirtualMessageList
          items={displayItems}
          getItemKey={getItemKey}
          renderItem={renderItem}
          highlightedIndices={searchMode === 'highlight' ? highlightedIndices : []}
          currentHighlightIndex={searchState.currentIndex >= 0 ? searchState.matchedIndices[searchState.currentIndex] : -1}
          estimateSize={140}
          overscan={3}
          followTail={false}
          emptyContent={
            <div
              style={{
                padding: 24,
                textAlign: 'center',
                color: token.colorTextSecondary,
              }}
            >
              {searchMode === 'filter' && searchQuery
                ? 'No events match your search'
                : 'No events'}
            </div>
          }
        />
      </div>
    </div>
  );
};
