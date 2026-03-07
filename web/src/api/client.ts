import axios, { AxiosError } from 'axios';
import type { AxiosRequestConfig } from 'axios';
import { getClientId } from '../services/clientId';

const API_BASE = '/_bifrost/api';

const client = axios.create({
  baseURL: API_BASE,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

client.interceptors.request.use((config) => {
  config.headers = config.headers ?? {};
  config.headers['X-Client-Id'] = getClientId();
  return config;
});

client.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    const message = error.response?.data
      ? (error.response.data as { error?: string }).error || error.message
      : error.message;
    console.error('[API Error]', message);
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
