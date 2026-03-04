import { useRef, useCallback, useEffect } from 'react';
import { useVirtualizer, type Virtualizer, type VirtualItem } from '@tanstack/react-virtual';
import type { MessageItem } from './types';

export interface UseVirtualMessageListOptions {
  items: MessageItem[];
  getItemKey: (item: MessageItem, index: number) => string | number;
  estimateSize?: number;
  overscan?: number;
  followTail?: boolean;
  onFollowTailChange?: (following: boolean) => void;
}

export interface UseVirtualMessageListReturn {
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  virtualItems: VirtualItem[];
  totalSize: number;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  measureElement: (node: HTMLElement | null) => void;
  scrollToIndex: (index: number, options?: { align?: 'start' | 'center' | 'end'; behavior?: 'auto' | 'smooth' }) => void;
  scrollToLatest: () => void;
  isAtBottom: boolean;
  remeasure: () => void;
}

const BOTTOM_THRESHOLD = 50;

export function useVirtualMessageList({
  items,
  getItemKey,
  estimateSize = 100,
  overscan = 5,
  followTail = true,
  onFollowTailChange,
}: UseVirtualMessageListOptions): UseVirtualMessageListReturn {
  const scrollRef = useRef<HTMLDivElement>(null);
  const isAtBottomRef = useRef(true);
  const prevItemCountRef = useRef(items.length);
  const wasAtBottomBeforeNewItems = useRef(true);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => estimateSize,
    overscan,
    getItemKey: (index) => getItemKey(items[index], index),
    measureElement: (el) => {
      if (!el) return estimateSize;
      return el.getBoundingClientRect().height;
    },
  });

  const checkIsAtBottom = useCallback(() => {
    const container = scrollRef.current;
    if (!container) return true;
    const { scrollTop, scrollHeight, clientHeight } = container;
    return scrollHeight - (scrollTop + clientHeight) < BOTTOM_THRESHOLD;
  }, []);

  const scrollToIndex = useCallback((
    index: number, 
    options?: { align?: 'start' | 'center' | 'end'; behavior?: 'auto' | 'smooth' }
  ) => {
    virtualizer.scrollToIndex(index, {
      align: options?.align || 'center',
      behavior: options?.behavior || 'auto',
    });
  }, [virtualizer]);

  const scrollToLatest = useCallback(() => {
    if (items.length > 0) {
      virtualizer.scrollToIndex(items.length - 1, { align: 'end', behavior: 'smooth' });
      isAtBottomRef.current = true;
      onFollowTailChange?.(true);
    }
  }, [items.length, virtualizer, onFollowTailChange]);

  const remeasure = useCallback(() => {
    virtualizer.measure();
  }, [virtualizer]);

  useEffect(() => {
    const container = scrollRef.current;
    if (!container) return;

    const handleScroll = () => {
      const atBottom = checkIsAtBottom();
      if (isAtBottomRef.current !== atBottom) {
        isAtBottomRef.current = atBottom;
        if (followTail) {
          onFollowTailChange?.(atBottom);
        }
      }
    };

    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [checkIsAtBottom, followTail, onFollowTailChange]);

  useEffect(() => {
    wasAtBottomBeforeNewItems.current = isAtBottomRef.current;
  }, [items.length]);

  useEffect(() => {
    const prevCount = prevItemCountRef.current;
    const newCount = items.length;
    
    if (newCount > prevCount && followTail && wasAtBottomBeforeNewItems.current) {
      requestAnimationFrame(() => {
        virtualizer.scrollToIndex(newCount - 1, { align: 'end' });
        isAtBottomRef.current = true;
      });
    }
    
    prevItemCountRef.current = newCount;
  }, [items.length, followTail, virtualizer]);

  return {
    virtualizer,
    virtualItems: virtualizer.getVirtualItems(),
    totalSize: virtualizer.getTotalSize(),
    scrollRef,
    measureElement: virtualizer.measureElement,
    scrollToIndex,
    scrollToLatest,
    isAtBottom: isAtBottomRef.current,
    remeasure,
  };
}
