import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type {
  SearchScope,
  SearchFilters,
  SearchResultItem,
  SearchResponse,
  SearchRequest,
  TrafficSummary,
  TrafficSummaryCompact,
} from '../types';
import { TrafficFlags } from '../types';

interface SearchState {
  mode: 'normal' | 'search';
  keyword: string;
  scope: SearchScope;

  results: SearchResultItem[];
  totalSearched: number;
  totalMatched: number;
  hasMore: boolean;
  nextCursor: number | null;

  isSearching: boolean;
  isLoadingMore: boolean;
  searchId: string | null;

  setMode: (mode: 'normal' | 'search') => void;
  setKeyword: (keyword: string) => void;
  setScope: (scope: Partial<SearchScope>) => void;
  search: (filters: SearchFilters) => Promise<void>;
  loadMore: (filters: SearchFilters) => Promise<void>;
  reset: () => void;
}

const defaultScope: SearchScope = {
  request_body: false,
  response_body: false,
  request_headers: false,
  response_headers: false,
  url: false,
  websocket_messages: false,
  sse_events: false,
  all: true,
};

export const useSearchStore = create<SearchState>()(
  persist(
    (set, get) => ({
      mode: 'normal',
      keyword: '',
      scope: { ...defaultScope },

  results: [],
  totalSearched: 0,
  totalMatched: 0,
  hasMore: false,
  nextCursor: null,

  isSearching: false,
  isLoadingMore: false,
  searchId: null,

  setMode: (mode) => {
    if (mode === 'normal') {
      set({
        mode,
        results: [],
        totalSearched: 0,
        totalMatched: 0,
        hasMore: false,
        nextCursor: null,
        searchId: null,
      });
    } else {
      set({ mode });
    }
  },

  setKeyword: (keyword) => set({ keyword }),

  setScope: (scopeUpdate) => {
    const { scope } = get();
    if (scopeUpdate.all === true) {
      set({
        scope: {
          ...scope,
          request_body: false,
          response_body: false,
          request_headers: false,
          response_headers: false,
          url: false,
          websocket_messages: false,
          sse_events: false,
          all: true,
        },
      });
    } else {
      const newScope = { ...scope, ...scopeUpdate, all: false };
      const hasAny = newScope.request_body || newScope.response_body ||
        newScope.request_headers || newScope.response_headers || newScope.url ||
        newScope.websocket_messages || newScope.sse_events;
      if (!hasAny) {
        newScope.all = true;
      }
      set({ scope: newScope });
    }
  },

  search: async (filters) => {
    const { keyword, scope } = get();
    if (!keyword.trim()) {
      return;
    }

    set({
      isSearching: true,
      results: [],
      totalSearched: 0,
      totalMatched: 0,
      hasMore: false,
      nextCursor: null,
    });

    try {
      const request: SearchRequest = {
        keyword: keyword.trim(),
        scope,
        filters,
        limit: 50,
      };

      const response = await fetch('/_bifrost/api/search', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });

      if (!response.ok) {
        throw new Error(`Search failed: ${response.statusText}`);
      }

      const data: SearchResponse = await response.json();

      set({
        results: data.results,
        totalSearched: data.total_searched,
        totalMatched: data.total_matched,
        hasMore: data.has_more,
        nextCursor: data.next_cursor,
        searchId: data.search_id,
        isSearching: false,
      });
    } catch (error) {
      console.error('[SearchStore] Search failed:', error);
      set({ isSearching: false });
    }
  },

  loadMore: async (filters) => {
    const { keyword, scope, nextCursor, hasMore, isLoadingMore, results } = get();
    if (!keyword.trim() || !hasMore || isLoadingMore || nextCursor === null) {
      return;
    }

    set({ isLoadingMore: true });

    try {
      const request: SearchRequest = {
        keyword: keyword.trim(),
        scope,
        filters,
        cursor: nextCursor,
        limit: 50,
      };

      const response = await fetch('/_bifrost/api/search', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });

      if (!response.ok) {
        throw new Error(`Search failed: ${response.statusText}`);
      }

      const data: SearchResponse = await response.json();

      set({
        results: [...results, ...data.results],
        totalSearched: get().totalSearched + data.total_searched,
        totalMatched: get().totalMatched + data.total_matched,
        hasMore: data.has_more,
        nextCursor: data.next_cursor,
        isLoadingMore: false,
      });
    } catch (error) {
      console.error('[SearchStore] Load more failed:', error);
      set({ isLoadingMore: false });
    }
  },

      reset: () => {
        set({
          mode: 'normal',
          keyword: '',
          scope: { ...defaultScope },
          results: [],
          totalSearched: 0,
          totalMatched: 0,
          hasMore: false,
          nextCursor: null,
          isSearching: false,
          isLoadingMore: false,
          searchId: null,
        });
      },
    }),
    {
      name: 'bifrost-search-ui',
      partialize: (state) => ({
        mode: state.mode,
        keyword: state.keyword,
        scope: state.scope,
      }),
      version: 1,
    },
  ),
);

export const compactToSummary = (c: TrafficSummaryCompact): TrafficSummary => {
  return {
    id: c.id,
    sequence: c.seq,
    timestamp: c.ts,
    method: c.m,
    host: c.h,
    path: c.p,
    status: c.s,
    content_type: c.ct || null,
    request_size: c.req_sz,
    response_size: c.res_sz,
    duration_ms: c.dur,
    protocol: c.proto,
    client_ip: c.cip,
    client_app: c.capp || undefined,
    client_pid: c.cpid || undefined,
    is_tunnel: (c.flags & TrafficFlags.IS_TUNNEL) !== 0,
    is_websocket: (c.flags & TrafficFlags.IS_WEBSOCKET) !== 0,
    is_sse: (c.flags & TrafficFlags.IS_SSE) !== 0,
    is_h3: (c.flags & TrafficFlags.IS_H3) !== 0,
    has_rule_hit: (c.flags & TrafficFlags.HAS_RULE_HIT) !== 0,
    matched_rule_count: c.rc || 0,
    matched_protocols: c.rp || [],
    frame_count: c.fc,
    socket_status: c.ss || undefined,
    url: `${c.proto === 'https' ? 'https' : 'http'}://${c.h}${c.p}`,
    start_time: c.st,
    end_time: c.et || undefined,
  };
};
