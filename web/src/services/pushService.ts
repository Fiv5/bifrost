import type {
  TrafficSummary,
  TrafficSummaryCompact,
  TrafficDeltaData,
  SystemOverview,
  MetricsSnapshot,
} from '../types';

export interface TrafficUpdatesData {
  new_records: TrafficSummary[];
  updated_records: TrafficSummary[];
  has_more: boolean;
  server_total: number;
}

export interface TrafficUpdatesDataCompact {
  new_records: TrafficSummaryCompact[];
  updated_records: TrafficSummaryCompact[];
  has_more: boolean;
  server_total: number;
  server_sequence: number;
}

export type { TrafficDeltaData };

export interface OverviewData {
  system: SystemOverview['system'];
  metrics: MetricsSnapshot;
  rules: { total: number; enabled: number };
  traffic: { recorded: number };
  server: { port: number; admin_url: string };
  pending_authorizations: number;
}

export interface MetricsData {
  metrics: MetricsSnapshot;
}

export interface HistoryData {
  history: MetricsSnapshot[];
}

export interface ConnectedData {
  client_id: number;
  message: string;
}

export interface ErrorData {
  message: string;
}

export type PushMessageType =
  | 'traffic_updates'
  | 'traffic_delta'
  | 'overview_update'
  | 'metrics_update'
  | 'history_update'
  | 'connected'
  | 'error';

export interface PushMessage {
  type: PushMessageType;
  data:
    | TrafficUpdatesData
    | TrafficDeltaData
    | OverviewData
    | MetricsData
    | HistoryData
    | ConnectedData
    | ErrorData;
}

export interface ClientSubscription {
  last_traffic_id?: string;
  last_sequence?: number;
  pending_ids?: string[];
  need_overview?: boolean;
  need_metrics?: boolean;
  need_history?: boolean;
  history_limit?: number;
  metrics_interval_ms?: number;
}

export const METRICS_INTERVAL_MIN_MS = 200;
export const METRICS_INTERVAL_MAX_MS = 5000;
export const METRICS_INTERVAL_DEFAULT_MS = 1000;
export const METRICS_INTERVAL_FAST_MS = 250;

type MessageHandler<T> = (data: T) => void;

interface PushServiceConfig {
  reconnectInterval?: number;
  maxReconnectAttempts?: number;
}

class PushService {
  private ws: WebSocket | null = null;
  private subscription: ClientSubscription = {};
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private config: Required<PushServiceConfig>;
  private isManualClose = false;

  private trafficHandlers: Set<MessageHandler<TrafficUpdatesData>> = new Set();
  private trafficDeltaHandlers: Set<MessageHandler<TrafficDeltaData>> = new Set();
  private overviewHandlers: Set<MessageHandler<OverviewData>> = new Set();
  private metricsHandlers: Set<MessageHandler<MetricsData>> = new Set();
  private historyHandlers: Set<MessageHandler<HistoryData>> = new Set();
  private connectionHandlers: Set<MessageHandler<{ connected: boolean; clientId?: number }>> = new Set();

  constructor(config: PushServiceConfig = {}) {
    this.config = {
      reconnectInterval: config.reconnectInterval ?? 3000,
      maxReconnectAttempts: config.maxReconnectAttempts ?? 10,
    };
  }

  connect(subscription: ClientSubscription = {}): void {
    if (this.ws?.readyState === WebSocket.OPEN || this.ws?.readyState === WebSocket.CONNECTING) {
      this.updateSubscription(subscription);
      return;
    }

    this.subscription = subscription;
    this.isManualClose = false;
    this.createConnection();
  }

  private createConnection(): void {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.host;
    const params = this.buildQueryParams();
    const url = `${protocol}//${host}/_bifrost/api/push${params ? `?${params}` : ''}`;

    try {
      this.ws = new WebSocket(url);
      this.setupEventHandlers();
    } catch (error) {
      console.error('[PushService] Failed to create WebSocket:', error);
      this.scheduleReconnect();
    }
  }

