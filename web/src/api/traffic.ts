import { get, del } from './client';
import type { TrafficListResponse, TrafficRecord, TrafficFilter, ApiResponse } from '../types';

export async function getTrafficList(filter?: TrafficFilter): Promise<TrafficListResponse> {
  const params = new URLSearchParams();
  if (filter) {
    Object.entries(filter).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    });
  }
  const query = params.toString();
  return get<TrafficListResponse>(`/traffic${query ? `?${query}` : ''}`);
}

export async function getTrafficDetail(id: string): Promise<TrafficRecord> {
  return get<TrafficRecord>(`/traffic/${encodeURIComponent(id)}`);
}

export async function clearTraffic(): Promise<ApiResponse> {
  return del<ApiResponse>('/traffic');
}
