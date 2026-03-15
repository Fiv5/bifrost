import { get, del, post } from './client';
import type { TrafficListResponse, TrafficRecord, TrafficFilter, TrafficUpdatesFilter, TrafficUpdatesResponseCompact, ApiResponse, TrafficQueryRequest, TrafficQueryResponse } from '../types';
import { buildApiUrl } from '../runtime';

export async function queryTraffic(request: TrafficQueryRequest): Promise<TrafficQueryResponse> {
  return post<TrafficQueryResponse>('/traffic/query', request);
}

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

export async function getTrafficUpdates(filter?: TrafficUpdatesFilter): Promise<TrafficUpdatesResponseCompact> {
  const params = new URLSearchParams();
  if (filter) {
    Object.entries(filter).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    });
  }
  const query = params.toString();
  return get<TrafficUpdatesResponseCompact>(`/traffic/updates${query ? `?${query}` : ''}`);
}

export async function getTrafficDetail(id: string): Promise<TrafficRecord> {
  return get<TrafficRecord>(`/traffic/${encodeURIComponent(id)}`);
}

export async function clearTraffic(ids?: string[]): Promise<ApiResponse> {
  if (ids && ids.length > 0) {
    return del<ApiResponse>('/traffic', { ids });
  }
  return del<ApiResponse>('/traffic');
}

export async function getRequestBody(id: string): Promise<string | null> {
  const response = await get<ApiResponse<string>>(`/traffic/${encodeURIComponent(id)}/request-body`);
  return response.data || null;
}

export async function getResponseBody(id: string): Promise<string | null> {
  const response = await get<ApiResponse<string>>(`/traffic/${encodeURIComponent(id)}/response-body`);
  return response.data || null;
}

export function getResponseBodyContentUrl(id: string, raw = true): string {
  const params = new URLSearchParams();
  if (raw) {
    params.set('raw', '1');
  }
  const suffix = params.toString();
  return buildApiUrl(
    `/traffic/${encodeURIComponent(id)}/response-body/content${suffix ? `?${suffix}` : ''}`
  );
}
