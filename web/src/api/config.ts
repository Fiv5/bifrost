import { get, put } from './client';

export interface TlsConfig {
  enable_tls_interception: boolean;
  intercept_exclude: string[];
  intercept_include: string[];
  unsafe_ssl: boolean;
  disconnect_on_config_change: boolean;
}

export interface ProxySettings {
  tls: TlsConfig;
  port: number;
  host: string;
}

export interface UpdateTlsConfigRequest {
  enable_tls_interception?: boolean;
  intercept_exclude?: string[];
  intercept_include?: string[];
  unsafe_ssl?: boolean;
  disconnect_on_config_change?: boolean;
}

export async function getProxySettings(): Promise<ProxySettings> {
  return get<ProxySettings>('/config');
}

export async function getTlsConfig(): Promise<TlsConfig> {
  return get<TlsConfig>('/config/tls');
}

export async function updateTlsConfig(config: UpdateTlsConfigRequest): Promise<TlsConfig> {
  return put<TlsConfig>('/config/tls', config);
}

export interface TrafficConfig {
  max_records: number;
  max_body_memory_size: number;
  max_body_buffer_size: number;
  file_retention_days: number;
}

export interface BodyStoreStats {
  file_count: number;
  total_size: number;
  temp_dir: string;
  max_memory_size: number;
  retention_days: number;
}

export interface PerformanceConfig {
  traffic: TrafficConfig;
  body_store_stats: BodyStoreStats | null;
}

export interface UpdateTrafficConfigRequest {
  max_records?: number;
  max_body_memory_size?: number;
  max_body_buffer_size?: number;
  file_retention_days?: number;
}

export async function getPerformanceConfig(): Promise<PerformanceConfig> {
  return get<PerformanceConfig>('/config/performance');
}

export async function updatePerformanceConfig(config: UpdateTrafficConfigRequest): Promise<PerformanceConfig> {
  return put<PerformanceConfig>('/config/performance', config);
}
