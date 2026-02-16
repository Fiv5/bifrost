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
  const { value, next, tab } = searchValue;
  const recordWrapRef = useRef<HTMLElement | null>(null);

  const getNode = useCallback(() => {
    return getWrapNode();
  }, [getWrapNode]);

  useLayoutEffect(() => {
    const wrapNode = getNode();
    if (!wrapNode || wrapNode === recordWrapRef.current) {
      return;
    }
    recordWrapRef.current = wrapNode;
    markRef.current = new Mark(wrapNode);
  }, [getNode]);

  const handleInitCurrent = useCallback(() => {
    if (!markListRef.current?.length) {
      return;
    }
    const { current } = currentRef;
    const { current: markList } = markListRef;
    const markEl = markList[current];
    if (markEl) markEl.className = '';
  }, []);

  const handleJumpToCurrent = useCallback(() => {
    const markList = markListRef.current;
    if (!markList?.length) {
      return;
    }

    const preMark = markList[currentRef.current];
    if (preMark) preMark.removeAttribute('class');

    currentRef.current = Math.min(markList.length - 1, (next ?? 1) - 1);
    const currentMark = markList[currentRef.current];
    if (currentMark) {
      currentMark.className = 'mark-current';
      currentMark.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
    }
  }, [next]);

  const startMarkSearch = useCallback(() => {
    const mark = markRef.current;
    const onMarkDone = (total: number) => {
      onSearch({ total, next: 1 });
      currentRef.current = 0;
      markListRef.current = getNode()?.getElementsByTagName('mark') ?? null;
      handleJumpToCurrent();
    };

    const onUnMarkDone = () => {
      if (value) {
        mark?.mark(value, {
          done: onMarkDone,
        });
      }
    };

    mark?.unmark({
      done: onUnMarkDone,
    });
  }, [value, onSearch, getNode, handleJumpToCurrent]);

  useEffect(() => {
    if (!markRef.current) {
      return;
    }
    const node = getNode();
    if (!node?.offsetParent) {
      return;
    }
    startMarkSearch();
  }, [value, tab, getNode, startMarkSearch]);

  useEffect(() => {
    if (next === undefined) {
      return;
    }
    if (!recordWrapRef.current?.offsetParent) {
      return;
    }
    handleInitCurrent();
    handleJumpToCurrent();
  }, [next, handleInitCurrent, handleJumpToCurrent]);

  return { startMarkSearch };
};
