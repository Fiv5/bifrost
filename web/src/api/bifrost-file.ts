import axios from 'axios';
import { getClientId } from '../services/clientId';

const API_BASE = '/_bifrost/api/bifrost-file';

export type BifrostFileType = 'rules' | 'network' | 'script' | 'values' | 'template';

export interface DetectResponse {
  file_type: BifrostFileType;
  meta: Record<string, unknown>;
}

export interface ImportResponse {
  success: boolean;
  file_type: BifrostFileType;
  data: ImportedData;
  warnings?: string[];
}

export interface ImportedData {
  rule_names?: string[];
  rule_count?: number;
  record_count?: number;
  script_names?: string[];
  script_count?: number;
  value_names?: string[];
  value_count?: number;
  group_count?: number;
  request_count?: number;
}

export interface ExportRulesRequest {
  rule_names: string[];
  description?: string;
}

export interface ExportNetworkRequest {
  record_ids: string[];
  include_body?: boolean;
  description?: string;
}

export interface ExportScriptRequest {
  script_names: string[];
  description?: string;
}

export interface ExportValuesRequest {
  value_names?: string[];
  description?: string;
}

export interface ExportTemplateRequest {
  group_ids?: string[];
  request_ids?: string[];
  description?: string;
}

export async function detectType(content: string): Promise<DetectResponse> {
  const response = await axios.post<DetectResponse>(`${API_BASE}/detect`, content, {
    headers: { 'Content-Type': 'text/plain', 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function importFile(content: string): Promise<ImportResponse> {
  const response = await axios.post<ImportResponse>(`${API_BASE}/import`, content, {
    headers: { 'Content-Type': 'text/plain', 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function exportRules(request: ExportRulesRequest): Promise<string> {
  const response = await axios.post<string>(`${API_BASE}/export/rules`, request, {
    responseType: 'text',
    headers: { 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function exportNetwork(request: ExportNetworkRequest): Promise<string> {
  const response = await axios.post<string>(`${API_BASE}/export/network`, request, {
    responseType: 'text',
    headers: { 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function exportScripts(request: ExportScriptRequest): Promise<string> {
  const response = await axios.post<string>(`${API_BASE}/export/scripts`, request, {
    responseType: 'text',
    headers: { 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function exportValues(request: ExportValuesRequest): Promise<string> {
  const response = await axios.post<string>(`${API_BASE}/export/values`, request, {
    responseType: 'text',
    headers: { 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export async function exportTemplates(request: ExportTemplateRequest): Promise<string> {
  const response = await axios.post<string>(`${API_BASE}/export/templates`, request, {
    responseType: 'text',
    headers: { 'X-Client-Id': getClientId() },
  });
  return response.data;
}

export function downloadFile(content: string, filename: string): void {
  const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function formatExportFilename(type: BifrostFileType, count?: number): string {
  const date = new Date().toISOString().slice(0, 19).replace(/[:-]/g, '');
  const suffix = count && count > 1 ? `-${count}` : '';
  return `bifrost-${type}${suffix}-${date}.bifrost`;
}
