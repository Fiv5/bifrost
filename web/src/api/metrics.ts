import { get } from './client';
import type { MetricsSnapshot, SystemOverview, AppMetrics } from '../types';

export async function getMetrics(): Promise<MetricsSnapshot> {
  return get<MetricsSnapshot>('/metrics');
}

export async function getMetricsHistory(limit?: number): Promise<MetricsSnapshot[]> {
  const query = limit ? `?limit=${limit}` : '';
  return get<MetricsSnapshot[]>(`/metrics/history${query}`);
}

export async function getSystemOverview(): Promise<SystemOverview> {
  return get<SystemOverview>('/system/overview');
}

export async function getAppMetrics(): Promise<AppMetrics[]> {
  return get<AppMetrics[]>('/metrics/apps');
}
