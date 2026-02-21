import { create } from 'zustand';
import type { TrafficSummary, TrafficRecord, TrafficFilter, ToolbarFilters, FilterCondition, TrafficUpdatesFilter } from '../types';
import * as api from '../api';

interface TrafficState {
  records: TrafficSummary[];
  currentRecord: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  serverTotal: number;
  hasMore: boolean;
  lastId: string | null;
  pendingIds: Set<string>;
  toolbarFilters: ToolbarFilters;
  filterConditions: FilterCondition[];
  paused: boolean;
  loading: boolean;
  detailLoading: boolean;
  polling: boolean;
  error: string | null;
  pollTimeoutId: number | null;
  autoScroll: boolean;
  newRecordsCount: number;

  startPolling: () => void;
  stopPolling: () => void;
  fetchUpdates: () => Promise<void>;
  fetchInitialData: () => Promise<void>;
  fetchInitialDataWithTransition: () => Promise<void>;
  fetchTrafficDetail: (id: string) => Promise<void>;
  clearTraffic: () => Promise<boolean>;
  setToolbarFilters: (filters: ToolbarFilters) => void;
  setFilterConditions: (conditions: FilterCondition[]) => void;
  setPaused: (paused: boolean) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  clearNewRecordsCount: () => void;
  clearError: () => void;
  clearCurrentRecord: () => void;
  initFromUrl: (filters: FilterCondition[], toolbar: ToolbarFilters | null) => void;
}

const POLL_INTERVAL = 1000;
const BATCH_LIMIT = 100;

const buildFilterFromToolbar = (toolbar: ToolbarFilters, conditions: FilterCondition[]): Partial<TrafficFilter> => {
  const filter: Partial<TrafficFilter> = {};

  if (toolbar.rule.includes('Hit Rule')) {
    filter.has_rule_hit = true;
  }

  if (toolbar.protocol.length > 0) {
    const protocols = toolbar.protocol.map(p => p.toLowerCase());
    if (protocols.length === 1) {
      filter.protocol = protocols[0];
    }
  }

  if (toolbar.status.length > 0) {
    const statusRanges: { min: number; max: number }[] = [];
    toolbar.status.forEach(s => {
      if (s === '1xx') statusRanges.push({ min: 100, max: 199 });
      else if (s === '2xx') statusRanges.push({ min: 200, max: 299 });
      else if (s === '3xx') statusRanges.push({ min: 300, max: 399 });
      else if (s === '4xx') statusRanges.push({ min: 400, max: 499 });
      else if (s === '5xx') statusRanges.push({ min: 500, max: 599 });
    });
    if (statusRanges.length === 1) {
      filter.status_min = statusRanges[0].min;
      filter.status_max = statusRanges[0].max;
    }
  }

  if (toolbar.type.length > 0) {
    const typeMap: Record<string, string> = {
      'JSON': 'json',
      'Form': 'form',
      'XML': 'xml',
      'JS': 'javascript',
      'CSS': 'css',
      'Font': 'font',
      'Doc': 'html',
      'Media': 'image',
      'SSE': 'event-stream',
    };
    const types = toolbar.type.map(t => typeMap[t] || t.toLowerCase());
    if (types.length === 1) {
      filter.content_type = types[0];
    }
  }

  conditions.forEach(cond => {
    if (!cond.value) return;
    const value = cond.value;
    switch (cond.field) {
      case 'url':
        filter.url_contains = value;
        break;
      case 'host':
        filter.host = value;
        break;
      case 'path':
        filter.path_contains = value;
        break;
      case 'method':
        filter.method = value.toUpperCase();
        break;
      case 'content_type':
        filter.content_type = value;
        break;
      case 'request_header':
      case 'response_header':
        filter.header_contains = value;
        break;
      case 'domain':
        filter.domain = value;
        break;
      case 'client_app':
        filter.client_app = value;
        break;
    }
  });

  return filter;
};