  private buildQueryParams(): string {
    const params = new URLSearchParams();

    if (this.subscription.last_traffic_id) {
      params.append('last_traffic_id', this.subscription.last_traffic_id);
    }

    if (this.subscription.pending_ids && this.subscription.pending_ids.length > 0) {
      params.append('pending_ids', this.subscription.pending_ids.join(','));
    }

    if (this.subscription.need_overview) {
      params.append('need_overview', 'true');
    }

    if (this.subscription.need_metrics) {
      params.append('need_metrics', 'true');
    }

    if (this.subscription.need_history) {
      params.append('need_history', 'true');
    }

    if (this.subscription.history_limit) {
      params.append('history_limit', String(this.subscription.history_limit));
    }

    if (this.subscription.metrics_interval_ms) {
      params.append('metrics_interval_ms', String(this.subscription.metrics_interval_ms));
    }

    return params.toString();
  }

  private setupEventHandlers(): void {
    if (!this.ws) return;

    this.ws.onopen = () => {
      console.log('[PushService] Connected');
      this.reconnectAttempts = 0;
    };

    this.ws.onclose = (event) => {
      console.log('[PushService] Disconnected:', event.code, event.reason);
      this.notifyConnectionHandlers(false);
      if (!this.isManualClose) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = (error) => {
      console.error('[PushService] Error:', error);
    };

    this.ws.onmessage = (event) => {
      try {
        const message: PushMessage = JSON.parse(event.data);
        this.handleMessage(message);
      } catch (error) {
        console.error('[PushService] Failed to parse message:', error);
      }
    };
  }

  private handleMessage(message: PushMessage): void {
    switch (message.type) {
      case 'connected': {
        const data = message.data as ConnectedData;
        console.log('[PushService] Connected with client ID:', data.client_id);
        this.notifyConnectionHandlers(true, data.client_id);
        break;
      }
      case 'traffic_updates': {
        const data = message.data as TrafficUpdatesData;
        this.trafficHandlers.forEach((handler) => handler(data));
        break;
      }
      case 'traffic_delta': {
        const data = message.data as TrafficDeltaData;
        this.trafficDeltaHandlers.forEach((handler) => handler(data));
        break;
      }
      case 'overview_update': {
        const data = message.data as OverviewData;
        this.overviewHandlers.forEach((handler) => handler(data));
        break;
      }
      case 'metrics_update': {
        const data = message.data as MetricsData;
        this.metricsHandlers.forEach((handler) => handler(data));
        break;
      }
      case 'history_update': {
        const data = message.data as HistoryData;
        this.historyHandlers.forEach((handler) => handler(data));
        break;
      }
      case 'error': {
        const data = message.data as ErrorData;
        console.error('[PushService] Server error:', data.message);
        break;
      }
    }
  }

  private notifyConnectionHandlers(connected: boolean, clientId?: number): void {
    this.connectionHandlers.forEach((handler) => handler({ connected, clientId }));
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
    }

    if (this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      console.error('[PushService] Max reconnect attempts reached');
      return;
    }

    const delay = this.config.reconnectInterval * Math.pow(1.5, this.reconnectAttempts);
    console.log(`[PushService] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts + 1})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectAttempts++;
      this.createConnection();
    }, delay);
  }

  disconnect(): void {
    this.isManualClose = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  updateSubscription(subscription: Partial<ClientSubscription>): void {
    this.subscription = { ...this.subscription, ...subscription };

    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(this.subscription));
    }
  }

  getSubscription(): ClientSubscription {
    return { ...this.subscription };
  }

  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  onTrafficUpdates(handler: MessageHandler<TrafficUpdatesData>): () => void {
    this.trafficHandlers.add(handler);
    return () => this.trafficHandlers.delete(handler);
  }

  onTrafficDelta(handler: MessageHandler<TrafficDeltaData>): () => void {
    this.trafficDeltaHandlers.add(handler);
    return () => this.trafficDeltaHandlers.delete(handler);
  }

  onOverviewUpdate(handler: MessageHandler<OverviewData>): () => void {
    this.overviewHandlers.add(handler);
    return () => this.overviewHandlers.delete(handler);
  }

  onMetricsUpdate(handler: MessageHandler<MetricsData>): () => void {
    this.metricsHandlers.add(handler);
    return () => this.metricsHandlers.delete(handler);
  }

  onHistoryUpdate(handler: MessageHandler<HistoryData>): () => void {
    this.historyHandlers.add(handler);
    return () => this.historyHandlers.delete(handler);
  }

  onConnectionChange(handler: MessageHandler<{ connected: boolean; clientId?: number }>): () => void {
    this.connectionHandlers.add(handler);
    return () => this.connectionHandlers.delete(handler);
  }
}

export const pushService = new PushService();
export default pushService;
