import { get, post, put, del } from './client';
import type { ApiResponse } from '../types';

export interface ValueItem {
  name: string;
  value: string;
  created_at: string;
  updated_at: string;
}

export interface ValuesListResponse {
  values: ValueItem[];
  total: number;
}

export async function getValues(): Promise<ValuesListResponse> {
  return get<ValuesListResponse>('/values');
}

export async function getValue(name: string): Promise<ValueItem> {
  return get<ValueItem>(`/values/${encodeURIComponent(name)}`);
}

export async function createValue(name: string, value: string): Promise<ApiResponse> {
  return post<ApiResponse>('/values', { name, value });
}

export async function updateValue(name: string, value: string): Promise<ApiResponse> {
  return put<ApiResponse>(`/values/${encodeURIComponent(name)}`, { value });
}

export async function deleteValue(name: string): Promise<ApiResponse> {
  return del<ApiResponse>(`/values/${encodeURIComponent(name)}`);
}