export const useTrafficStore = create<TrafficState>((set, get) => ({
  records: [],
  currentRecord: null,
  requestBody: null,
  responseBody: null,
  serverTotal: 0,
  hasMore: false,
  lastId: null,
  pendingIds: new Set(),
  toolbarFilters: { rule: [], protocol: [], type: [], status: [] },
  filterConditions: [],
  paused: false,
  loading: false,
  detailLoading: false,
  polling: false,
  error: null,
  pollTimeoutId: null,
  autoScroll: true,
  newRecordsCount: 0,

  startPolling: () => {
    const state = get();
    if (state.polling) return;

    set({ polling: true });
    get().fetchUpdates();
  },

  stopPolling: () => {
    const state = get();
    if (state.pollTimeoutId) {
      clearTimeout(state.pollTimeoutId);
    }
    set({ polling: false, pollTimeoutId: null });
  },

  fetchInitialData: async () => {
    set({ loading: true, error: null });
    try {
      const state = get();
      const toolbarFilter = buildFilterFromToolbar(state.toolbarFilters, state.filterConditions);
      const filter: TrafficUpdatesFilter = {
        ...toolbarFilter,
        limit: BATCH_LIMIT,
      };
      const response = await api.getTrafficUpdates(filter);

      const newPendingIds = new Set<string>();
      response.new_records.forEach(r => {
        if (r.status === 0 || ((r.is_websocket || r.is_sse) && r.socket_status?.is_open)) {
          newPendingIds.add(r.id);
        }
      });

      const lastRecord = response.new_records[response.new_records.length - 1];

      set({
        records: response.new_records,
        serverTotal: response.server_total,
        hasMore: response.has_more,
        lastId: lastRecord?.id || null,
        pendingIds: newPendingIds,
        loading: false,
      });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  fetchInitialDataWithTransition: async () => {
    set({ error: null });
    try {
      const state = get();
      const toolbarFilter = buildFilterFromToolbar(state.toolbarFilters, state.filterConditions);
      const filter: TrafficUpdatesFilter = {
        ...toolbarFilter,
        limit: BATCH_LIMIT,
      };
      const response = await api.getTrafficUpdates(filter);

      const newPendingIds = new Set<string>();
      response.new_records.forEach(r => {
        if (r.status === 0 || ((r.is_websocket || r.is_sse) && r.socket_status?.is_open)) {
          newPendingIds.add(r.id);
        }
      });

      const lastRecord = response.new_records[response.new_records.length - 1];

      set({
        records: response.new_records,
        serverTotal: response.server_total,
        hasMore: response.has_more,
        lastId: lastRecord?.id || null,
        pendingIds: newPendingIds,
      });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  fetchUpdates: async () => {
    const state = get();
    if (state.paused || !state.polling) return;

    try {
      const toolbarFilter = buildFilterFromToolbar(state.toolbarFilters, state.filterConditions);
      const pendingIdsArray = Array.from(state.pendingIds);

      const filter: TrafficUpdatesFilter = {
        ...toolbarFilter,
        after_id: state.lastId || undefined,
        pending_ids: pendingIdsArray.length > 0 ? pendingIdsArray.join(',') : undefined,
        limit: BATCH_LIMIT,
      };

      const response = await api.getTrafficUpdates(filter);

      set((prevState) => {
        const recordsMap = new Map(prevState.records.map(r => [r.id, r]));

        response.updated_records.forEach(r => {
          recordsMap.set(r.id, r);
        });

        response.new_records.forEach(r => {
          if (!recordsMap.has(r.id)) {
            recordsMap.set(r.id, r);
          }
        });

        const newPendingIds = new Set(prevState.pendingIds);

        response.updated_records.forEach(r => {
          const isPending = r.status === 0 || ((r.is_websocket || r.is_sse) && r.socket_status?.is_open);
          if (!isPending) {
            newPendingIds.delete(r.id);
          }
        });

        response.new_records.forEach(r => {
          const isPending = r.status === 0 || ((r.is_websocket || r.is_sse) && r.socket_status?.is_open);
          if (isPending) {
            newPendingIds.add(r.id);
          }
        });

        const allRecords = Array.from(recordsMap.values());
        allRecords.sort((a, b) => a.sequence - b.sequence);

        const lastRecord = response.new_records[response.new_records.length - 1];
        const newLastId = lastRecord?.id || prevState.lastId;

        const newCount = response.new_records.length;
        const updatedNewRecordsCount = prevState.autoScroll
          ? 0
          : prevState.newRecordsCount + newCount;

        return {
          records: allRecords,
          serverTotal: response.server_total,
          hasMore: response.has_more,
          lastId: newLastId,
          pendingIds: newPendingIds,
          newRecordsCount: updatedNewRecordsCount,
        };
      });

      const currentState = get();
      if (currentState.polling) {
        const nextDelay = response.has_more ? 0 : POLL_INTERVAL;
        const timeoutId = window.setTimeout(() => {
          get().fetchUpdates();
        }, nextDelay);
        set({ pollTimeoutId: timeoutId });
      }
    } catch (e) {
      set({ error: (e as Error).message });

      const currentState = get();
      if (currentState.polling) {
        const timeoutId = window.setTimeout(() => {
          get().fetchUpdates();
        }, POLL_INTERVAL);
        set({ pollTimeoutId: timeoutId });
      }
    }
  },

  fetchTrafficDetail: async (id: string) => {
    set({ detailLoading: true, error: null, requestBody: null, responseBody: null });
    try {
      const record = await api.getTrafficDetail(id);
      set({ currentRecord: record, detailLoading: false });

      api.getRequestBody(id).then(body => {
        set({ requestBody: body });
      }).catch(() => { });

      api.getResponseBody(id).then(body => {
        set({ responseBody: body });
      }).catch(() => { });
    } catch (e) {
      set({ error: (e as Error).message, detailLoading: false });
    }
  },

  clearTraffic: async () => {
    set({ loading: true, error: null });
    try {
      await api.clearTraffic();
      set({
        records: [],
        serverTotal: 0,
        hasMore: false,
        lastId: null,
        pendingIds: new Set(),
        currentRecord: null,
        requestBody: null,
        responseBody: null,
        loading: false
      });
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setToolbarFilters: (filters: ToolbarFilters) => {
    const state = get();
    state.stopPolling();
    set({
      toolbarFilters: filters,
      lastId: null,
      pendingIds: new Set(),
    });
    get().fetchInitialDataWithTransition().then(() => {
      get().startPolling();
    });
  },

  setFilterConditions: (conditions: FilterCondition[]) => {
    const state = get();
    state.stopPolling();
    set({
      filterConditions: conditions,
      lastId: null,
      pendingIds: new Set(),
    });
    get().fetchInitialDataWithTransition().then(() => {
      get().startPolling();
    });
  },

  setPaused: (paused: boolean) => {
    set({ paused });
    if (paused) {
      get().stopPolling();
    } else {
      get().startPolling();
    }
  },

  setAutoScroll: (autoScroll: boolean) => {
    set({ autoScroll });
    if (autoScroll) {
      set({ newRecordsCount: 0 });
    }
  },

  clearNewRecordsCount: () => set({ newRecordsCount: 0 }),

  clearError: () => set({ error: null }),

  clearCurrentRecord: () => set({
    currentRecord: null,
    requestBody: null,
    responseBody: null
  }),

  initFromUrl: (filters: FilterCondition[], toolbar: ToolbarFilters | null) => {
    set({
      filterConditions: filters,
      toolbarFilters: toolbar || { rule: [], protocol: [], type: [], status: [] },
    });
  },
}));
