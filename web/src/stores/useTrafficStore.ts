import { create } from 'zustand';
import type { TrafficSummary, TrafficRecord, ToolbarFilters, FilterCondition, TrafficUpdatesFilter } from '../types';
import * as api from '../api';
import pushService, { type TrafficUpdatesData } from '../services/pushService';

interface TrafficState {
  records: TrafficSummary[];
  recordsMap: Map<string, TrafficSummary>;
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
  filterVersion: number;

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
const BATCH_LIMIT = 1000;
const MAX_RECORDS = 10000;
const UPDATE_THROTTLE_MS = 100;

interface BatchedUpdate {
  newRecords: TrafficSummary[];
  updatedRecords: TrafficSummary[];
  serverTotal: number;
  hasMore: boolean;
}

let pendingBatch: BatchedUpdate | null = null;
let rafId: number | null = null;
let lastUpdateTime = 0;

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

const hasActiveFilters = (toolbar: ToolbarFilters, conditions: FilterCondition[]): boolean => {
  return toolbar.rule.length > 0 ||
    toolbar.protocol.length > 0 ||
    toolbar.status.length > 0 ||
    toolbar.type.length > 0 ||
    conditions.some(c => c.value);
};

interface CompiledCondition {
  field: string;
  operator: string;
  valueLower: string;
  regex: RegExp | null;
}

const compileConditions = (conditions: FilterCondition[]): CompiledCondition[] => {
  return conditions
    .filter(c => c.value)
    .map(c => {
      let regex: RegExp | null = null;
      if (c.operator === 'regex') {
        try {
          regex = new RegExp(c.value, 'i');
        } catch {
          regex = null;
        }
      }
      return {
        field: c.field,
        operator: c.operator,
        valueLower: c.value.toLowerCase(),
        regex,
      };
    });
};

const matchRecord = (
  record: TrafficSummary,
  toolbar: ToolbarFilters,
  compiledConditions: CompiledCondition[],
  protocolSet: Set<string>,
  statusSet: Set<string>,
  typeSet: Set<string>
): boolean => {
  if (toolbar.rule.length > 0 && !record.has_rule_hit) {
    return false;
  }

  if (protocolSet.size > 0) {
    const protocol = record.protocol?.toUpperCase() || '';
    let matched = false;
    if (protocolSet.has('H2') && protocol.includes('HTTP/2')) matched = true;
    else if (protocolSet.has('HTTP') && (protocol === 'HTTP/1.0' || protocol === 'HTTP/1.1')) matched = true;
    else if (protocolSet.has('HTTPS') && protocol === 'HTTPS') matched = true;
    else if (protocolSet.has('WS') && record.is_websocket && protocol === 'WS') matched = true;
    else if (protocolSet.has('WSS') && record.is_websocket && protocol === 'WSS') matched = true;
    else if (protocolSet.has('H3') && (record.is_h3 || protocol === 'H3')) matched = true;
    else if (protocolSet.has('H3S') && (record.is_h3 || protocol === 'H3S' || protocol === 'H3')) matched = true;
    if (!matched) return false;
  }

  if (statusSet.size > 0) {
    const status = record.status;
    let matched = false;
    if (statusSet.has('error') && (status === 0 || status >= 500)) matched = true;
    else if (statusSet.has('1xx') && status >= 100 && status < 200) matched = true;
    else if (statusSet.has('2xx') && status >= 200 && status < 300) matched = true;
    else if (statusSet.has('3xx') && status >= 300 && status < 400) matched = true;
    else if (statusSet.has('4xx') && status >= 400 && status < 500) matched = true;
    else if (statusSet.has('5xx') && status >= 500 && status < 600) matched = true;
    if (!matched) return false;
  }

  if (typeSet.size > 0) {
    const contentType = (record.content_type || '').toLowerCase();
    let matched = false;
    for (const t of typeSet) {
      const patterns = contentTypeMap[t] || [t.toLowerCase()];
      if (patterns.some(pattern => contentType.includes(pattern))) {
        matched = true;
        break;
      }
    }
    if (!matched) return false;
  }

  for (const cond of compiledConditions) {
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

    const fieldValueLower = fieldValue.toLowerCase();
    let matched = false;

    switch (cond.operator) {
      case 'contains':
        matched = fieldValueLower.includes(cond.valueLower);
        break;
      case 'equals':
        matched = fieldValueLower === cond.valueLower;
        break;
      case 'regex':
        matched = cond.regex ? cond.regex.test(fieldValue) : false;
        break;
      case 'not_contains':
        matched = !fieldValueLower.includes(cond.valueLower);
        break;
      default:
        matched = fieldValueLower.includes(cond.valueLower);
    }

    if (!matched) return false;
  }

  return true;
};

export const filterRecords = (
  records: TrafficSummary[],
  toolbar: ToolbarFilters,
  conditions: FilterCondition[]
): TrafficSummary[] => {
  if (!hasActiveFilters(toolbar, conditions)) {
    return records;
  }

  const compiledConditions = compileConditions(conditions);
  const protocolSet = new Set(toolbar.protocol.map(p => p.toUpperCase()));
  const statusSet = new Set(toolbar.status);
  const typeSet = new Set(toolbar.type);

  const result: TrafficSummary[] = [];
  for (const record of records) {
    if (matchRecord(record, toolbar, compiledConditions, protocolSet, statusSet, typeSet)) {
      result.push(record);
    }
  }
  return result;
};

export const useTrafficStore = create<TrafficState>((set, get) => ({
  records: [],
  recordsMap: new Map(),
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
  filterVersion: 0,

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
    if (data.new_records.length === 0 && data.updated_records.length === 0) return;

    if (pendingBatch) {
      pendingBatch.newRecords.push(...data.new_records);
      pendingBatch.updatedRecords.push(...data.updated_records);
      pendingBatch.serverTotal = data.server_total;
      pendingBatch.hasMore = data.has_more;
    } else {
      pendingBatch = {
        newRecords: [...data.new_records],
        updatedRecords: [...data.updated_records],
        serverTotal: data.server_total,
        hasMore: data.has_more,
      };
    }

    const now = performance.now();
    const timeSinceLastUpdate = now - lastUpdateTime;

    if (rafId !== null) {
      return;
    }

    const scheduleUpdate = () => {
      rafId = requestAnimationFrame(() => {
        rafId = null;
        const batch = pendingBatch;
        if (!batch) return;
        pendingBatch = null;
        lastUpdateTime = performance.now();

        set((prevState) => {
          const recordsMap = prevState.recordsMap;
          let hasChanges = false;

          for (const r of batch.updatedRecords) {
            const existing = recordsMap.get(r.id);
            const socketStatusChanged =
              existing?.socket_status?.send_bytes !== r.socket_status?.send_bytes ||
              existing?.socket_status?.receive_bytes !== r.socket_status?.receive_bytes ||
              existing?.socket_status?.is_open !== r.socket_status?.is_open;
            if (!existing || existing.sequence !== r.sequence || existing.status !== r.status || socketStatusChanged) {
              recordsMap.set(r.id, r);
              hasChanges = true;
            }
          }

          for (const r of batch.newRecords) {
            if (!recordsMap.has(r.id)) {
              recordsMap.set(r.id, r);
              hasChanges = true;
            }
          }

          const newPendingIds = prevState.pendingIds;

          for (const r of batch.updatedRecords) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (!isPending) {
              newPendingIds.delete(r.id);
            }
          }

          for (const r of batch.newRecords) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (isPending) {
              newPendingIds.add(r.id);
            }
          }

          let allRecords: TrafficSummary[];
          if (hasChanges) {
            allRecords = Array.from(recordsMap.values());
            allRecords.sort((a, b) => a.sequence - b.sequence);

            if (allRecords.length > MAX_RECORDS) {
              const toRemove = allRecords.slice(0, allRecords.length - MAX_RECORDS);
              for (const r of toRemove) {
                recordsMap.delete(r.id);
                newPendingIds.delete(r.id);
              }
              allRecords = allRecords.slice(allRecords.length - MAX_RECORDS);
            }
          } else {
            allRecords = prevState.records;
          }

          const lastRecord = batch.newRecords[batch.newRecords.length - 1];
          const newLastId = lastRecord?.id || prevState.lastId;

          const newCount = batch.newRecords.length;
          const updatedNewRecordsCount = prevState.autoScroll
            ? 0
            : prevState.newRecordsCount + newCount;

          pushService.updateSubscription({
            last_traffic_id: newLastId || undefined,
            pending_ids: Array.from(newPendingIds),
          });

          let updatedCurrentRecord = prevState.currentRecord;
          if (updatedCurrentRecord) {
            const updatedSummary = batch.updatedRecords.find(r => r.id === updatedCurrentRecord!.id);
            if (updatedSummary && updatedSummary.socket_status) {
              updatedCurrentRecord = {
                ...updatedCurrentRecord,
                socket_status: updatedSummary.socket_status,
                frame_count: updatedSummary.frame_count,
              };
            }
          }

          return {
            records: allRecords,
            recordsMap,
            serverTotal: batch.serverTotal,
            hasMore: batch.hasMore,
            lastId: newLastId,
            pendingIds: newPendingIds,
            newRecordsCount: updatedNewRecordsCount,
            currentRecord: updatedCurrentRecord,
          };
        });
      });
    };

    if (timeSinceLastUpdate >= UPDATE_THROTTLE_MS) {
      scheduleUpdate();
    } else {
      setTimeout(scheduleUpdate, UPDATE_THROTTLE_MS - timeSinceLastUpdate);
    }
  },

