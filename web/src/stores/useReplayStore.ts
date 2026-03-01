import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { message } from 'antd';
import type {
  ReplayGroup,
  ReplayRequest,
  ReplayRequestSummary,
  ReplayHistory,
  ReplayExecuteResponse,
  RuleConfig,
  ReplayKeyValueItem,
  ReplayBody,
  TrafficRecord,
  SSEEvent,
  WebSocketMessage,
  StreamingConnection,
} from '../types';
import * as replayApi from '../api/replay';
import * as trafficApi from '../api/traffic';
import { pushService } from '../services/pushService';

import type { RequestType } from '../types';

export type RequestPanelTab = 'params' | 'headers' | 'body' | 'history';
export type ResponsePanelTab = 'body' | 'cookies' | 'headers' | 'rules' | 'messages';
export type ResponseViewMode = 'pretty' | 'raw' | 'preview';
export type ResponseContentType = 'json' | 'xml' | 'html' | 'javascript' | 'css' | 'text';
export type ReplayMode = 'composer' | 'history';

interface ReplayUIState {
  mode: ReplayMode;
  requestType: RequestType;
  requestPanelActiveTab: RequestPanelTab;
  responsePanelActiveTab: ResponsePanelTab;
  responseViewMode: ResponseViewMode;
  responseContentType: ResponseContentType | null;
  saveModalVisible: boolean;
  saveName: string;
  ruleSelectVisible: boolean;
  collectionPanelSection: 'collections' | 'history';
  collectionSearchText: string;
  collectionExpandedKeys: string[];
  historySearchText: string;
  selectedHistoryId: string | null;
  wsMessageInput: string;
  selectedRequestId: string | null;
}

interface ReplayState {
  currentRequest: ReplayRequest | null;
  savedRequests: ReplayRequestSummary[];
  recentHistory: ReplayHistory[];
  allHistory: ReplayHistory[];
  groups: ReplayGroup[];

  currentResponse: ReplayExecuteResponse | null;
  currentTrafficRecord: TrafficRecord | null;

  selectedHistoryRecord: TrafficRecord | null;
  historyRequestBody: string | null;
  historyResponseBody: string | null;
  historyDetailLoading: boolean;

  ruleConfig: RuleConfig;

  loading: boolean;
  executing: boolean;
  responsePanelCollapsed: boolean;

  requestsTotal: number;
  historyTotal: number;
  allHistoryTotal: number;

  streamingConnection: StreamingConnection | null;
  sseEvents: SSEEvent[];
  wsMessages: WebSocketMessage[];
  eventSourceRef: EventSource | null;
  webSocketRef: WebSocket | null;

  uiState: ReplayUIState;

