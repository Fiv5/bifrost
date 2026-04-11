import { apiFetch } from '../api/apiFetch';

const TOKEN_KEY = 'bifrost.admin.jwt';

export type AdminAuthStatus = {
  remote_access_enabled: boolean;
  auth_required: boolean;
  username: string;
};

export function getAdminToken(): string | null {
  try {
    const v = window.localStorage.getItem(TOKEN_KEY);
    return v && v.trim() ? v : null;
  } catch {
    return null;
  }
}

export function setAdminToken(token: string): void {
  try {
    window.localStorage.setItem(TOKEN_KEY, token);
  } catch {
    // ignore
  }
}

export function clearAdminToken(): void {
  try {
    window.localStorage.removeItem(TOKEN_KEY);
  } catch {
    // ignore
  }
}

export async function fetchAdminAuthStatus(): Promise<AdminAuthStatus> {
  const resp = await apiFetch('/api/auth/status', { method: 'GET' });
  if (!resp.ok) {
    throw new Error(`Failed to load auth status: ${resp.status}`);
  }
  return (await resp.json()) as AdminAuthStatus;
}

