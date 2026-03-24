import { type ReactNode, type CSSProperties, useCallback, useRef, useEffect, useMemo, useState } from 'react';
import { ArrowUpOutlined, ArrowDownOutlined } from '@ant-design/icons';
import { theme } from 'antd';
import type { MessageItem } from './types';
import { useVirtualMessageList, type UseVirtualMessageListOptions } from './useVirtualMessageList';

export interface VirtualMessageListProps extends Omit<UseVirtualMessageListOptions, 'getItemKey'> {
  getItemKey?: (item: MessageItem, index: number) => string | number;
  renderItem: (item: MessageItem, index: number, isHighlighted: boolean) => ReactNode;
  highlightedIndices?: number[];
  currentHighlightIndex?: number;
  emptyContent?: ReactNode;
  className?: string;
  style?: CSSProperties;
  itemClassName?: string;
  itemStyle?: CSSProperties;
  onItemHeightChange?: (index: number) => void;
  showScrollControls?: boolean;
  scrollTopTestId?: string;
  scrollBottomTestId?: string;
}

const defaultGetItemKey = (item: MessageItem, index: number) => item.id || index;

export function VirtualMessageList({
  items,
  getItemKey = defaultGetItemKey,
  renderItem,
  highlightedIndices = [],
  currentHighlightIndex = -1,
  emptyContent,
  className,
  style,
  itemClassName,
  itemStyle,
  estimateSize = 100,
  overscan = 5,
  followTail = true,
  onFollowTailChange,
  onItemHeightChange,
  showScrollControls = false,
  scrollTopTestId,
  scrollBottomTestId,
}: VirtualMessageListProps) {
  const { token } = theme.useToken();
  const {
    virtualItems,
    totalSize,
    scrollRef,
    measureElement,
    scrollToIndex,
    scrollToLatest,
  } = useVirtualMessageList({
    items,
    getItemKey,
    estimateSize,
    overscan,
    followTail,
    onFollowTailChange,
  });

  const highlightedSet = useMemo(() => new Set(highlightedIndices), [highlightedIndices]);
  const [isAtTop, setIsAtTop] = useState(true);
  const [isAtBottom, setIsAtBottom] = useState(true);

  useEffect(() => {
    if (currentHighlightIndex >= 0 && currentHighlightIndex < items.length) {
      scrollToIndex(currentHighlightIndex, { align: 'center', behavior: 'smooth' });
    }
  }, [currentHighlightIndex, items.length, scrollToIndex]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el || !showScrollControls) return;

    const updateScrollState = () => {
      const threshold = 8;
      const distanceToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
      setIsAtTop(el.scrollTop <= threshold);
      setIsAtBottom(distanceToBottom <= threshold);
    };

    el.addEventListener('scroll', updateScrollState, { passive: true });
    updateScrollState();

    return () => {
      el.removeEventListener('scroll', updateScrollState);
    };
  }, [items.length, scrollRef, showScrollControls, totalSize]);

  const containerStyle: CSSProperties = {
    height: '100%',
    overflow: 'auto',
    ...style,
  };

  const innerStyle: CSSProperties = {
    height: totalSize,
    width: '100%',
    position: 'relative',
  };

  const wrapperStyle: CSSProperties = {
    height: '100%',
    position: 'relative',
  };

  const scrollButtonStyle: CSSProperties = {
    position: 'absolute',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: 36,
    height: 36,
    backgroundColor: token.colorBgElevated,
    color: token.colorTextSecondary,
    borderRadius: '50%',
    cursor: 'pointer',
    boxShadow: '0 2px 8px rgba(0, 0, 0, 0.08)',
    zIndex: 10,
    border: `1px solid ${token.colorBorderSecondary}`,
    transition: 'opacity 0.3s ease, transform 0.3s ease, background-color 0.2s',
    left: '50%',
    transform: 'translateX(-50%)',
  };

  if (items.length === 0) {
    return (
      <div className={className} style={containerStyle}>
        {emptyContent}
      </div>
    );
  }

  return (
    <div style={wrapperStyle}>
      <div ref={scrollRef} className={className} style={containerStyle}>
        <div style={innerStyle}>
          {virtualItems.map((virtualRow) => {
            const item = items[virtualRow.index];
            const isHighlighted = highlightedSet.has(virtualRow.index);
            const isCurrentHighlight = virtualRow.index === currentHighlightIndex;

            return (
              <VirtualRow
                key={virtualRow.key}
                virtualRow={virtualRow}
                measureElement={measureElement}
                className={itemClassName}
                style={itemStyle}
                isCurrentHighlight={isCurrentHighlight}
                onHeightChange={onItemHeightChange ? () => onItemHeightChange(virtualRow.index) : undefined}
              >
                {renderItem(item, virtualRow.index, isHighlighted)}
              </VirtualRow>
            );
          })}
        </div>
      </div>
      {showScrollControls && !isAtTop && (
        <div
          style={{ ...scrollButtonStyle, top: 16 }}
          onClick={() => scrollToIndex(0, { align: 'start', behavior: 'smooth' })}
          data-testid={scrollTopTestId}
        >
          <ArrowUpOutlined style={{ fontSize: 14 }} />
        </div>
      )}
      {showScrollControls && !isAtBottom && (
        <div
          style={{ ...scrollButtonStyle, bottom: 16 }}
          onClick={() => scrollToLatest()}
          data-testid={scrollBottomTestId}
        >
          <ArrowDownOutlined style={{ fontSize: 14 }} />
        </div>
      )}
    </div>
  );
}

interface VirtualRowProps {
  virtualRow: { index: number; start: number; size: number; key: string | number | bigint };
  measureElement: (node: HTMLElement | null) => void;
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  isCurrentHighlight?: boolean;
  onHeightChange?: () => void;
}

function VirtualRow({
  virtualRow,
  measureElement,
  children,
  className,
  style,
  isCurrentHighlight,
  onHeightChange,
}: VirtualRowProps) {
  const rowRef = useRef<HTMLDivElement>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);

  const handleRef = useCallback((node: HTMLDivElement | null) => {
    rowRef.current = node;
    measureElement(node);

    if (resizeObserverRef.current) {
      resizeObserverRef.current.disconnect();
      resizeObserverRef.current = null;
    }

    if (node && onHeightChange) {
      resizeObserverRef.current = new ResizeObserver(() => {
        measureElement(node);
        onHeightChange();
      });
      resizeObserverRef.current.observe(node);
    }
  }, [measureElement, onHeightChange]);

  useEffect(() => {
    return () => {
      if (resizeObserverRef.current) {
        resizeObserverRef.current.disconnect();
      }
    };
  }, []);

  const rowStyle: CSSProperties = {
    position: 'absolute',
    top: 0,
    left: 0,
    width: '100%',
    transform: `translateY(${virtualRow.start}px)`,
    ...style,
    ...(isCurrentHighlight ? { outline: '2px solid var(--ant-color-primary, #1890ff)', outlineOffset: -2 } : {}),
  };

  return (
    <div
      ref={handleRef}
      data-index={virtualRow.index}
      className={className}
      style={rowStyle}
    >
      {children}
    </div>
  );
}
