import axios, { AxiosError } from 'axios';
import type { AxiosRequestConfig } from 'axios';
import { message } from 'antd';
import { getClientId } from '../services/clientId';
import { buildApiUrl } from '../runtime';
import {
  isDesktopCoreTransitionActive,
  useDesktopCoreStore,
} from '../stores/useDesktopCoreStore';

const API_BASE = buildApiUrl();

const client = axios.create({
  baseURL: API_BASE,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

const GLOBAL_API_MESSAGE_KEY = 'global-api-error';

type ApiErrorPayload = {
  kind: 'connection' | 'business';
  message: string;
  status?: number;
};

function extractErrorMessage(error: AxiosError): string {
  if (
    error.response?.data &&
    typeof error.response.data === 'object' &&
    'error' in error.response.data
  ) {
    return String((error.response.data as { error?: string }).error || error.message);
  }

  if (typeof error.response?.data === 'string' && error.response.data.trim()) {
    return error.response.data;
  }

  return error.message;
}

function isReadRequest(method?: string): boolean {
  const normalized = method?.toUpperCase();
  return normalized === 'GET' || normalized === 'HEAD';
}

export function getApiErrorPayload(error: unknown): ApiErrorPayload {
  if (!axios.isAxiosError(error)) {
    return {
      kind: 'business',
      message: error instanceof Error ? error.message : String(error),
    };
  }

  const status = error.response?.status;
  const resolvedMessage = extractErrorMessage(error);
  const requestMethod = error.config?.method;
  const readyOnce = useDesktopCoreStore.getState().readyOnce;
  const isGenericServerFailure =
    !!status &&
    status >= 500 &&
    (!error.response?.data ||
      resolvedMessage === error.message ||
      resolvedMessage === `Request failed with status code ${status}`);

  const isConnectionIssue =
    !error.response ||
    error.code === 'ERR_NETWORK' ||
    error.code === 'ECONNABORTED' ||
    (!readyOnce && !!status && status >= 500 && isReadRequest(requestMethod)) ||
    isGenericServerFailure;

  return {
    kind: isConnectionIssue ? 'connection' : 'business',
    message: isConnectionIssue
      ? 'Bifrost core is starting. Reconnecting the interface...'
      : resolvedMessage,
    status,
  };
}

export function isConnectionIssueError(error: unknown): boolean {
  return getApiErrorPayload(error).kind === 'connection';
}

export function normalizeApiErrorMessage(
  error: unknown,
  fallback = 'Request failed',
): string {
  const payload = getApiErrorPayload(error);
  return payload.message || fallback;
}

export function notifyApiBusinessError(
  error: unknown,
  fallback = 'Request failed',
): void {
  const payload = getApiErrorPayload(error);
  if (payload.kind === 'connection') {
    return;
  }

  message.open({
    key: GLOBAL_API_MESSAGE_KEY,
    type: 'error',
    content: payload.message || fallback,
  });
}

client.interceptors.request.use((config) => {
  config.headers = config.headers ?? {};
  config.headers['X-Client-Id'] = getClientId();
  return config;
});

client.interceptors.response.use(
  (response) => {
    useDesktopCoreStore.getState().markReady();
    useDesktopCoreStore.getState().resolveBooting();
    return response;
  },
  (error: AxiosError) => {
    const payload = getApiErrorPayload(error);

    if (payload.kind === 'connection') {
      useDesktopCoreStore.getState().showBooting(payload.message);
      return Promise.reject(error);
    }

    if (!isDesktopCoreTransitionActive()) {
      console.error('[API Error]', payload.message);
    }
    return Promise.reject(error);
  }
);

export async function get<T>(url: string, config?: AxiosRequestConfig): Promise<T> {
  const response = await client.get<T>(url, config);
  return response.data;
}

export async function post<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
  const response = await client.post<T>(url, data, config);
  return response.data;
}

export async function put<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
  const response = await client.put<T>(url, data, config);
  return response.data;
}

export async function del<T>(url: string, data?: unknown, config?: AxiosRequestConfig): Promise<T> {
  const response = await client.delete<T>(url, { ...config, data });
  return response.data;
}

export default client;
