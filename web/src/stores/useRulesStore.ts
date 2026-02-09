import { create } from 'zustand';
import type { RuleFile, RuleFileDetail } from '../types';
import * as api from '../api';

interface RulesState {
  rules: RuleFile[];
  currentRule: RuleFileDetail | null;
  loading: boolean;
  error: string | null;
  fetchRules: () => Promise<void>;
  fetchRule: (name: string) => Promise<void>;
  createRule: (name: string, content: string) => Promise<boolean>;
  updateRule: (name: string, content?: string, enabled?: boolean) => Promise<boolean>;
  deleteRule: (name: string) => Promise<boolean>;
  toggleRule: (name: string, enabled: boolean) => Promise<boolean>;
  clearError: () => void;
}

export const useRulesStore = create<RulesState>((set, get) => ({
  rules: [],
  currentRule: null,
  loading: false,
  error: null,

  fetchRules: async () => {
    set({ loading: true, error: null });
    try {
      const rules = await api.getRules();
      set({ rules, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  fetchRule: async (name: string) => {
    set({ loading: true, error: null });
    try {
      const rule = await api.getRule(name);
      set({ currentRule: rule, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  createRule: async (name: string, content: string) => {
    set({ loading: true, error: null });
    try {
      await api.createRule(name, content);
      await get().fetchRules();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  updateRule: async (name: string, content?: string, enabled?: boolean) => {
    set({ loading: true, error: null });
    try {
      await api.updateRule(name, content, enabled);
      await get().fetchRules();
      if (get().currentRule?.name === name) {
        await get().fetchRule(name);
      }
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  deleteRule: async (name: string) => {
    set({ loading: true, error: null });
    try {
      await api.deleteRule(name);
      await get().fetchRules();
      if (get().currentRule?.name === name) {
        set({ currentRule: null });
      }
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  toggleRule: async (name: string, enabled: boolean) => {
    set({ loading: true, error: null });
    try {
      if (enabled) {
        await api.enableRule(name);
      } else {
        await api.disableRule(name);
      }
      await get().fetchRules();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  clearError: () => set({ error: null }),
}));
