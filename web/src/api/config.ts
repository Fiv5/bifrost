import { get, put } from './client';

export interface TlsConfig {
  enable_tls_interception: boolean;
  intercept_exclude: string[];
  intercept_include: string[];
  unsafe_ssl: boolean;
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