  fetchInitialData: async () => {
    set({ loading: true, error: null });
    try {
      const filter: TrafficUpdatesFilter = {
        limit: BATCH_LIMIT,
      };
      const response = await api.getTrafficUpdates(filter);

      const newPendingIds = new Set<string>();
      const newRecordsMap = new Map<string, TrafficSummary>();
      for (const r of response.new_records) {
        newRecordsMap.set(r.id, r);
        if (r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open)) {
          newPendingIds.add(r.id);
        }
      }

      const lastRecord = response.new_records[response.new_records.length - 1];

      set({
        records: response.new_records,
        recordsMap: newRecordsMap,
        serverTotal: response.server_total,
        hasMore: response.has_more,
        lastId: lastRecord?.id || null,
        pendingIds: newPendingIds,
        loading: false,
        filterVersion: 0,
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

      if (response.new_records.length > 0 || response.updated_records.length > 0) {
        set((prevState) => {
          const recordsMap = prevState.recordsMap;
          let hasChanges = false;

          for (const r of response.updated_records) {
            const existing = recordsMap.get(r.id);
            if (!existing || existing.sequence !== r.sequence || existing.status !== r.status) {
              recordsMap.set(r.id, r);
              hasChanges = true;
            }
          }

          for (const r of response.new_records) {
            if (!recordsMap.has(r.id)) {
              recordsMap.set(r.id, r);
              hasChanges = true;
            }
          }

          const newPendingIds = prevState.pendingIds;

          for (const r of response.updated_records) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (!isPending) {
              newPendingIds.delete(r.id);
            }
          }

          for (const r of response.new_records) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (isPending) {
              newPendingIds.add(r.id);
            }
          }

          let allRecords: TrafficSummary[];
          if (hasChanges) {
            allRecords = Array.from(recordsMap.values());
            allRecords.sort((a, b) => a.sequence - b.sequence);

            if (allRecords.length > MAX_RECORDS) {
              const toRemove = allRecords.slice(0, allRecords.length - MAX_RECORDS);
              for (const r of toRemove) {
                recordsMap.delete(r.id);
                newPendingIds.delete(r.id);
              }
              allRecords = allRecords.slice(allRecords.length - MAX_RECORDS);
            }
          } else {
            allRecords = prevState.records;
          }

          const lastRecord = response.new_records[response.new_records.length - 1];
          const newLastId = lastRecord?.id || prevState.lastId;

          const newCount = response.new_records.length;
          const updatedNewRecordsCount = prevState.autoScroll
            ? 0
            : prevState.newRecordsCount + newCount;

          return {
            records: allRecords,
            recordsMap,
            serverTotal: response.server_total,
            hasMore: response.has_more,
            lastId: newLastId,
            pendingIds: newPendingIds,
            newRecordsCount: updatedNewRecordsCount,
          };
        });
      }

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
        recordsMap: new Map(),
        serverTotal: 0,
        hasMore: false,
        lastId: null,
        pendingIds: new Set(),
        currentRecord: null,
        requestBody: null,
        responseBody: null,
        loading: false,
        filterVersion: 0,
      });
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setToolbarFilters: (filters: ToolbarFilters) => {
    set((state) => ({ toolbarFilters: filters, filterVersion: state.filterVersion + 1 }));
  },

  setFilterConditions: (conditions: FilterCondition[]) => {
    set((state) => ({ filterConditions: conditions, filterVersion: state.filterVersion + 1 }));
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
