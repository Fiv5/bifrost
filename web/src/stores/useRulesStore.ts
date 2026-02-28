import { create } from 'zustand';
import type { RuleFile, RuleFileDetail } from '../types';
import * as api from '../api';

interface RulesState {
  rules: RuleFile[];
  currentRule: RuleFileDetail | null;
  selectedRuleName: string | null;
  editingContent: Record<string, string>;
  searchKeyword: string;
  loading: boolean;
  saving: boolean;
  error: string | null;
  fetchRules: () => Promise<void>;
  fetchRule: (name: string) => Promise<void>;
  selectRule: (name: string | null) => Promise<void>;
  createRule: (name: string, content: string) => Promise<boolean>;
  updateRule: (name: string, content?: string, enabled?: boolean) => Promise<boolean>;
  saveCurrentRule: () => Promise<boolean>;
  deleteRule: (name: string) => Promise<boolean>;
  toggleRule: (name: string, enabled: boolean) => Promise<boolean>;
  renameRule: (oldName: string, newName: string) => Promise<boolean>;
  setEditingContent: (name: string, content: string) => void;
  setSearchKeyword: (keyword: string) => void;
  hasUnsavedChanges: (name: string) => boolean;
  clearError: () => void;
}

export const useRulesStore = create<RulesState>((set, get) => ({
  rules: [],
  currentRule: null,
  selectedRuleName: null,
  editingContent: {},
  searchKeyword: '',
  loading: false,
  saving: false,
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

  selectRule: async (name: string | null) => {
    if (!name) {
      set({ selectedRuleName: null, currentRule: null });
      return;
    }
    set({ selectedRuleName: name, loading: true, error: null });
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
      set({ selectedRuleName: name });
      await get().selectRule(name);
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

  saveCurrentRule: async () => {
    const { selectedRuleName, editingContent, currentRule } = get();
    if (!selectedRuleName) return false;

    const content = editingContent[selectedRuleName];
    if (content === undefined || content === currentRule?.content) {
      return true;
    }

    set({ saving: true, error: null });
    try {
      await api.updateRule(selectedRuleName, content);
      await get().fetchRules();
      const rule = await api.getRule(selectedRuleName);
      set((state) => ({
        currentRule: rule,
        saving: false,
        editingContent: {
          ...state.editingContent,
          [selectedRuleName]: undefined as unknown as string,
        },
      }));
      const newEditingContent = { ...get().editingContent };
      delete newEditingContent[selectedRuleName];
      set({ editingContent: newEditingContent });
      return true;
    } catch (e) {
      set({ error: (e as Error).message, saving: false });
      return false;
    }
  },

  deleteRule: async (name: string) => {
    set({ loading: true, error: null });
    try {
      await api.deleteRule(name);
      const { selectedRuleName, editingContent } = get();
      const newEditingContent = { ...editingContent };
      delete newEditingContent[name];

      if (selectedRuleName === name) {
        await get().fetchRules();
        const rules = get().rules;
        const nextRule = rules.length > 0 ? rules[0].name : null;
        set({
          selectedRuleName: nextRule,
          currentRule: null,
          editingContent: newEditingContent,
        });
        if (nextRule) {
          await get().selectRule(nextRule);
        }
      } else {
        await get().fetchRules();
        set({ editingContent: newEditingContent });
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

  renameRule: async (oldName: string, newName: string) => {
    set({ loading: true, error: null });
    try {
      await api.renameRule(oldName, newName);
      const { selectedRuleName, editingContent } = get();
      const newEditingContent = { ...editingContent };
      if (newEditingContent[oldName] !== undefined) {
        newEditingContent[newName] = newEditingContent[oldName];
        delete newEditingContent[oldName];
      }
      await get().fetchRules();
      if (selectedRuleName === oldName) {
        set({
          selectedRuleName: newName,
          editingContent: newEditingContent,
        });
        await get().selectRule(newName);
      } else {
        set({ editingContent: newEditingContent });
      }
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setEditingContent: (name: string, content: string) => {
    set((state) => ({
      editingContent: {
        ...state.editingContent,
        [name]: content,
      },
    }));
  },

  setSearchKeyword: (keyword: string) => {
    set({ searchKeyword: keyword });
  },

  hasUnsavedChanges: (name: string) => {
    const { editingContent, currentRule, rules } = get();
    const edited = editingContent[name];
    if (edited === undefined) return false;
    const rule = rules.find((r) => r.name === name);
    if (!rule) return false;
    if (currentRule?.name === name) {
      return edited !== currentRule.content;
    }
    return true;
  },

  clearError: () => set({ error: null }),
}));
