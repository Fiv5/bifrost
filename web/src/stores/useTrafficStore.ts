import { create } from 'zustand';
import type { TrafficSummary, TrafficRecord, ToolbarFilters, FilterCondition, TrafficUpdatesFilter } from '../types';
import * as api from '../api';
import pushService, { type TrafficUpdatesData } from '../services/pushService';

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
  scrollTop: number;
  usePush: boolean;
  pushUnsubscribe: (() => void) | null;

  startPolling: () => void;
  stopPolling: () => void;
  fetchUpdates: () => Promise<void>;
  fetchInitialData: () => Promise<void>;
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
  setScrollTop: (scrollTop: number) => void;
  handleTrafficPush: (data: TrafficUpdatesData) => void;
  enablePush: () => void;
  disablePush: () => void;
}

const POLL_INTERVAL = 1000;
const BATCH_LIMIT = 500;

const contentTypeMap: Record<string, string[]> = {
  'JSON': ['json', 'application/json'],
  'Form': ['form', 'x-www-form-urlencoded', 'multipart/form-data'],
  'XML': ['xml', 'application/xml', 'text/xml'],
  'JS': ['javascript', 'text/javascript', 'application/javascript'],
  'CSS': ['css', 'text/css'],
  'Font': ['font', 'woff', 'woff2', 'ttf', 'otf', 'eot'],
  'Doc': ['html', 'text/html'],
  'Media': ['image', 'video', 'audio', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'mp4', 'webm', 'mp3', 'wav'],
  'SSE': ['event-stream', 'text/event-stream'],
};