  createNewRequest: () => void;
  updateCurrentRequest: (updates: Partial<ReplayRequest>) => void;
  saveRequest: (name?: string, groupId?: string) => Promise<boolean>;
  executeRequest: () => Promise<void>;
  loadSavedRequests: () => Promise<void>;
  loadRecentHistory: (requestId?: string) => Promise<void>;
  loadAllHistory: (requestId: string) => Promise<void>;
  loadGroups: () => Promise<void>;
  createGroup: (name: string) => Promise<boolean>;
  deleteGroup: (id: string) => Promise<boolean>;
  updateGroup: (id: string, name: string) => Promise<boolean>;
  moveRequest: (requestId: string, groupId: string | null) => Promise<boolean>;
  deleteRequest: (id: string) => Promise<boolean>;
  deleteHistory: (id: string) => Promise<boolean>;
  clearHistory: (requestId?: string) => Promise<boolean>;
  setRuleConfig: (config: RuleConfig) => void;
  selectRequest: (request: ReplayRequestSummary | ReplayRequest) => Promise<void>;
  selectHistory: (history: ReplayHistory) => Promise<void>;
  selectHistoryForDetail: (history: ReplayHistory) => Promise<void>;
  importFromTraffic: (trafficId: string) => Promise<void>;
  setResponsePanelCollapsed: (collapsed: boolean) => void;
  updateUIState: (updates: Partial<ReplayUIState>) => void;
  connectSSE: () => void;
  disconnectSSE: () => void;
  connectWebSocket: () => void;
  disconnectWebSocket: () => void;
  sendWebSocketMessage: (data: string, type?: 'text' | 'binary') => void;
  clearStreamingMessages: () => void;
  reset: () => void;
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

const defaultUIState: ReplayUIState = {
  mode: 'composer',
  requestType: 'http',
  requestPanelActiveTab: 'params',
  responsePanelActiveTab: 'body',
  responseViewMode: 'pretty',
  responseContentType: null,
  saveModalVisible: false,
  saveName: '',
  ruleSelectVisible: false,
  collectionPanelSection: 'collections',
  collectionSearchText: '',
  collectionExpandedKeys: ['saved', 'ungrouped'],
  historySearchText: '',
  selectedHistoryId: null,
  wsMessageInput: '',
  selectedRequestId: null,
};

function createEmptyRequest(requestType: RequestType = 'http'): ReplayRequest {
  const now = Date.now();
  return {
    id: generateId(),
    request_type: requestType,
    method: 'GET',
    url: '',
    headers: [
      { id: generateId(), key: 'Content-Type', value: 'application/json', enabled: true },
      { id: generateId(), key: 'Accept', value: '*/*', enabled: true },
    ],
    body: { type: 'none' },
    is_saved: false,
    sort_order: 0,
    created_at: now,
    updated_at: now,
  };
}

export const useReplayStore = create<ReplayState>()(
  persist(
    (set, get) => ({
  currentRequest: createEmptyRequest(),
  savedRequests: [],
  recentHistory: [],
  allHistory: [],
  groups: [],
  currentResponse: null,
  currentTrafficRecord: null,
  selectedHistoryRecord: null,
  historyRequestBody: null,
  historyResponseBody: null,
  historyDetailLoading: false,
  ruleConfig: { mode: 'enabled' },
  loading: false,
  executing: false,
  responsePanelCollapsed: true,
  requestsTotal: 0,
  historyTotal: 0,
  allHistoryTotal: 0,
  streamingConnection: null,
  sseEvents: [],
  wsMessages: [],
  eventSourceRef: null,
  webSocketRef: null,
  uiState: { ...defaultUIState },

  createNewRequest: () => {
    set({
      currentRequest: createEmptyRequest(),
      currentResponse: null,
      currentTrafficRecord: null,
      recentHistory: [],
    });
  },

  updateCurrentRequest: (updates) => {
    const { currentRequest } = get();
    if (!currentRequest) return;

    set({
      currentRequest: {
        ...currentRequest,
        ...updates,
        updated_at: Date.now(),
      },
    });
  },

  saveRequest: async (name?: string, groupId?: string) => {
    const { currentRequest, loadSavedRequests } = get();
    if (!currentRequest) return false;

    try {
      set({ loading: true });

      if (currentRequest.is_saved) {
        await replayApi.updateRequest(currentRequest.id, {
          name: name || currentRequest.name,
          method: currentRequest.method,
          url: currentRequest.url,
          headers: currentRequest.headers,
          body: currentRequest.body,
          group_id: groupId !== undefined ? groupId : currentRequest.group_id,
        });
      } else {
        const saved = await replayApi.createRequest({
          name: name || `Request ${Date.now()}`,
          method: currentRequest.method,
          url: currentRequest.url,
          headers: currentRequest.headers,
          body: currentRequest.body,
          is_saved: true,
          group_id: groupId,
        });
        set({ currentRequest: saved });
      }

      await loadSavedRequests();
      message.success('Request saved');
      return true;
    } catch (e) {
      message.error(`Failed to save: ${e}`);
      return false;
    } finally {
      set({ loading: false });
    }
  },

  executeRequest: async () => {
    const { currentRequest, ruleConfig, loadRecentHistory } = get();
    if (!currentRequest || !currentRequest.url) {
      message.warning('Please enter a URL');
      return;
    }

    try {
      set({ executing: true, responsePanelCollapsed: false });

      let bodyContent: string | undefined;
      if (currentRequest.body) {
        if (currentRequest.body.type === 'raw' && currentRequest.body.content) {
          bodyContent = currentRequest.body.content;
        } else if (currentRequest.body.type === 'x-www-form-urlencoded' && currentRequest.body.form_data) {
          const params = new URLSearchParams();
          currentRequest.body.form_data
            .filter(item => item.enabled && item.key)
            .forEach(item => params.append(item.key, item.value));
          bodyContent = params.toString();
        } else if (currentRequest.body.type === 'form-data' && currentRequest.body.form_data) {
          const formData: Record<string, string> = {};
          currentRequest.body.form_data
            .filter(item => item.enabled && item.key)
            .forEach(item => { formData[item.key] = item.value; });
          bodyContent = JSON.stringify(formData);
        }
      }

      const executeReq = replayApi.buildReplayExecuteRequest(
        currentRequest.method,
        currentRequest.url,
        currentRequest.headers,
        bodyContent,
        ruleConfig,
        currentRequest.is_saved ? currentRequest.id : undefined
      );

      const response = await replayApi.executeReplay(executeReq);
      set({ currentResponse: response });

      if (response.traffic_id) {
        try {
          const trafficRecord = await trafficApi.getTrafficDetail(response.traffic_id);
          set({ currentTrafficRecord: trafficRecord });
        } catch {
        }
      }

      if (currentRequest.is_saved) {
        await loadRecentHistory(currentRequest.id);
      }

      message.success(`Request completed: ${response.status}`);
    } catch (e) {
      message.error(`Request failed: ${e}`);
      set({
        currentResponse: {
          traffic_id: '',
          status: 0,
          headers: [],
          duration_ms: 0,
          applied_rules: [],
          error: String(e),
        },
      });
    } finally {
      set({ executing: false });
    }
  },

  loadSavedRequests: async () => {
    try {
      set({ loading: true });
      const response = await replayApi.listRequests({ saved: true, limit: 100 });
      set({
        savedRequests: response.requests,
        requestsTotal: response.total,
      });
    } catch (e) {
      message.error(`Failed to load requests: ${e}`);
    } finally {
      set({ loading: false });
    }
  },

  loadRecentHistory: async (requestId?: string) => {
    try {
      const response = await replayApi.listHistory({
        request_id: requestId,
        limit: 50,
      });
      set({
        recentHistory: response.history,
        historyTotal: response.total,
      });
    } catch (e) {
      message.error(`Failed to load history: ${e}`);
    }
  },

  loadAllHistory: async (requestId: string) => {
    try {
      set({ loading: true });
      const response = await replayApi.listHistory({ request_id: requestId, limit: 500 });
      set({
        allHistory: response.history,
        allHistoryTotal: response.total,
      });
    } catch (e) {
      message.error(`Failed to load history: ${e}`);
    } finally {
      set({ loading: false });
    }
  },

  loadGroups: async () => {
    try {
      const groups = await replayApi.listGroups();
      set({ groups });
    } catch (e) {
      message.error(`Failed to load groups: ${e}`);
    }
  },

  createGroup: async (name: string) => {
    try {
      const group = await replayApi.createGroup(name);
      const { groups } = get();
      set({ groups: [...groups, group] });
      message.success('Folder created');
      return true;
    } catch (e) {
      message.error(`Failed to create folder: ${e}`);
      return false;
    }
  },

  deleteGroup: async (id: string) => {
    try {
      await replayApi.deleteGroup(id);
      const { groups, loadSavedRequests } = get();
      set({ groups: groups.filter(g => g.id !== id) });
      await loadSavedRequests();
      message.success('Folder deleted');
      return true;
    } catch (e) {
      message.error(`Failed to delete folder: ${e}`);
      return false;
    }
  },

  updateGroup: async (id: string, name: string) => {
    try {
      await replayApi.updateGroup(id, { name });
      const { groups } = get();
      set({
        groups: groups.map(g => g.id === id ? { ...g, name } : g),
      });
      message.success('Folder renamed');
      return true;
    } catch (e) {
      message.error(`Failed to rename folder: ${e}`);
      return false;
    }
  },

  moveRequest: async (requestId: string, groupId: string | null) => {
    try {
      await replayApi.moveRequest(requestId, groupId ?? undefined);
      const { loadSavedRequests, currentRequest } = get();
      await loadSavedRequests();
      if (currentRequest?.id === requestId) {
        set({
          currentRequest: {
            ...currentRequest,
            group_id: groupId ?? undefined,
          },
        });
      }
      message.success('Request moved');
      return true;
    } catch (e) {
      message.error(`Failed to move request: ${e}`);
      return false;
    }
  },

  deleteRequest: async (id: string) => {
    try {
      await replayApi.deleteRequest(id);
      const { savedRequests, currentRequest, loadSavedRequests } = get();

      if (currentRequest?.id === id) {
        set({
          currentRequest: createEmptyRequest(),
          currentResponse: null,
          currentTrafficRecord: null,
          recentHistory: [],
        });
      }

      set({
        savedRequests: savedRequests.filter(r => r.id !== id),
      });

      await loadSavedRequests();
      message.success('Request deleted');
      return true;
    } catch (e) {
      message.error(`Failed to delete: ${e}`);
      return false;
    }
  },

  deleteHistory: async (id: string) => {
    try {
      await replayApi.deleteHistory(id);
      const { recentHistory } = get();
      set({
        recentHistory: recentHistory.filter(h => h.id !== id),
      });
      message.success('History deleted');
      return true;
    } catch (e) {
      message.error(`Failed to delete: ${e}`);
      return false;
    }
  },

  clearHistory: async (requestId?: string) => {
    try {
      const result = await replayApi.clearHistory(requestId);
      if (result.success) {
        set({ recentHistory: [] });
        message.success(`Deleted ${result.deleted} history records`);
        return true;
      }
      return false;
    } catch (e) {
      message.error(`Failed to clear: ${e}`);
      return false;
    }
  },

  setRuleConfig: (config) => {
    set({ ruleConfig: config });
  },

  selectRequest: async (request) => {
    try {
      set({ loading: true });
      const fullRequest = await replayApi.getRequest(request.id);
      const { uiState } = get();
      set({
        currentRequest: fullRequest,
        currentResponse: null,
        currentTrafficRecord: null,
        uiState: { ...uiState, selectedRequestId: fullRequest.is_saved ? fullRequest.id : null },
      });

      if (fullRequest.is_saved) {
        const { loadRecentHistory } = get();
        await loadRecentHistory(fullRequest.id);
      }
    } catch (e) {
      message.error(`Failed to load request: ${e}`);
    } finally {
      set({ loading: false });
    }
  },

  selectHistory: async (history) => {
    try {
      if (history.traffic_id) {
        const trafficRecord = await trafficApi.getTrafficDetail(history.traffic_id);
        set({
          currentTrafficRecord: trafficRecord,
          currentResponse: {
            traffic_id: history.traffic_id,
            status: history.status,
            headers: trafficRecord.response_headers || [],
            body: trafficRecord.response_body || undefined,
            duration_ms: history.duration_ms,
            applied_rules: trafficRecord.matched_rules || [],
          },
          responsePanelCollapsed: false,
        });
      }
    } catch (e) {
      message.error(`Failed to load history detail: ${e}`);
    }
  },

  selectHistoryForDetail: async (history) => {
    const { uiState } = get();
    set({
      uiState: { ...uiState, selectedHistoryId: history.id },
      historyDetailLoading: true,
      selectedHistoryRecord: null,
      historyRequestBody: null,
      historyResponseBody: null,
    });

    try {
      if (history.traffic_id) {
        const trafficRecord = await trafficApi.getTrafficDetail(history.traffic_id);
        let requestBody: string | null = null;
        let responseBody: string | null = null;

        try {
          requestBody = await trafficApi.getRequestBody(history.traffic_id);
        } catch { }

        try {
          responseBody = await trafficApi.getResponseBody(history.traffic_id);
        } catch { }

        set({
          selectedHistoryRecord: trafficRecord,
          historyRequestBody: requestBody,
          historyResponseBody: responseBody,
        });
      }
    } catch (e) {
      message.error(`Failed to load history detail: ${e}`);
    } finally {
      set({ historyDetailLoading: false });
    }
  },

  importFromTraffic: async (trafficId: string) => {
    try {
      set({ loading: true });
      const record = await trafficApi.getTrafficDetail(trafficId);
      let requestBody: string | null = null;

      try {
        requestBody = await trafficApi.getRequestBody(trafficId);
      } catch {
      }

      const headers: ReplayKeyValueItem[] = (record.request_headers || []).map(([key, value]) => ({
        id: generateId(),
        key,
        value,
        enabled: true,
      }));

      let body: ReplayBody = { type: 'none' };
      if (requestBody) {
        const contentType = record.request_content_type || '';
        if (contentType.includes('json')) {
          body = { type: 'raw', raw_type: 'json', content: requestBody };
        } else if (contentType.includes('xml')) {
          body = { type: 'raw', raw_type: 'xml', content: requestBody };
        } else if (contentType.includes('form-urlencoded')) {
          const params = new URLSearchParams(requestBody);
          const formData: ReplayKeyValueItem[] = [];
          params.forEach((value, key) => {
            formData.push({ id: generateId(), key, value, enabled: true });
          });
          body = { type: 'x-www-form-urlencoded', form_data: formData };
        } else {
          body = { type: 'raw', raw_type: 'text', content: requestBody };
        }
      }

      const now = Date.now();
      const newRequest: ReplayRequest = {
        id: generateId(),
        request_type: 'http',
        method: record.method,
        url: record.url,
        headers,
        body,
        is_saved: false,
        sort_order: 0,
        created_at: now,
        updated_at: now,
      };

      set({
        currentRequest: newRequest,
        currentResponse: null,
        currentTrafficRecord: null,
        responsePanelCollapsed: true,
        recentHistory: [],
      });

      message.success('Request imported from traffic');
    } catch (e) {
      message.error(`Failed to import: ${e}`);
    } finally {
      set({ loading: false });
    }
  },

  setResponsePanelCollapsed: (collapsed) => {
    set({ responsePanelCollapsed: collapsed });
  },

  updateUIState: (updates) => {
    const { uiState } = get();
    set({ uiState: { ...uiState, ...updates } });
  },

  connectSSE: async () => {
    const { currentRequest, disconnectSSE } = get();
    if (!currentRequest?.url) {
      message.warning('Please enter a URL');
      return;
    }

    disconnectSSE();

    const connectionId = `sse-${Date.now()}`;
    const connection: StreamingConnection = {
      id: connectionId,
      type: 'sse',
      status: 'connecting',
      url: currentRequest.url,
      startedAt: Date.now(),
    };

    set({
      streamingConnection: connection,
      sseEvents: [],
      responsePanelCollapsed: false,
      uiState: { ...get().uiState, responsePanelActiveTab: 'messages' },
    });

    try {
      const headers = currentRequest.headers
        .filter(h => h.enabled)
        .map(h => [h.key, h.value]);

      const abortController = new AbortController();

      const response = await fetch('/_bifrost/api/replay/execute/stream', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          url: currentRequest.url,
          headers,
          request_id: currentRequest.is_saved ? currentRequest.id : undefined,
        }),
        signal: abortController.signal,
      });

      if (!response.ok) {
        throw new Error(`SSE connection failed: ${response.status}`);
      }

      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error('No response body');
      }

      const sseController = {
        close: () => {
          abortController.abort();
          reader.cancel().catch(() => { });
        },
      };

      set({
        eventSourceRef: sseController as unknown as EventSource,
        streamingConnection: { ...connection, status: 'connected' },
      });

      let buffer = '';
      let isRunning = true;

      const processStream = async () => {
        try {
          while (isRunning) {
            const { done, value } = await reader.read();
            if (done) break;

            buffer += new TextDecoder().decode(value);

            const lines = buffer.split('\n');
            buffer = lines.pop() || '';

            for (const line of lines) {
              if (line === '') continue;
              if (line.startsWith('data: ')) {
                const data = line.substring(6);
                try {
                  const eventData = JSON.parse(data);
                  const sseEvent: SSEEvent = {
                    id: eventData.id,
                    event: eventData.event,
                    data: typeof eventData.data === 'string' ? eventData.data : JSON.stringify(eventData.data),
                    timestamp: Date.now(),
                  };
                  const { sseEvents } = get();
                  set({ sseEvents: [...sseEvents, sseEvent] });
                } catch {
                  const sseEvent: SSEEvent = {
                    data: data,
                    timestamp: Date.now(),
                  };
                  const { sseEvents } = get();
                  set({ sseEvents: [...sseEvents, sseEvent] });
                }
              }
            }
          }
        } catch (e) {
          if ((e as Error).name !== 'AbortError') {
            console.error('SSE stream error:', e);
          }
        } finally {
          isRunning = false;
          const { streamingConnection } = get();
          if (streamingConnection?.status === 'connected') {
            set({
              streamingConnection: { ...streamingConnection, status: 'disconnected', endedAt: Date.now() },
              eventSourceRef: null,
            });
          }
        }
      };

      processStream();
    } catch (e) {
      set({
        streamingConnection: {
          ...connection,
          status: 'error',
          error: String(e),
          endedAt: Date.now(),
        },
        eventSourceRef: null,
      });
    }
  },

