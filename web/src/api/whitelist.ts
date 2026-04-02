import { get, post, put, del } from './client';
import type { WhitelistStatus, AccessMode, PendingAuth, UserPassAccountUpdate } from '../types';

export async function getWhitelistStatus(): Promise<WhitelistStatus> {
  return get<WhitelistStatus>('/whitelist');
}

export async function addToWhitelist(ipOrCidr: string): Promise<{ success: boolean; message: string }> {
  return post('/whitelist', { ip_or_cidr: ipOrCidr });
}

export async function removeFromWhitelist(ipOrCidr: string): Promise<{ success: boolean; message: string }> {
  return del('/whitelist', { ip_or_cidr: ipOrCidr });
}

export async function getAccessMode(): Promise<{ mode: AccessMode }> {
  return get('/whitelist/mode');
}

export async function setAccessMode(mode: AccessMode): Promise<{ success: boolean; mode: AccessMode }> {
  return put('/whitelist/mode', { mode });
}

export async function getAllowLan(): Promise<{ allow_lan: boolean }> {
  return get('/whitelist/allow-lan');
}

export async function setAllowLan(allowLan: boolean): Promise<{ success: boolean; allow_lan: boolean }> {
  return put('/whitelist/allow-lan', { allow_lan: allowLan });
}

export async function setUserPassConfig(
  enabled: boolean,
  accounts: UserPassAccountUpdate[],
  loopback_requires_auth: boolean = false
): Promise<{ success: boolean }> {
  return put('/whitelist/userpass', { enabled, accounts, loopback_requires_auth });
}

export async function addTemporary(ip: string): Promise<{ success: boolean; message: string }> {
  return post('/whitelist/temporary', { ip });
}

export async function removeTemporary(ip: string): Promise<{ success: boolean; message: string }> {
  return del('/whitelist/temporary', { ip });
}

export async function getPendingAuthorizations(): Promise<PendingAuth[]> {
  return get<PendingAuth[]>('/whitelist/pending');
}

export async function approvePending(ip: string): Promise<{ success: boolean; message: string }> {
  return post('/whitelist/pending/approve', { ip });
}

export async function rejectPending(ip: string): Promise<{ success: boolean; message: string }> {
  return post('/whitelist/pending/reject', { ip });
}

export async function clearPendingAuthorizations(): Promise<{ success: boolean; message: string }> {
  return del('/whitelist/pending');
}

export const clearPending = clearPendingAuthorizations;
