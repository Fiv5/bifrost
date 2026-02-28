import { get } from './client';
import type { VersionCheckResponse } from '../types';

export async function checkVersion(forceRefresh = false): Promise<VersionCheckResponse> {
  const query = forceRefresh ? '?refresh=true' : '';
  return get<VersionCheckResponse>(`/system/version-check${query}`);
}
