export interface RuleFile {
  name: string;
  enabled: boolean;
  rule_count: number;
}

export interface RuleFileDetail {
  name: string;
  content: string;
  enabled: boolean;
}

export interface MatchedRule {
  pattern: string;
  protocol: string;
  value: string;
  rule_name?: string;
  raw?: string;
  line?: number;
}

export interface RequestTiming {
  dns_ms?: number;
  connect_ms?: number;
  tls_ms?: number;
  send_ms?: number;
  wait_ms?: number;
  receive_ms?: number;
  total_ms: number;
}

export interface SocketStatus {
  is_open: boolean;
  send_count: number;
  receive_count: number;
  send_bytes: number;
  receive_bytes: number;
  frame_count: number;
  close_code?: number;
  close_reason?: string;
}

export interface TrafficSummary {
  id: string;
  sequence: number;
  timestamp: number;
  method: string;
  url: string;
  status: number;
  content_type: string | null;
  request_size: number;
  response_size: number;
  duration_ms: number;
  host: string;
  path: string;
  protocol: string;
  client_ip: string;
  client_app?: string;
  client_pid?: number;
  has_rule_hit: boolean;
  matched_rule_count: number;
  matched_protocols: string[];
  is_websocket?: boolean;
  is_sse?: boolean;
  is_tunnel?: boolean;
  frame_count?: number;
  socket_status?: SocketStatus | null;
  start_time: string;
  end_time?: string | null;
}

export interface TrafficRecord extends TrafficSummary {
  request_headers: [string, string][] | null;
  response_headers: [string, string][] | null;
  request_body: string | null;
  response_body: string | null;
  matched_rules: MatchedRule[] | null;
  request_content_type: string | null;
  timing?: RequestTiming | null;
  last_frame_id?: number;
}

export type FrameDirection = 'send' | 'receive';

export type FrameType = 'text' | 'binary' | 'ping' | 'pong' | 'close' | 'continuation' | 'sse';

export interface WebSocketFrame {
  frame_id: number;
  timestamp: number;
  direction: FrameDirection;
  frame_type: FrameType;
  payload_size: number;
  payload_preview?: string;
  is_masked: boolean;
  is_fin: boolean;
}

export interface FramesResponse {
  frames: WebSocketFrame[];
  socket_status: SocketStatus | null;
  last_frame_id: number;
  has_more: boolean;
  is_monitored: boolean;
}

export interface TrafficListResponse {
  total: number;
  offset: number;
  limit: number;
  records: TrafficSummary[];
}

export interface TrafficUpdatesResponse {
  new_records: TrafficSummary[];
  updated_records: TrafficSummary[];
  has_more: boolean;
  server_total: number;
}

export interface TrafficUpdatesFilter extends TrafficFilter {
  after_id?: string;
  pending_ids?: string;
}

export interface TrafficFilter {
  method?: string;
  status?: number;
  status_min?: number;
  status_max?: number;
  url_contains?: string;
  host?: string;
  content_type?: string;
  limit?: number;
  offset?: number;
  has_rule_hit?: boolean;
  protocol?: string;
  request_content_type?: string;
  domain?: string;
  path_contains?: string;
  header_contains?: string;
  client_ip?: string;
  client_app?: string;
}

export interface ToolbarFilters {
  rule: string[];
  protocol: string[];
  type: string[];
  status: string[];
}

export interface FilterCondition {
  id: string;
  field: string;
  operator: string;
  value: string;
}

export interface TrafficTypeMetrics {
  requests: number;
  bytes_sent: number;
  bytes_received: number;
  active_connections: number;
}

export interface MetricsSnapshot {
  timestamp: number;
  memory_used: number;
  memory_total: number;
  cpu_usage: number;
  total_requests: number;
  active_connections: number;
  bytes_sent: number;
  bytes_received: number;
  bytes_sent_rate: number;
  bytes_received_rate: number;
  qps: number;
  max_qps: number;
  max_bytes_sent_rate: number;
  max_bytes_received_rate: number;
  http: TrafficTypeMetrics;
  https: TrafficTypeMetrics;
  tunnel: TrafficTypeMetrics;
  ws: TrafficTypeMetrics;
  wss: TrafficTypeMetrics;
}

export interface AppMetrics {
  app_name: string;
  requests: number;
  active_connections: number;
  bytes_sent: number;
  bytes_received: number;
  http_requests: number;
  https_requests: number;
  tunnel_requests: number;
  ws_requests: number;
  wss_requests: number;
}

export interface SystemInfo {
  version: string;
  rust_version: string;
  os: string;
  arch: string;
  uptime_secs: number;
  pid: number;
}

export interface SystemOverview {
  system: SystemInfo;
  metrics: MetricsSnapshot;
  rules: {
    total: number;
    enabled: number;
  };
  traffic: {
    recorded: number;
  };
  server: {
    port: number;
    admin_url: string;
  };
  pending_authorizations: number;
}

export interface ApiResponse<T = unknown> {
  success?: boolean;
  message?: string;
  error?: string;
  status?: number;
  data?: T;
}

export type AccessMode = 'allow_all' | 'local_only' | 'whitelist' | 'interactive';

export interface WhitelistStatus {
  mode: AccessMode;
  allow_lan: boolean;
  whitelist: string[];
  temporary_whitelist: string[];
}

export interface PendingAuth {
  ip: string;
  first_seen: number;
  attempt_count: number;
}

export interface TlsConfig {
  enable_tls_interception: boolean;
  intercept_exclude: string[];
  intercept_include: string[];
  app_intercept_exclude: string[];
  app_intercept_include: string[];
  unsafe_ssl: boolean;
  disconnect_on_config_change: boolean;
}

export interface ProxySettings {
  tls: TlsConfig;
  port: number;
  host: string;
}

export interface CertInfo {
  available: boolean;
  local_ips: string[];
  download_urls: string[];
  qrcode_urls: string[];
}

export interface SessionTargetSearchState {
  value?: string;
  show?: boolean;
  total?: number;
  next?: number;
  tab?: string;
}

export const DisplayFormat = {
  HighLight: 'HighLight',
  Hex: 'Hex',
  Tree: 'Tree',
  Media: 'Media',
} as const;

export type DisplayFormat = typeof DisplayFormat[keyof typeof DisplayFormat];

export type RecordContentType =
  | 'JSON'
  | 'HTML'
  | 'XML'
  | 'JavaScript'
  | 'CSS'
  | 'Media'
  | 'Other';

export interface KeyValueItem {
  key: string;
  value?: string | number;
  id?: string;
  children?: KeyValueItem[];
}
