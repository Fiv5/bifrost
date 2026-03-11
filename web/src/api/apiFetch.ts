import { getClientId } from '../services/clientId';
import { resolveRequestUrl } from '../runtime';

export function apiFetch(input: RequestInfo | URL, init: RequestInit = {}) {
  const headers = new Headers(init.headers);
  headers.set('X-Client-Id', getClientId());
  return fetch(resolveRequestUrl(input), { ...init, headers });
}
