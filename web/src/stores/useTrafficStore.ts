import { create } from 'zustand';
import type { TrafficSummary, TrafficRecord, TrafficFilter } from '../types';
import * as api from '../api';

interface TrafficState {
  records: TrafficSummary[];
  currentRecord: TrafficRecord | null;
  total: number;
  filter: TrafficFilter;
  loading: boolean;
  error: string | null;
  fetchTraffic: () => Promise<void>;
  fetchTrafficDetail: (id: string) => Promise<void>;
  clearTraffic: () => Promise<boolean>;
  setFilter: (filter: Partial<TrafficFilter>) => void;
  clearError: () => void;
}

export const useTrafficStore = create<TrafficState>((set, get) => ({
  records: [],
  currentRecord: null,
  total: 0,
  filter: { limit: 100, offset: 0 },
  loading: false,
  error: null,

  fetchTraffic: async () => {
    set({ loading: true, error: null });
    try {
      const response = await api.getTrafficList(get().filter);
      set({ 
        records: response.records, 
        total: response.total, 
        loading: false 
      });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  fetchTrafficDetail: async (id: string) => {
    set({ loading: true, error: null });
    try {
      const record = await api.getTrafficDetail(id);
      set({ currentRecord: record, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  clearTraffic: async () => {
    set({ loading: true, error: null });
    try {
      await api.clearTraffic();
      set({ records: [], total: 0, currentRecord: null, loading: false });
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setFilter: (filter: Partial<TrafficFilter>) => {
    set((state) => ({
      filter: { ...state.filter, ...filter },
    }));
  },

  clearError: () => set({ error: null }),
}));