  disconnectSSE: () => {
    const { eventSourceRef, streamingConnection } = get();
    if (eventSourceRef) {
      eventSourceRef.close();
    }
    set({
      eventSourceRef: null,
      streamingConnection: streamingConnection
        ? { ...streamingConnection, status: 'disconnected', endedAt: Date.now() }
        : null,
    });
  },

  connectWebSocket: () => {
    const { currentRequest, webSocketRef, disconnectWebSocket } = get();
    if (!currentRequest?.url) {
      message.warning('Please enter a URL');
      return;
    }

    if (webSocketRef) {
      disconnectWebSocket();
    }

    let wsUrl = currentRequest.url;
    if (wsUrl.startsWith('http://')) {
      wsUrl = wsUrl.replace('http://', 'ws://');
    } else if (wsUrl.startsWith('https://')) {
      wsUrl = wsUrl.replace('https://', 'wss://');
    } else if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
      wsUrl = `ws://${wsUrl}`;
    }

    // 构建 Bifrost WebSocket 代理 URL
    const proxyUrl = new URL('/_bifrost/api/replay/execute/ws', window.location.origin);
    proxyUrl.searchParams.set('url', wsUrl);
    if (currentRequest.is_saved) {
      proxyUrl.searchParams.set('request_id', currentRequest.id);
    }

