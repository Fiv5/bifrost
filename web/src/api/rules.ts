import { get, post, put, del } from './client';
import type { RuleFile, RuleFileDetail, ApiResponse } from '../types';

export async function getRules(): Promise<RuleFile[]> {
  return get<RuleFile[]>('/rules');
}

export async function getRule(name: string): Promise<RuleFileDetail> {
  return get<RuleFileDetail>(`/rules/${encodeURIComponent(name)}`);
}

export async function createRule(name: string, content: string, enabled = true): Promise<ApiResponse> {
  return post<ApiResponse>('/rules', { name, content, enabled });
}

export async function updateRule(name: string, content?: string, enabled?: boolean): Promise<ApiResponse> {
  return put<ApiResponse>(`/rules/${encodeURIComponent(name)}`, { content, enabled });
}

export async function deleteRule(name: string): Promise<ApiResponse> {
  return del<ApiResponse>(`/rules/${encodeURIComponent(name)}`);
}

export async function enableRule(name: string): Promise<ApiResponse> {
  return put<ApiResponse>(`/rules/${encodeURIComponent(name)}/enable`);
}

export async function disableRule(name: string): Promise<ApiResponse> {
  return put<ApiResponse>(`/rules/${encodeURIComponent(name)}/disable`);
}