export const filterRecords = (
  records: TrafficSummary[],
  toolbar: ToolbarFilters,
  conditions: FilterCondition[]
): TrafficSummary[] => {
  return records.filter(record => {
    if (toolbar.rule.includes('Hit Rule') && !record.has_rule_hit) {
      return false;
    }

    if (toolbar.protocol.length > 0) {
      const protocol = record.protocol?.toUpperCase() || '';
      const matches = toolbar.protocol.some(p => {
        const pUpper = p.toUpperCase();
        if (pUpper === 'H2') return protocol.includes('HTTP/2');
        if (pUpper === 'HTTP') return protocol === 'HTTP/1.0' || protocol === 'HTTP/1.1';
        if (pUpper === 'HTTPS') return record.host?.startsWith('https:') || protocol.includes('HTTPS');
        if (pUpper === 'WS') return record.is_websocket && !record.host?.startsWith('wss:');
        if (pUpper === 'WSS') return record.is_websocket && record.host?.startsWith('wss:');
        return false;
      });
      if (!matches) return false;
    }

    if (toolbar.status.length > 0) {
      const status = record.status;
      const matches = toolbar.status.some(s => {
        if (s === 'error') return status === 0 || status >= 500;
        if (s === '1xx') return status >= 100 && status < 200;
        if (s === '2xx') return status >= 200 && status < 300;
        if (s === '3xx') return status >= 300 && status < 400;
        if (s === '4xx') return status >= 400 && status < 500;
        if (s === '5xx') return status >= 500 && status < 600;
        return false;
      });
      if (!matches) return false;
    }

    if (toolbar.type.length > 0) {
      const contentType = (record.content_type || '').toLowerCase();
      const matches = toolbar.type.some(t => {
        const patterns = contentTypeMap[t] || [t.toLowerCase()];
        return patterns.some(pattern => contentType.includes(pattern));
      });
      if (!matches) return false;
    }

    for (const cond of conditions) {
      if (!cond.value) continue;

      const value = cond.value;
      let fieldValue = '';

      switch (cond.field) {
        case 'url':
          fieldValue = `${record.host || ''}${record.path || ''}`;
          break;
        case 'host':
          fieldValue = record.host || '';
          break;
        case 'path':
          fieldValue = record.path || '';
          break;
        case 'method':
          fieldValue = record.method || '';
          break;
        case 'content_type':
          fieldValue = record.content_type || '';
          break;
        case 'client_app':
          fieldValue = record.client_app || '';
          break;
        case 'client_ip':
          fieldValue = record.client_ip || '';
          break;
        default:
          continue;
      }

      let matches = false;
      switch (cond.operator) {
        case 'contains':
          matches = fieldValue.toLowerCase().includes(value.toLowerCase());
          break;
        case 'equals':
          matches = fieldValue.toLowerCase() === value.toLowerCase();
          break;
        case 'regex':
          try {
            const regex = new RegExp(value, 'i');
            matches = regex.test(fieldValue);
          } catch {
            matches = false;
          }
          break;
        case 'not_contains':
          matches = !fieldValue.toLowerCase().includes(value.toLowerCase());
          break;
        default:
          matches = fieldValue.toLowerCase().includes(value.toLowerCase());
      }

      if (!matches) return false;
    }

    return true;
  });
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
  scrollTop: 0,
  usePush: true,
  pushUnsubscribe: null,

  startPolling: () => {
    const state = get();
    if (state.polling) return;

    set({ polling: true });

    if (state.usePush) {
      get().enablePush();
    } else {
      get().fetchUpdates();
    }
  },

  stopPolling: () => {
    const state = get();
    if (state.pollTimeoutId) {
      clearTimeout(state.pollTimeoutId);
    }
    if (state.usePush) {
      get().disablePush();
    }
    set({ polling: false, pollTimeoutId: null });
  },

  enablePush: () => {
    const state = get();
    if (state.pushUnsubscribe) return;

    const subscription = {
      last_traffic_id: state.lastId || undefined,
      pending_ids: Array.from(state.pendingIds),
    };

    pushService.connect(subscription);
    const unsubscribe = pushService.onTrafficUpdates((data) => {
      get().handleTrafficPush(data);
    });
    set({ pushUnsubscribe: unsubscribe });
  },

  disablePush: () => {
    const state = get();
    if (state.pushUnsubscribe) {
      state.pushUnsubscribe();
      set({ pushUnsubscribe: null });
    }
  },

  handleTrafficPush: (data: TrafficUpdatesData) => {
    const state = get();
    if (state.paused) return;

    set((prevState) => {
      const recordsMap = new Map(prevState.records.map(r => [r.id, r]));

      data.updated_records.forEach(r => {
        recordsMap.set(r.id, r);
      });

      data.new_records.forEach(r => {
        if (!recordsMap.has(r.id)) {
          recordsMap.set(r.id, r);
        }
      });

      const newPendingIds = new Set(prevState.pendingIds);

      data.updated_records.forEach(r => {
        const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
        if (!isPending) {
          newPendingIds.delete(r.id);
        }
      });

      data.new_records.forEach(r => {
        const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
        if (isPending) {
          newPendingIds.add(r.id);
        }
      });

      const allRecords = Array.from(recordsMap.values());
      allRecords.sort((a, b) => a.sequence - b.sequence);

      const lastRecord = data.new_records[data.new_records.length - 1];
      const newLastId = lastRecord?.id || prevState.lastId;

      const newCount = data.new_records.length;
      const updatedNewRecordsCount = prevState.autoScroll
        ? 0
        : prevState.newRecordsCount + newCount;

      pushService.updateSubscription({
        last_traffic_id: newLastId || undefined,
        pending_ids: Array.from(newPendingIds),
      });

      return {
        records: allRecords,
        serverTotal: data.server_total,
        hasMore: data.has_more,
        lastId: newLastId,
        pendingIds: newPendingIds,
        newRecordsCount: updatedNewRecordsCount,
      };
    });
  },

  fetchInitialData: async () => {
    set({ loading: true, error: null });
    try {
      const filter: TrafficUpdatesFilter = {
        limit: BATCH_LIMIT,
      };
      const response = await api.getTrafficUpdates(filter);

      const newPendingIds = new Set<string>();
      response.new_records.forEach(r => {
        if (r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open)) {
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

  fetchUpdates: async () => {
    const state = get();
    if (state.paused || !state.polling) return;

    try {
      const pendingIdsArray = Array.from(state.pendingIds);

      const filter: TrafficUpdatesFilter = {
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
          const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
          if (!isPending) {
            newPendingIds.delete(r.id);
          }
        });

        response.new_records.forEach(r => {
          const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
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
    set({ toolbarFilters: filters });
  },

  setFilterConditions: (conditions: FilterCondition[]) => {
    set({ filterConditions: conditions });
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

  setScrollTop: (scrollTop: number) => set({ scrollTop }),
}));
