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

export function getCertQRCodeUrl(): string {
  const host = window.location.host;
  return `http://${host}/_bifrost/public/cert/qrcode`;
}
