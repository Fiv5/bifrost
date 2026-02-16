import { useEffect, useRef, useCallback } from 'react';

export const useTextSelection = (ready: boolean) => {
  const ref = useRef<HTMLDivElement>(null);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
      e.preventDefault();
      e.stopImmediatePropagation();

      if (!ref.current) {
        return;
      }

      const selection = window.getSelection();
      selection?.removeAllRanges();

      const range = document.createRange();
      range.setStart(ref.current, 0);
      range.setEnd(ref.current, ref.current.childNodes.length);

      selection?.addRange(range);
    }
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (!el || !ready) {
      return;
    }

    el.setAttribute('tabindex', '-1');
    el.addEventListener('keydown', handleKeyDown);

    return () => {
      el.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown, ready]);

  return ref;
};
