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

export interface TrafficSummary {
  id: string;
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
}

export interface TrafficRecord extends TrafficSummary {
  request_headers: [string, string][] | null;
  response_headers: [string, string][] | null;
  request_body: string | null;
  response_body: string | null;
  client_ip: string;
  protocol: string;
}

export interface TrafficListResponse {
  total: number;
  offset: number;
  limit: number;
  records: TrafficSummary[];
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
  qps: number;
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
}

export interface ApiResponse<T = unknown> {
  success?: boolean;
  message?: string;
  error?: string;
  status?: number;
  data?: T;
}