    // 替换为 WebSocket 协议
    const wsProxyUrl = proxyUrl.toString().replace('http://', 'ws://').replace('https://', 'wss://');

    const connectionId = `ws-${Date.now()}`;
    const connection: StreamingConnection = {
      id: connectionId,
      type: 'websocket',
      status: 'connecting',
      url: wsUrl,
      startedAt: Date.now(),
    };

    set({
      streamingConnection: connection,
      wsMessages: [],
      responsePanelCollapsed: false,
      uiState: { ...get().uiState, responsePanelActiveTab: 'messages' },
    });

    try {
      const ws = new WebSocket(wsProxyUrl);

      ws.onopen = () => {
        set({
          streamingConnection: { ...connection, status: 'connected' },
        });
        message.success('WebSocket connected');
      };

      ws.onmessage = (event) => {
        const wsMessage: WebSocketMessage = {
          id: `msg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
          direction: 'receive',
          type: typeof event.data === 'string' ? 'text' : 'binary',
          data: typeof event.data === 'string' ? event.data : '[Binary Data]',
          timestamp: Date.now(),
        };
        const { wsMessages } = get();
        set({ wsMessages: [...wsMessages, wsMessage] });
      };

      ws.onclose = (event) => {
        const { streamingConnection } = get();
        set({
          streamingConnection: streamingConnection
            ? {
              ...streamingConnection,
              status: 'disconnected',
              endedAt: Date.now(),
              error: event.code !== 1000 ? `Closed: ${event.code} ${event.reason}` : undefined,
            }
            : null,
          webSocketRef: null,
        });
      };

      ws.onerror = () => {
        const { streamingConnection } = get();
        set({
          streamingConnection: streamingConnection
            ? {
              ...streamingConnection,
              status: 'error',
              error: 'Connection error',
              endedAt: Date.now(),
            }
            : null,
          webSocketRef: null,
        });
      };

      set({ webSocketRef: ws });
    } catch (e) {
      set({
        streamingConnection: {
          ...connection,
          status: 'error',
          error: String(e),
          endedAt: Date.now(),
        },
      });
    }
  },

  disconnectWebSocket: () => {
    const { webSocketRef, streamingConnection } = get();
    if (webSocketRef) {
      webSocketRef.close(1000, 'User disconnected');
      set({
        webSocketRef: null,
        streamingConnection: streamingConnection
          ? { ...streamingConnection, status: 'disconnected', endedAt: Date.now() }
          : null,
      });
    }
  },

  sendWebSocketMessage: (data: string, type: 'text' | 'binary' = 'text') => {
    const { webSocketRef, wsMessages, streamingConnection } = get();
    if (!webSocketRef || streamingConnection?.status !== 'connected') {
      message.warning('WebSocket is not connected');
      return;
    }

    try {
      webSocketRef.send(data);
      const wsMessage: WebSocketMessage = {
        id: `msg-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
        direction: 'send',
        type,
        data,
        timestamp: Date.now(),
      };
      set({ wsMessages: [...wsMessages, wsMessage] });
    } catch (e) {
      message.error(`Failed to send: ${e}`);
    }
  },

