import { type ReactNode, type CSSProperties, useCallback, useRef, useEffect, useMemo } from 'react';
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
}: VirtualMessageListProps) {
  const {
    virtualItems,
    totalSize,
    scrollRef,
    measureElement,
    scrollToIndex,
  } = useVirtualMessageList({
    items,
    getItemKey,
    estimateSize,
    overscan,
    followTail,
    onFollowTailChange,
  });

  const highlightedSet = useMemo(() => new Set(highlightedIndices), [highlightedIndices]);

  useEffect(() => {
    if (currentHighlightIndex >= 0 && currentHighlightIndex < items.length) {
      scrollToIndex(currentHighlightIndex, { align: 'center', behavior: 'smooth' });
    }
  }, [currentHighlightIndex, items.length, scrollToIndex]);

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

  if (items.length === 0) {
    return (
      <div className={className} style={containerStyle}>
        {emptyContent}
      </div>
    );
  }

  return (
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
