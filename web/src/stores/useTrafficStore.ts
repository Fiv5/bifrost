import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { TrafficSummary, TrafficRecord, ToolbarFilters, FilterCondition, TrafficUpdatesFilter, TrafficSummaryCompact, TrafficDeltaData } from '../types';
import * as api from '../api';
import pushService, { type TrafficUpdatesData } from '../services/pushService';

interface TrafficState {
  records: TrafficSummary[];
  recordsMap: Map<string, TrafficSummary>;
  currentRecord: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
  serverTotal: number;
  serverSequence: number;
  hasMore: boolean;
  lastId: string | null;
  lastSequence: number | null;
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
  pushDeltaUnsubscribe: (() => void) | null;
  filterVersion: number;
  initialized: boolean;
  selectedId: string | undefined;
  useDbMode: boolean;

  startPolling: () => void;
  stopPolling: () => void;
  fetchUpdates: () => Promise<void>;
  fetchInitialData: () => Promise<void>;
  fetchTrafficDetail: (id: string) => Promise<void>;
  appendSseResponseBody: (recordId: string, payload: string) => void;
  setResponseBody: (recordId: string, body: string | null) => void;
  clearTraffic: (ids?: string[]) => Promise<boolean>;
  setToolbarFilters: (filters: ToolbarFilters) => void;
  setFilterConditions: (conditions: FilterCondition[]) => void;
  setPaused: (paused: boolean) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  clearNewRecordsCount: () => void;
  clearError: () => void;
  clearCurrentRecord: () => void;
  initFromUrl: (filters: FilterCondition[], toolbar: ToolbarFilters | null) => void;
  setScrollTop: (scrollTop: number) => void;
  setSelectedId: (id: string | undefined) => void;
  handleTrafficPush: (data: TrafficUpdatesData) => void;
  handleTrafficDelta: (data: TrafficDeltaData) => void;
  enablePush: () => void;
  disablePush: () => void;
}

const POLL_INTERVAL = 1000;
const POLL_MIN_INTERVAL = 200;
const HAS_MORE_BURST_LIMIT = 3;
const HAS_MORE_BACKOFF_INTERVAL = 500;
const BATCH_LIMIT = 1000;
const MAX_RECORDS = 10000;
const UPDATE_THROTTLE_MS = 100;
const MAX_PENDING_IDS = 500;

interface BatchedUpdate {
  newRecords: TrafficSummary[];
  updatedRecords: TrafficSummary[];
  serverTotal: number;
  hasMore: boolean;
}

let pendingBatch: BatchedUpdate | null = null;
let rafId: number | null = null;
let lastUpdateTime = 0;
let hasMoreBurst = 0;

function capPendingIds(ids: Set<string>) {
  while (ids.size > MAX_PENDING_IDS) {
    const first = ids.values().next().value as string | undefined;
    if (!first) break;
    ids.delete(first);
  }
}

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

const METHOD_COLORS: Record<string, string> = {
  GET: "green",
  POST: "blue",
  PUT: "orange",
  DELETE: "red",
  PATCH: "purple",
  OPTIONS: "default",
  HEAD: "cyan",
  CONNECT: "magenta",
};

const STATUS_DOT_COLORS: Record<string, string> = {
  pending: "#d9d9d9",
  info: "#73d13d",
  success: "#52c41a",
  redirect: "#faad14",
  clientError: "#fa8c16",
  serverError: "#f5222d",
};

const getStatusDotColor = (status: number): string => {
  if (status === 0) return STATUS_DOT_COLORS.pending;
  if (status >= 100 && status < 200) return STATUS_DOT_COLORS.info;
  if (status >= 200 && status < 300) return STATUS_DOT_COLORS.success;
  if (status >= 300 && status < 400) return STATUS_DOT_COLORS.redirect;
  if (status >= 400 && status < 500) return STATUS_DOT_COLORS.clientError;
  if (status >= 500) return STATUS_DOT_COLORS.serverError;
  return STATUS_DOT_COLORS.pending;
};

const getStatusColor = (status: number): string => {
  if (status >= 500) return "error";
  if (status >= 400) return "warning";
  if (status >= 300) return "processing";
  if (status >= 200) return "success";
  return "default";
};

const formatSize = (bytes: number): string => {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
};

const mergeSseBody = (prev: string | null, payload: string): string => {
  const trimmed = payload.replace(/\n+$/, '');
  if (!trimmed) return prev || '';
  if (!prev || prev.length === 0) return trimmed;
  if (prev.endsWith('\n\n')) return `${prev}${trimmed}`;
  if (prev.endsWith('\n')) return `${prev}\n${trimmed}`;
  return `${prev}\n\n${trimmed}`;
};

