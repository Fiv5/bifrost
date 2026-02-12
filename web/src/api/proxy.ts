import { get, put } from './client';

export interface SystemProxyStatus {
  supported: boolean;
  enabled: boolean;
  host: string;
  port: number;
  bypass: string;
}

export interface SystemProxySupportStatus {
  supported: boolean;
  platform: string;
}

export interface SetSystemProxyRequest {
  enabled: boolean;
  bypass?: string;
}

export async function getSystemProxyStatus(): Promise<SystemProxyStatus> {
  return get<SystemProxyStatus>('/proxy/system');
}

export async function setSystemProxy(request: SetSystemProxyRequest): Promise<SystemProxyStatus> {
  return put<SystemProxyStatus>('/proxy/system', request);
}

export async function getSystemProxySupport(): Promise<SystemProxySupportStatus> {
  return get<SystemProxySupportStatus>('/proxy/system/support');
}
