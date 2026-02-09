import { create } from 'zustand';
import type { MetricsSnapshot, SystemOverview } from '../types';
import * as api from '../api';

interface MetricsState {
  current: MetricsSnapshot | null;
  history: MetricsSnapshot[];
  overview: SystemOverview | null;
  loading: boolean;
  error: string | null;
  fetchMetrics: () => Promise<void>;
  fetchHistory: (limit?: number) => Promise<void>;
  fetchOverview: () => Promise<void>;
  clearError: () => void;
}

export const useMetricsStore = create<MetricsState>((set) => ({
  current: null,
  history: [],
  overview: null,
  loading: false,
  error: null,

  fetchMetrics: async () => {
    try {
      const metrics = await api.getMetrics();
      set({ current: metrics });
    } catch (e) {
      set({ error: (e as Error).message });
    }
  },

  fetchHistory: async (limit?: number) => {
    set({ loading: true, error: null });
    try {
      const history = await api.getMetricsHistory(limit);
      set({ history, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  fetchOverview: async () => {
    set({ loading: true, error: null });
    try {
      const overview = await api.getSystemOverview();
      set({ overview, current: overview.metrics, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  clearError: () => set({ error: null }),
}));
