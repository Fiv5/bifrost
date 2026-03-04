import { useMemo, useState, useCallback } from 'react';
import type { MessageItem, MessageSearchState } from './types';

export interface UseMessageSearchOptions {
  items: MessageItem[];
  initialQuery?: string;
  initialMatchMode?: 'highlight' | 'filter';
  caseSensitive?: boolean;
}

export interface UseMessageSearchReturn {
  searchState: MessageSearchState;
  setQuery: (query: string) => void;
  setMatchMode: (mode: 'highlight' | 'filter') => void;
  setCaseSensitive: (cs: boolean) => void;
  filteredItems: MessageItem[];
  highlightedIndices: number[];
  goToNext: () => void;
  goToPrev: () => void;
  goToIndex: (index: number) => void;
  clearSearch: () => void;
  matchTokens: string[];
}

function tokenize(query: string): string[] {
  return query
    .trim()
    .split(/\s+/)
    .filter(t => t.length > 0);
}

function matchesTokens(searchText: string, tokens: string[], caseSensitive: boolean): boolean {
  if (tokens.length === 0) return true;
  const text = caseSensitive ? searchText : searchText.toLowerCase();
  const normalizedTokens = caseSensitive ? tokens : tokens.map(t => t.toLowerCase());
  return normalizedTokens.every(token => text.includes(token));
}

export function useMessageSearch({
  items,
  initialQuery = '',
  initialMatchMode = 'highlight',
  caseSensitive: initialCaseSensitive = false,
}: UseMessageSearchOptions): UseMessageSearchReturn {
  const [query, setQuery] = useState(initialQuery);
  const [matchMode, setMatchMode] = useState<'highlight' | 'filter'>(initialMatchMode);
  const [caseSensitive, setCaseSensitive] = useState(initialCaseSensitive);
  const [currentIndex, setCurrentIndex] = useState(-1);

  const matchTokens = useMemo(() => tokenize(query), [query]);

  const matchedIndices = useMemo(() => {
    if (matchTokens.length === 0) return [];
    
    const indices: number[] = [];
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      const searchText = item.searchText || item.data || '';
      if (matchesTokens(searchText, matchTokens, caseSensitive)) {
        indices.push(i);
      }
    }
    return indices;
  }, [items, matchTokens, caseSensitive]);

  const filteredItems = useMemo(() => {
    if (matchMode !== 'filter' || matchTokens.length === 0) return items;
    return matchedIndices.map(i => items[i]);
  }, [items, matchMode, matchTokens, matchedIndices]);

  const highlightedIndices = useMemo(() => {
    if (matchMode === 'filter') {
      return filteredItems.map((_, i) => i);
    }
    return matchedIndices;
  }, [matchMode, matchedIndices, filteredItems]);

  const clampedCurrentIndex = useMemo(() => {
    if (matchedIndices.length === 0) return -1;
    if (currentIndex < 0) return -1;
    return Math.min(currentIndex, matchedIndices.length - 1);
  }, [currentIndex, matchedIndices.length]);

  const goToNext = useCallback(() => {
    if (matchedIndices.length === 0) return;
    setCurrentIndex(prev => {
      if (prev < 0) return 0;
      return (prev + 1) % matchedIndices.length;
    });
  }, [matchedIndices.length]);

  const goToPrev = useCallback(() => {
    if (matchedIndices.length === 0) return;
    setCurrentIndex(prev => {
      if (prev < 0) return matchedIndices.length - 1;
      return (prev - 1 + matchedIndices.length) % matchedIndices.length;
    });
  }, [matchedIndices.length]);

  const goToIndex = useCallback((index: number) => {
    if (index >= 0 && index < matchedIndices.length) {
      setCurrentIndex(index);
    }
  }, [matchedIndices.length]);

  const clearSearch = useCallback(() => {
    setQuery('');
    setCurrentIndex(-1);
  }, []);

  const handleSetQuery = useCallback((newQuery: string) => {
    setQuery(newQuery);
    setCurrentIndex(-1);
  }, []);

  const searchState: MessageSearchState = {
    query,
    caseSensitive,
    matchMode,
    matchedIndices,
    currentIndex: clampedCurrentIndex,
    total: matchedIndices.length,
  };

  return {
    searchState,
    setQuery: handleSetQuery,
    setMatchMode,
    setCaseSensitive,
    filteredItems,
    highlightedIndices,
    goToNext,
    goToPrev,
    goToIndex,
    clearSearch,
    matchTokens,
  };
}

export function highlightText(
  text: string,
  tokens: string[],
  caseSensitive: boolean = false
): { segments: Array<{ text: string; highlighted: boolean }> } {
  if (tokens.length === 0 || !text) {
    return { segments: [{ text, highlighted: false }] };
  }

  const normalizedText = caseSensitive ? text : text.toLowerCase();
  const normalizedTokens = caseSensitive ? tokens : tokens.map(t => t.toLowerCase());

  const matches: Array<{ start: number; end: number }> = [];

  for (const token of normalizedTokens) {
    let pos = 0;
    while (pos < normalizedText.length) {
      const idx = normalizedText.indexOf(token, pos);
      if (idx === -1) break;
      matches.push({ start: idx, end: idx + token.length });
      pos = idx + 1;
    }
  }

  if (matches.length === 0) {
    return { segments: [{ text, highlighted: false }] };
  }

  matches.sort((a, b) => a.start - b.start);

  const merged: Array<{ start: number; end: number }> = [];
  for (const m of matches) {
    if (merged.length === 0 || m.start > merged[merged.length - 1].end) {
      merged.push({ ...m });
    } else {
      merged[merged.length - 1].end = Math.max(merged[merged.length - 1].end, m.end);
    }
  }

  const segments: Array<{ text: string; highlighted: boolean }> = [];
  let pos = 0;
  for (const m of merged) {
    if (pos < m.start) {
      segments.push({ text: text.slice(pos, m.start), highlighted: false });
    }
    segments.push({ text: text.slice(m.start, m.end), highlighted: true });
    pos = m.end;
  }
  if (pos < text.length) {
    segments.push({ text: text.slice(pos), highlighted: false });
  }

  return { segments };
}