const preprocessRecord = (record: TrafficSummary): TrafficSummary => {
  const isH3 = record.is_h3 || record.protocol === 'h3' || record.protocol === 'h3s';
  const displayProtocol = isH3
    ? 'H3'
    : record.protocol?.replace("HTTP/", "").toUpperCase() || "-";

  const methodColor = METHOD_COLORS[record.method?.toUpperCase()] || "default";
  const statusColor = getStatusColor(record.status);
  const statusDotColor = getStatusDotColor(record.status);

  const size = (record.is_websocket || record.is_sse || record.is_tunnel) && record.socket_status
    ? record.socket_status.send_bytes + record.socket_status.receive_bytes
    : record.response_size;
  const displaySize = formatSize(size);

  const contentTypeShort = record.content_type?.split(";")[0]?.split("/").pop() || "-";

  const clientApp = record.client_app || "";
  const clientIp = record.client_ip || "";
  const hasApp = Boolean(clientApp);
  const clientDisplay = clientApp || clientIp || "-";
  const clientTooltip = hasApp
    ? `${clientApp} (PID: ${record.client_pid || "?"}, IP: ${clientIp || "?"})`
    : clientIp || "-";

  record._displayProtocol = displayProtocol;
  record._methodColor = methodColor;
  record._statusColor = statusColor;
  record._statusDotColor = statusDotColor;
  record._displaySize = displaySize;
  record._contentTypeShort = contentTypeShort;
  record._clientDisplay = clientDisplay;
  record._clientTooltip = clientTooltip;

  return record;
};

const preprocessRecords = (records: TrafficSummary[]): TrafficSummary[] => {
  for (let i = 0; i < records.length; i++) {
    preprocessRecord(records[i]);
  }
  return records;
};

const applyDisplayIndex = (records: TrafficSummary[]): void => {
  for (let i = 0; i < records.length; i++) {
    records[i]._displayIndex = i + 1;
  }
};

