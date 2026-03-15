import { get, del, post, put } from './client';
import type {
  ReplayGroup,
  ReplayRequest,
  ReplayRequestSummary,
  ReplayHistory,
  ReplayExecuteRequest,
  ReplayExecuteResponse,
  ReplayDbStats,
  ReplayKeyValueItem,
  ReplayBody,
  RuleConfig,
  RequestType,
} from '../types';

interface GroupsResponse {
  groups: ReplayGroup[];
}

interface RequestsResponse {
  requests: ReplayRequestSummary[];
  total: number;
  max_requests: number;
}

interface HistoryResponse {
  history: ReplayHistory[];
  total: number;
  max_history: number;
}

interface CountResponse {
  count: number;
  max_requests?: number;
  max_history?: number;
}

interface ExecuteResult {
  success: boolean;
  data: ReplayExecuteResponse;
}

interface ClearHistoryResponse {
  success: boolean;
  deleted: number;
}

export async function executeReplay(request: ReplayExecuteRequest): Promise<ReplayExecuteResponse> {
  const result = await post<ExecuteResult>('/replay/execute', request);
  return result.data;
}

export async function listGroups(): Promise<ReplayGroup[]> {
  const response = await get<GroupsResponse>('/replay/groups');
  return response.groups;
}

export async function createGroup(name: string, parentId?: string): Promise<ReplayGroup> {
  return post<ReplayGroup>('/replay/groups', { name, parent_id: parentId });
}

export async function getGroup(id: string): Promise<ReplayGroup> {
  return get<ReplayGroup>(`/replay/groups/${encodeURIComponent(id)}`);
}

export async function updateGroup(
  id: string,
  data: { name?: string; parent_id?: string; sort_order?: number }
): Promise<ReplayGroup> {
  return put<ReplayGroup>(`/replay/groups/${encodeURIComponent(id)}`, data);
}

export async function deleteGroup(id: string): Promise<void> {
  await del(`/replay/groups/${encodeURIComponent(id)}`);
}

interface ListRequestsParams {
  saved?: boolean;
  group_id?: string;
  limit?: number;
  offset?: number;
}

export async function listRequests(params?: ListRequestsParams): Promise<RequestsResponse> {
  const urlParams = new URLSearchParams();
  if (params) {
    if (params.saved !== undefined) urlParams.append('saved', String(params.saved));
    if (params.group_id !== undefined) urlParams.append('group_id', params.group_id);
    if (params.limit !== undefined) urlParams.append('limit', String(params.limit));
    if (params.offset !== undefined) urlParams.append('offset', String(params.offset));
  }
  const query = urlParams.toString();
  return get<RequestsResponse>(`/replay/requests${query ? `?${query}` : ''}`);
}

export async function countRequests(): Promise<CountResponse> {
  return get<CountResponse>('/replay/requests/count');
}

export interface CreateRequestParams {
  group_id?: string;
  name?: string;
  request_type?: RequestType;
  method: string;
  url: string;
  headers?: ReplayKeyValueItem[];
  body?: ReplayBody;
  is_saved?: boolean;
}

export async function createRequest(params: CreateRequestParams): Promise<ReplayRequest> {
  return post<ReplayRequest>('/replay/requests', params);
}

export async function getRequest(id: string): Promise<ReplayRequest> {
  return get<ReplayRequest>(`/replay/requests/${encodeURIComponent(id)}`);
}

export interface UpdateRequestParams {
  group_id?: string;
  name?: string;
  request_type?: RequestType;
  method?: string;
  url?: string;
  headers?: ReplayKeyValueItem[];
  body?: ReplayBody;
  is_saved?: boolean;
  sort_order?: number;
}

export async function updateRequest(id: string, params: UpdateRequestParams): Promise<ReplayRequest> {
  return put<ReplayRequest>(`/replay/requests/${encodeURIComponent(id)}`, params);
}

export async function deleteRequest(id: string): Promise<void> {
  await del(`/replay/requests/${encodeURIComponent(id)}`);
}

export async function moveRequest(id: string, groupId?: string): Promise<void> {
  await put(`/replay/requests/${encodeURIComponent(id)}/move`, { group_id: groupId });
}

interface ListHistoryParams {
  request_id?: string;
  binding?: 'unbound';
  limit?: number;
  offset?: number;
}

export async function listHistory(params?: ListHistoryParams): Promise<HistoryResponse> {
  const urlParams = new URLSearchParams();
  if (params) {
    if (params.request_id !== undefined) urlParams.append('request_id', params.request_id);
    if (params.binding !== undefined) urlParams.append('binding', params.binding);
    if (params.limit !== undefined) urlParams.append('limit', String(params.limit));
    if (params.offset !== undefined) urlParams.append('offset', String(params.offset));
  }
  const query = urlParams.toString();
  return get<HistoryResponse>(`/replay/history${query ? `?${query}` : ''}`);
}

export async function countHistory(requestId?: string, binding?: 'unbound'): Promise<CountResponse> {
  const urlParams = new URLSearchParams();
  if (requestId !== undefined) urlParams.append('request_id', requestId);
  if (binding !== undefined) urlParams.append('binding', binding);
  const query = urlParams.toString();
  return get<CountResponse>(`/replay/history/count${query ? `?${query}` : ''}`);
}

export async function deleteHistory(id: string): Promise<void> {
  await del(`/replay/history/${encodeURIComponent(id)}`);
}

export async function clearHistory(requestId?: string): Promise<ClearHistoryResponse> {
  const urlParams = new URLSearchParams();
  if (requestId !== undefined) urlParams.append('request_id', requestId);
  const query = urlParams.toString();
  return del<ClearHistoryResponse>(`/replay/history${query ? `?${query}` : ''}`);
}

export async function getReplayStats(): Promise<ReplayDbStats> {
  return get<ReplayDbStats>('/replay/stats');
}

export function buildReplayExecuteRequest(
  method: string,
  url: string,
  headers: ReplayKeyValueItem[],
  body: string | undefined,
  ruleConfig: RuleConfig,
  requestId?: string,
  timeoutMs?: number
): ReplayExecuteRequest {
  const enabledHeaders: [string, string][] = headers
    .filter(h => h.enabled && h.key.trim())
    .map(h => [h.key, h.value]);

  return {
    request: {
      method,
      url,
      headers: enabledHeaders,
      body: body || undefined,
    },
    rule_config: ruleConfig,
    request_id: requestId,
    timeout_ms: timeoutMs,
  };
}
