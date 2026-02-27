import Mark from 'mark.js';
import { useEffect, useRef, useCallback, useLayoutEffect } from 'react';
import type { SessionTargetSearchState } from '../../../types';

export const useMarkSearch = (
  searchValue: SessionTargetSearchState,
  getWrapNode: () => HTMLElement | null,
  onSearch: (v: Partial<SessionTargetSearchState>) => void
) => {
  const markRef = useRef<Mark | null>(null);
  const currentRef = useRef<number>(0);
  const markListRef = useRef<HTMLCollectionOf<HTMLElement> | null>(null);
  const { value, next } = searchValue;
  const recordWrapRef = useRef<HTMLElement | null>(null);
  const getWrapNodeRef = useRef(getWrapNode);
  const onSearchRef = useRef(onSearch);

  useLayoutEffect(() => {
    getWrapNodeRef.current = getWrapNode;
  }, [getWrapNode]);

  useLayoutEffect(() => {
    onSearchRef.current = onSearch;
  }, [onSearch]);

  const getNode = useCallback(() => {
    return getWrapNodeRef.current();
  }, []);

  const ensureMarkInstance = useCallback(() => {
    const wrapNode = getNode();
    if (!wrapNode) {
      markRef.current = null;
      recordWrapRef.current = null;
      return null;
    }
    if (wrapNode !== recordWrapRef.current) {
      recordWrapRef.current = wrapNode;
      markRef.current = new Mark(wrapNode);
    }
    return markRef.current;
  }, [getNode]);

  useLayoutEffect(() => {
    ensureMarkInstance();
  });

  const handleInitCurrent = useCallback(() => {
    if (!markListRef.current?.length) {
      return;
    }
    const { current } = currentRef;
    const { current: markList } = markListRef;
    const markEl = markList[current];
    if (markEl) markEl.className = '';
  }, []);

  const handleJumpToCurrent = useCallback((nextValue: number) => {
    const markList = markListRef.current;
    if (!markList?.length) {
      return;
    }

    const preMark = markList[currentRef.current];
    if (preMark) preMark.removeAttribute('class');

    currentRef.current = Math.min(markList.length - 1, Math.max(0, (nextValue ?? 1) - 1));
    const currentMark = markList[currentRef.current];
    if (currentMark) {
      currentMark.className = 'mark-current';
      currentMark.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
    }
  }, []);

  const startMarkSearch = useCallback(() => {
    const mark = ensureMarkInstance();
    if (!mark) {
      return;
    }

    const onMarkDone = (total: number) => {
      onSearchRef.current({ total, next: 1 });
      currentRef.current = 0;
      const node = getNode();
      markListRef.current = node?.getElementsByTagName('mark') ?? null;
      handleJumpToCurrent(1);
    };

    const onUnMarkDone = () => {
      if (value) {
        mark.mark(value, {
          done: onMarkDone,
        });
      } else {
        onSearchRef.current({ total: 0, next: 1 });
        markListRef.current = null;
      }
    };

    mark.unmark({
      done: onUnMarkDone,
    });
  }, [value, getNode, ensureMarkInstance, handleJumpToCurrent]);

  useEffect(() => {
    const node = getNode();
    if (!node) {
      return;
    }
    const checkAndSearch = () => {
      if (node.offsetParent) {
        startMarkSearch();
      }
    };
    const timeoutId = setTimeout(checkAndSearch, 0);
    return () => clearTimeout(timeoutId);
  }, [value, getNode, startMarkSearch]);

  useEffect(() => {
    if (next === undefined || next === null) {
      return;
    }
    const node = recordWrapRef.current;
    if (!node?.offsetParent) {
      return;
    }
    handleInitCurrent();
    handleJumpToCurrent(next);
  }, [next, handleInitCurrent, handleJumpToCurrent]);

  return { startMarkSearch };
};
