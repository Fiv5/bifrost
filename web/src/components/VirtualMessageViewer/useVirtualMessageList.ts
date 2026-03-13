import { useRef, useCallback } from 'react';
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

  // eslint-disable-next-line react-hooks/incompatible-library
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
      if (followTail) {
        onFollowTailChange?.(true);
      }
    }
  }, [items.length, virtualizer, onFollowTailChange, followTail]);

  const remeasure = useCallback(() => {
    virtualizer.measure();
  }, [virtualizer]);

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