const hasActiveFilters = (toolbar: ToolbarFilters, conditions: FilterCondition[]): boolean => {
  return toolbar.rule.length > 0 ||
    toolbar.protocol.length > 0 ||
    toolbar.status.length > 0 ||
    toolbar.type.length > 0 ||
    toolbar.imported.length > 0 ||
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

  if (toolbar.imported.length > 0) {
    const isImported = record.id.startsWith('OUT-') || record.client_app === 'Bifrost Import';
    if (!isImported) {
      return false;
    }
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
    const resContentType = (record.content_type || '').toLowerCase();
    const reqContentType = (record.request_content_type || '').toLowerCase();
    let matched = false;
    for (const t of typeSet) {
      const patterns = contentTypeMap[t] || [t.toLowerCase()];
      if (patterns.some(pattern => resContentType.includes(pattern) || reqContentType.includes(pattern))) {
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

export interface PanelFilters {
  clientIps: string[];
  clientApps: string[];
  domains: string[];
}

const hasPanelFilters = (panel: PanelFilters): boolean => {
  return panel.clientIps.length > 0 || panel.clientApps.length > 0 || panel.domains.length > 0;
};

const matchPanelFilters = (record: TrafficSummary, panel: PanelFilters): boolean => {
  const clientIpMatch = panel.clientIps.length === 0
    || panel.clientIps.includes(record.client_ip || '');

  const clientAppMatch = panel.clientApps.length === 0
    || panel.clientApps.includes(record.client_app || '');

  const domainMatch = panel.domains.length === 0
    || panel.domains.some(domain => (record.host || '').includes(domain));

  return clientIpMatch && clientAppMatch && domainMatch;
};

export const filterRecords = (
  records: TrafficSummary[],
  toolbar: ToolbarFilters,
  conditions: FilterCondition[],
  panel: PanelFilters = { clientIps: [], clientApps: [], domains: [] }
): TrafficSummary[] => {
  const hasToolbarOrConditions = hasActiveFilters(toolbar, conditions);
  const hasPanelActive = hasPanelFilters(panel);

  if (!hasToolbarOrConditions && !hasPanelActive) {
    return records;
  }

  const compiledConditions = compileConditions(conditions);
  const protocolSet = new Set(toolbar.protocol.map(p => p.toUpperCase()));
  const statusSet = new Set(toolbar.status);
  const typeSet = new Set(toolbar.type);

  const result: TrafficSummary[] = [];
  for (const record of records) {
    const toolbarMatch = !hasToolbarOrConditions || matchRecord(record, toolbar, compiledConditions, protocolSet, statusSet, typeSet);
    const panelMatch = !hasPanelActive || matchPanelFilters(record, panel);

    if (toolbarMatch && panelMatch) {
      result.push(record);
    }
  }
  return result;
};

const compactToSummary = (c: TrafficSummaryCompact): TrafficSummary => {
  const FLAGS = { IS_TUNNEL: 1, IS_WEBSOCKET: 2, IS_SSE: 4, IS_H3: 8, HAS_RULE_HIT: 16 };
  return {
    id: c.id,
    sequence: c.seq,
    timestamp: c.ts,
    method: c.m,
    host: c.h,
    path: c.p,
    status: c.s,
    content_type: c.ct || null,
    request_content_type: c.req_ct || null,
    request_size: c.req_sz,
    response_size: c.res_sz,
    duration_ms: c.dur,
    protocol: c.proto,
    client_ip: c.cip,
    client_app: c.capp || undefined,
    client_pid: c.cpid || undefined,
    is_tunnel: (c.flags & FLAGS.IS_TUNNEL) !== 0,
    is_websocket: (c.flags & FLAGS.IS_WEBSOCKET) !== 0,
    is_sse: (c.flags & FLAGS.IS_SSE) !== 0,
    is_h3: (c.flags & FLAGS.IS_H3) !== 0,
    has_rule_hit: (c.flags & FLAGS.HAS_RULE_HIT) !== 0,
    matched_rule_count: c.rc || 0,
    matched_protocols: c.rp || [],
    frame_count: c.fc,
    socket_status: c.ss || undefined,
    url: `${c.proto === 'https' ? 'https' : 'http'}://${c.h}${c.p}`,
    start_time: c.st,
    end_time: c.et || undefined,
  };
};

export const useTrafficStore = create<TrafficState>()(
  persist(
    (set, get) => ({
      records: [],
      recordsMap: new Map(),
      currentRecord: null,
      requestBody: null,
      responseBody: null,
      serverTotal: 0,
      serverSequence: 0,
      hasMore: false,
      lastId: null,
      lastSequence: null,
      pendingIds: new Set(),
      toolbarFilters: { rule: [], protocol: [], type: [], status: [], imported: [] },
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
      pushDeltaUnsubscribe: null,
      filterVersion: 0,
      initialized: false,
      selectedId: undefined,
      useDbMode: true,

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
        hasMoreBurst = 0;
        if (state.usePush) {
          get().disablePush();
        }
        set({ polling: false, pollTimeoutId: null });
      },

      enablePush: () => {
        const state = get();
        if (state.pushUnsubscribe || state.pushDeltaUnsubscribe) return;

        const subscription = {
          last_traffic_id: state.lastId || undefined,
          last_sequence: state.lastSequence || undefined,
          pending_ids: Array.from(state.pendingIds),
        };

        pushService.connect(subscription);

        const unsubscribe = pushService.onTrafficUpdates((data) => {
          get().handleTrafficPush(data);
        });

        const unsubscribeDelta = pushService.onTrafficDelta((data) => {
          get().handleTrafficDelta(data);
        });

        set({ pushUnsubscribe: unsubscribe, pushDeltaUnsubscribe: unsubscribeDelta });
      },

      disablePush: () => {
        const state = get();
        if (state.pushUnsubscribe) {
          state.pushUnsubscribe();
        }
        if (state.pushDeltaUnsubscribe) {
          state.pushDeltaUnsubscribe();
        }
        set({ pushUnsubscribe: null, pushDeltaUnsubscribe: null });
        pushService.disconnectIfIdle();
      },

      handleTrafficPush: (data: TrafficUpdatesData) => {
        const state = get();
        if (state.paused) return;
        if (data.new_records.length === 0 && data.updated_records.length === 0) return;

        const preprocessedNew = preprocessRecords(data.new_records);
        const preprocessedUpdated = preprocessRecords(data.updated_records);

        if (pendingBatch) {
          pendingBatch.newRecords.push(...preprocessedNew);
          pendingBatch.updatedRecords.push(...preprocessedUpdated);
          pendingBatch.serverTotal = data.server_total;
          pendingBatch.hasMore = data.has_more;
        } else {
          pendingBatch = {
            newRecords: [...preprocessedNew],
            updatedRecords: [...preprocessedUpdated],
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
                if (!existing || existing.status !== r.status || socketStatusChanged) {
                  recordsMap.set(r.id, r);
                  hasChanges = true;
                }
              }

              let actualNewCount = 0;
              for (const r of batch.newRecords) {
                if (!recordsMap.has(r.id)) {
                  recordsMap.set(r.id, r);
                  hasChanges = true;
                  actualNewCount++;
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
              capPendingIds(newPendingIds);

              let allRecords: TrafficSummary[];
              if (hasChanges) {
                allRecords = Array.from(recordsMap.values());
                applyDisplayIndex(allRecords);

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

              const updatedNewRecordsCount = prevState.autoScroll
                ? 0
                : prevState.newRecordsCount + actualNewCount;

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

      handleTrafficDelta: (data: TrafficDeltaData) => {
        const state = get();
        if (state.paused) return;
        if (data.inserts.length === 0 && data.updates.length === 0) return;

        const newRecords = data.inserts.map(c => preprocessRecord(compactToSummary(c)));
        const updatedRecords = data.updates.map(c => preprocessRecord(compactToSummary(c)));

        set((prevState) => {
          const recordsMap = prevState.recordsMap;
          let hasChanges = false;

          for (const r of updatedRecords) {
            const existing = recordsMap.get(r.id);
            const socketStatusChanged =
              existing?.socket_status?.send_bytes !== r.socket_status?.send_bytes ||
              existing?.socket_status?.receive_bytes !== r.socket_status?.receive_bytes ||
              existing?.socket_status?.is_open !== r.socket_status?.is_open;
            if (!existing || existing.status !== r.status || socketStatusChanged) {
              recordsMap.set(r.id, r);
              hasChanges = true;
            }
          }

          let actualNewCount = 0;
          for (const r of newRecords) {
            if (!recordsMap.has(r.id)) {
              recordsMap.set(r.id, r);
              hasChanges = true;
              actualNewCount++;
            }
          }

          const newPendingIds = prevState.pendingIds;

          for (const r of updatedRecords) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (!isPending) {
              newPendingIds.delete(r.id);
            }
          }

          for (const r of newRecords) {
            const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
            if (isPending) {
              newPendingIds.add(r.id);
            }
          }
          capPendingIds(newPendingIds);

          let allRecords: TrafficSummary[];
          if (hasChanges) {
            allRecords = Array.from(recordsMap.values());
            applyDisplayIndex(allRecords);
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

          const lastRecord = newRecords[newRecords.length - 1];
          const newLastId = lastRecord?.id || prevState.lastId;
          const newLastSeq = lastRecord?.sequence || prevState.lastSequence;

          const updatedNewRecordsCount = prevState.autoScroll
            ? 0
            : prevState.newRecordsCount + actualNewCount;

          pushService.updateSubscription({
            last_sequence: newLastSeq || undefined,
            pending_ids: Array.from(newPendingIds),
          });

          let updatedCurrentRecord = prevState.currentRecord;
          if (updatedCurrentRecord) {
            const updatedSummary = updatedRecords.find(r => r.id === updatedCurrentRecord!.id);
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
            serverTotal: data.server_total,
            serverSequence: data.server_sequence,
            hasMore: data.has_more,
            lastId: newLastId,
            lastSequence: newLastSeq,
            pendingIds: newPendingIds,
            newRecordsCount: updatedNewRecordsCount,
            currentRecord: updatedCurrentRecord,
          };
        });
      },

      fetchInitialData: async () => {
        const state = get();
        if (state.initialized && state.records.length > 0) {
          return;
        }

        set({ loading: true, error: null });
        try {
          const filter: TrafficUpdatesFilter = {
            limit: BATCH_LIMIT,
          };
          const response = await api.getTrafficUpdates(filter);

          const convertedRecords = response.new_records.map(compactToSummary);
          const preprocessedRecords = preprocessRecords(convertedRecords);
          applyDisplayIndex(preprocessedRecords);

          const newPendingIds = new Set<string>();
          const newRecordsMap = new Map<string, TrafficSummary>();
          for (const r of preprocessedRecords) {
            newRecordsMap.set(r.id, r);
            if (r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open)) {
              newPendingIds.add(r.id);
            }
          }
          capPendingIds(newPendingIds);

          const lastRecord = preprocessedRecords[preprocessedRecords.length - 1];

          set({
            records: preprocessedRecords,
            recordsMap: newRecordsMap,
            serverTotal: response.server_total,
            serverSequence: response.server_sequence,
            hasMore: response.has_more,
            lastId: lastRecord?.id || null,
            lastSequence: lastRecord?.sequence || null,
            pendingIds: newPendingIds,
            loading: false,
            filterVersion: 0,
            initialized: true,
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
            const convertedNew = response.new_records.map(compactToSummary);
            const convertedUpdated = response.updated_records.map(compactToSummary);
            const preprocessedNew = preprocessRecords(convertedNew);
            const preprocessedUpdated = preprocessRecords(convertedUpdated);

            set((prevState) => {
              const recordsMap = prevState.recordsMap;
              let hasChanges = false;

              for (const r of preprocessedUpdated) {
                const existing = recordsMap.get(r.id);
                if (!existing || existing.status !== r.status) {
                  recordsMap.set(r.id, r);
                  hasChanges = true;
                }
              }

              let actualNewCount = 0;
              for (const r of preprocessedNew) {
                if (!recordsMap.has(r.id)) {
                  recordsMap.set(r.id, r);
                  hasChanges = true;
                  actualNewCount++;
                }
              }

              const newPendingIds = prevState.pendingIds;

              for (const r of preprocessedUpdated) {
                const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
                if (!isPending) {
                  newPendingIds.delete(r.id);
                }
              }

              for (const r of preprocessedNew) {
                const isPending = r.status === 0 || ((r.is_websocket || r.is_sse || r.is_tunnel) && r.socket_status?.is_open);
                if (isPending) {
                  newPendingIds.add(r.id);
                }
              }
              capPendingIds(newPendingIds);

              let allRecords: TrafficSummary[];
              if (hasChanges) {
                allRecords = Array.from(recordsMap.values());
                applyDisplayIndex(allRecords);

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

              const lastRecord = preprocessedNew[preprocessedNew.length - 1];
              const newLastId = lastRecord?.id || prevState.lastId;

              const updatedNewRecordsCount = prevState.autoScroll
                ? 0
                : prevState.newRecordsCount + actualNewCount;

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
            if (response.has_more) {
              hasMoreBurst += 1;
            } else {
              hasMoreBurst = 0;
            }
            const nextDelay = response.has_more
              ? (hasMoreBurst > HAS_MORE_BURST_LIMIT ? HAS_MORE_BACKOFF_INTERVAL : POLL_MIN_INTERVAL)
              : POLL_INTERVAL;
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

          const isOpenSse = !!record.is_sse && !!record.socket_status?.is_open;
          if (!isOpenSse) {
            api.getResponseBody(id).then(body => {
              set({ responseBody: body });
            }).catch(() => { });
          }
        } catch (e) {
          set({ error: (e as Error).message, detailLoading: false });
        }
      },

      appendSseResponseBody: (recordId: string, payload: string) => {
        set((state) => {
          if (!payload) return {};
          if (state.currentRecord?.id !== recordId) return {};
          return { responseBody: mergeSseBody(state.responseBody, payload) };
        });
      },

      setResponseBody: (recordId: string, body: string | null) => {
        set((state) => {
          if (state.currentRecord?.id !== recordId) return {};
          return { responseBody: body };
        });
      },

      clearTraffic: async (ids?: string[]) => {
        set({ loading: true, error: null });
        try {
          await api.clearTraffic(ids);

          if (ids && ids.length > 0) {
            const idsToRemove = new Set(ids);
            set((state) => {
              const newRecordsMap = new Map(state.recordsMap);
              const newPendingIds = new Set(state.pendingIds);

              for (const id of idsToRemove) {
                newRecordsMap.delete(id);
                newPendingIds.delete(id);
              }

              const newRecords = Array.from(newRecordsMap.values());
              applyDisplayIndex(newRecords);
              return {
                records: newRecords,
                recordsMap: newRecordsMap,
                pendingIds: newPendingIds,
                loading: false,
                filterVersion: state.filterVersion + 1,
              };
            });
          } else {
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
              initialized: false,
              selectedId: undefined,
            });
          }
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
          toolbarFilters: toolbar || { rule: [], protocol: [], type: [], status: [], imported: [] },
        });
      },

      setScrollTop: (scrollTop: number) => set({ scrollTop }),

      setSelectedId: (id: string | undefined) => set({ selectedId: id }),
    }),
    {
      name: 'bifrost-traffic-ui',
      partialize: (state) => ({
        toolbarFilters: state.toolbarFilters,
        filterConditions: state.filterConditions,
        autoScroll: state.autoScroll,
        scrollTop: state.scrollTop,
        selectedId: state.selectedId,
        useDbMode: state.useDbMode,
      }),
      version: 1,
    },
  ),
);
