import { create } from 'zustand';
import type { MetricsSnapshot, SystemOverview } from '../types';
import * as api from '../api';
import pushService, {
  type OverviewData,
  type MetricsData,
  type HistoryData,
} from '../services/pushService';

interface MetricsState {
  current: MetricsSnapshot | null;
  history: MetricsSnapshot[];
  overview: SystemOverview | null;
  loading: boolean;
  error: string | null;
  usePush: boolean;
  pushUnsubscribes: (() => void)[];
  fetchMetrics: () => Promise<void>;
  fetchHistory: (limit?: number) => Promise<void>;
  fetchOverview: () => Promise<void>;
  clearError: () => void;
  enablePush: (options?: { needOverview?: boolean; needMetrics?: boolean; needHistory?: boolean; historyLimit?: number }) => void;
  disablePush: () => void;
  handleOverviewPush: (data: OverviewData) => void;
  handleMetricsPush: (data: MetricsData) => void;
  handleHistoryPush: (data: HistoryData) => void;
}

export const useMetricsStore = create<MetricsState>((set, get) => ({
  current: null,
  history: [],
  overview: null,
  loading: false,
  error: null,
  usePush: true,
  pushUnsubscribes: [],

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

  enablePush: (options = {}) => {
    const state = get();
    if (state.pushUnsubscribes.length > 0) return;

    const {
      needOverview = true,
      needMetrics = true,
      needHistory = false,
      historyLimit = 3600,
    } = options;

    const subscription = {
      need_overview: needOverview,
      need_metrics: needMetrics,
      need_history: needHistory,
      history_limit: historyLimit,
    };

    pushService.connect(subscription);

    const unsubscribes: (() => void)[] = [];

    if (needOverview) {
      unsubscribes.push(
        pushService.onOverviewUpdate((data) => {
          get().handleOverviewPush(data);
        })
      );
    }

    if (needMetrics) {
      unsubscribes.push(
        pushService.onMetricsUpdate((data) => {
          get().handleMetricsPush(data);
        })
      );
    }

    if (needHistory) {
      unsubscribes.push(
        pushService.onHistoryUpdate((data) => {
          get().handleHistoryPush(data);
        })
      );
    }

    set({ pushUnsubscribes: unsubscribes });
  },

  disablePush: () => {
    const state = get();
    state.pushUnsubscribes.forEach((unsub) => unsub());
    set({ pushUnsubscribes: [] });
  },

  handleOverviewPush: (data: OverviewData) => {
    const overview: SystemOverview = {
      system: data.system,
      metrics: data.metrics,
      rules: data.rules,
      traffic: data.traffic,
      server: data.server,
      pending_authorizations: data.pending_authorizations,
    };
    set({ overview, current: data.metrics });
  },

  handleMetricsPush: (data: MetricsData) => {
    set({ current: data.metrics });
  },

  handleHistoryPush: (data: HistoryData) => {
    set({ history: data.history });
  },
}));