  clearStreamingMessages: () => {
    set({ sseEvents: [], wsMessages: [] });
  },

  reset: () => {
    const { disconnectSSE, disconnectWebSocket } = get();
    disconnectSSE();
    disconnectWebSocket();
    set({
      currentRequest: createEmptyRequest(),
      savedRequests: [],
      recentHistory: [],
      groups: [],
      currentResponse: null,
      currentTrafficRecord: null,
      ruleConfig: { mode: 'enabled' },
      loading: false,
      executing: false,
      streamingConnection: null,
      sseEvents: [],
      wsMessages: [],
    });
  },
}),
    {
      name: 'bifrost-replay',
      partialize: (state) => ({
        uiState: {
          collectionExpandedKeys: state.uiState.collectionExpandedKeys,
          selectedRequestId: state.uiState.selectedRequestId,
          requestPanelActiveTab: state.uiState.requestPanelActiveTab,
          responsePanelActiveTab: state.uiState.responsePanelActiveTab,
          responseViewMode: state.uiState.responseViewMode,
        },
      }),
      merge: (persisted, current) => {
        const persistedState = persisted as Partial<ReplayState>;
        const mergedExpandedKeys = persistedState?.uiState?.collectionExpandedKeys || [];
        if (!mergedExpandedKeys.includes('ungrouped')) {
          mergedExpandedKeys.push('ungrouped');
        }
        return {
          ...current,
          uiState: {
            ...current.uiState,
            ...(persistedState?.uiState || {}),
            collectionExpandedKeys: mergedExpandedKeys,
          },
        };
      },
    },
  ),
);

pushService.onReplayRequestUpdated((data) => {
  const { loadSavedRequests, loadGroups } = useReplayStore.getState();
  console.log('[ReplayStore] Received replay_request_updated:', data);

  if (data.action === 'group_created' || data.action === 'group_deleted') {
    loadGroups();
  }
  if (data.action === 'request_created' || data.action === 'request_deleted' || data.action === 'request_updated') {
    loadSavedRequests();
  }
});

pushService.onReplayHistoryUpdated((data) => {
  const { currentRequest, loadRecentHistory } = useReplayStore.getState();
  console.log('[ReplayStore] Received replay_history_updated:', data);

  if (currentRequest?.id && currentRequest.id === data.request_id) {
    loadRecentHistory(data.request_id);
  }
});
