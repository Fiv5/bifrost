import { getClientId } from '../services/clientId';
import { getAdminToken } from '../services/adminAuth';
import { resolveRequestUrl } from '../runtime';

export function apiFetch(input: RequestInfo | URL, init: RequestInit = {}) {
  const headers = new Headers(init.headers);
  headers.set('X-Client-Id', getClientId());
  const token = getAdminToken();
  if (token && !headers.has('Authorization')) {
    headers.set('Authorization', `Bearer ${token}`);
  }
  return fetch(resolveRequestUrl(input), { ...init, headers });
}
