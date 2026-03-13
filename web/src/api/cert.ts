import { get } from './client';
import { buildPublicUrl } from '../runtime';

export interface CertInfo {
  available: boolean;
  status: "not_installed" | "installed_not_trusted" | "installed_and_trusted" | "unknown";
  status_label: string;
  installed: boolean;
  trusted: boolean;
  status_message: string;
  local_ips: string[];
  download_urls: string[];
  qrcode_urls: string[];
}

export async function getCertInfo(): Promise<CertInfo> {
  return get<CertInfo>('/cert/info');
}

export function getCertDownloadUrl(): string {
  return buildPublicUrl('/cert');
}

export function getCertQRCodeUrl(ip?: string): string {
  const baseUrl = buildPublicUrl('/cert/qrcode');
  if (ip) {
    return `${baseUrl}?ip=${encodeURIComponent(ip)}`;
  }
  return baseUrl;
}
