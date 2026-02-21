import { get } from './client';

export interface CertInfo {
  available: boolean;
  local_ips: string[];
  download_urls: string[];
  qrcode_urls: string[];
}

export async function getCertInfo(): Promise<CertInfo> {
  return get<CertInfo>('/cert/info');
}

export function getCertDownloadUrl(): string {
  return '/_bifrost/public/cert';
}

export function getCertQRCodeUrl(ip?: string): string {
  const host = window.location.host;
  const baseUrl = `http://${host}/_bifrost/public/cert/qrcode`;
  if (ip) {
    return `${baseUrl}?ip=${encodeURIComponent(ip)}`;
  }
  return baseUrl;
}
