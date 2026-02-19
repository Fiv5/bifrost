import { create } from 'zustand';
import type { ValueItem } from '../api/values';
import * as api from '../api';

interface ValuesState {
  values: ValueItem[];
  loading: boolean;
  error: string | null;
  searchText: string;
  fetchValues: () => Promise<void>;
  createValue: (name: string, value: string) => Promise<boolean>;
  updateValue: (name: string, value: string) => Promise<boolean>;
  deleteValue: (name: string) => Promise<boolean>;
  clearError: () => void;
  setSearchText: (text: string) => void;
}

export const useValuesStore = create<ValuesState>((set, get) => ({
  values: [],
  loading: false,
  error: null,
  searchText: '',

  fetchValues: async () => {
    set({ loading: true, error: null });
    try {
      const response = await api.getValues();
      set({ values: response.values, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  createValue: async (name: string, value: string) => {
    set({ loading: true, error: null });
    try {
      await api.createValue(name, value);
      await get().fetchValues();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  updateValue: async (name: string, value: string) => {
    set({ loading: true, error: null });
    try {
      await api.updateValue(name, value);
      await get().fetchValues();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  deleteValue: async (name: string) => {
    set({ loading: true, error: null });
    try {
      await api.deleteValue(name);
      await get().fetchValues();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  clearError: () => set({ error: null }),

  setSearchText: (text: string) => set({ searchText: text }),
}));
