import { getClientId } from '../services/clientId';

export function apiFetch(input: RequestInfo | URL, init: RequestInit = {}) {
  const headers = new Headers(init.headers);
  headers.set('X-Client-Id', getClientId());
  return fetch(input, { ...init, headers });
}

