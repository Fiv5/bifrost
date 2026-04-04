import { create } from 'zustand';
import type { RuleFile, RuleFileDetail } from '../types';
import * as api from '../api';
import { isConnectionIssueError } from '../api/client';
import {
  fetchGroupRules,
  getGroupRule,
  createGroupRule,
  updateGroupRule,
  deleteGroupRule,
  enableGroupRule,
  disableGroupRule,
  type GroupRuleInfo,
} from '../api/group';

function sortRulesByManualOrder(rules: RuleFile[]): RuleFile[] {
  return [...rules].sort((left, right) => {
    return left.sort_order - right.sort_order || left.name.localeCompare(right.name);
  });
}

function groupRuleToRuleFile(info: GroupRuleInfo): RuleFile {
  return {
    name: info.name,
    enabled: info.enabled,
    sort_order: info.sort_order,
    rule_count: info.rule_count,
    created_at: info.created_at,
    updated_at: info.updated_at,
  };
}

interface RulesState {
  rules: RuleFile[];
  currentRule: RuleFileDetail | null;
  selectedRuleName: string | null;
  editingContent: Record<string, string>;
  searchKeyword: string;
  loading: boolean;
  saving: boolean;
  error: string | null;
  activeGroupId: string | null;
  isGroupMode: boolean;
  groupWritable: boolean;
  setActiveGroupId: (groupId: string | null) => void;
  fetchRules: () => Promise<void>;
  fetchRule: (name: string) => Promise<void>;
  selectRule: (name: string | null) => Promise<void>;
  createRule: (name: string, content: string) => Promise<boolean>;
  updateRule: (name: string, content?: string, enabled?: boolean) => Promise<boolean>;
  saveCurrentRule: () => Promise<boolean>;
  deleteRule: (name: string) => Promise<boolean>;
  toggleRule: (name: string, enabled: boolean) => Promise<boolean>;
  renameRule: (oldName: string, newName: string) => Promise<boolean>;
  reorderRules: (order: string[]) => Promise<boolean>;
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
  activeGroupId: null,
  isGroupMode: false,
  groupWritable: false,

  setActiveGroupId: (groupId: string | null) => {
    set({
      activeGroupId: groupId,
      isGroupMode: groupId !== null,
      groupWritable: false,
      rules: [],
      selectedRuleName: null,
      currentRule: null,
      editingContent: {},
    });
  },

  fetchRules: async () => {
    const { activeGroupId } = get();
    set({ loading: true, error: null });

    if (!activeGroupId) {
      try {
        const rules = await api.getRules();
        set({ rules: sortRulesByManualOrder(rules), loading: false, isGroupMode: false, groupWritable: false });
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      }
      return;
    }

    try {
      const resp = await fetchGroupRules(activeGroupId);
      const ruleFiles = resp.rules.map(groupRuleToRuleFile);
      set({
        rules: ruleFiles,
        loading: false,
        isGroupMode: true,
        groupWritable: resp.writable,
      });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
    }
  },

  fetchRule: async (name: string) => {
    const { isGroupMode, activeGroupId } = get();
    set({ loading: true, error: null });

    if (isGroupMode && activeGroupId) {
      try {
        const detail = await getGroupRule(activeGroupId, name);
        set({
          currentRule: {
            name: detail.name,
            content: detail.content,
            enabled: detail.enabled,
            sort_order: detail.sort_order,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            sync: detail.sync,
          },
          loading: false,
        });
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      }
      return;
    }

    try {
      const rule = await api.getRule(name);
      set({ currentRule: rule, loading: false });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
    }
  },

  selectRule: async (name: string | null) => {
    if (!name) {
      set({ selectedRuleName: null, currentRule: null });
      return;
    }
    const { isGroupMode, activeGroupId } = get();
    set({ selectedRuleName: name, loading: true, error: null });

    if (isGroupMode && activeGroupId) {
      try {
        const detail = await getGroupRule(activeGroupId, name);
        set({
          currentRule: {
            name: detail.name,
            content: detail.content,
            enabled: detail.enabled,
            sort_order: detail.sort_order,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            sync: detail.sync,
          },
          loading: false,
        });
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      }
      return;
    }

    try {
      const rule = await api.getRule(name);
      set({ currentRule: rule, loading: false });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
    }
  },

  createRule: async (name: string, content: string) => {
    const { isGroupMode, groupWritable, activeGroupId } = get();

    if (isGroupMode) {
      if (!groupWritable || !activeGroupId) return false;
      set({ loading: true, error: null });
      try {
        const detail = await createGroupRule(activeGroupId, name, content);
        await get().fetchRules();
        set({ selectedRuleName: detail.name });
        await get().selectRule(detail.name);
        return true;
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
        return false;
      }
    }

    set({ loading: true, error: null });
    try {
      await api.createRule(name, content);
      await get().fetchRules();
      set({ selectedRuleName: name });
      await get().selectRule(name);
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
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
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  saveCurrentRule: async () => {
    const { selectedRuleName, editingContent, currentRule, isGroupMode, groupWritable, activeGroupId } = get();
    if (!selectedRuleName) return false;

    const content = editingContent[selectedRuleName];
    if (content === undefined || content === currentRule?.content) {
      return true;
    }

    set({ saving: true, error: null });

    if (isGroupMode) {
      if (!groupWritable || !activeGroupId) {
        set({ saving: false });
        return false;
      }
      try {
        const detail = await updateGroupRule(activeGroupId, selectedRuleName, content);
        const newEditingContent = { ...get().editingContent };
        delete newEditingContent[selectedRuleName];
        set({
          currentRule: {
            name: detail.name,
            content: detail.content,
            enabled: detail.enabled,
            sort_order: detail.sort_order,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            sync: detail.sync,
          },
          saving: false,
          editingContent: newEditingContent,
        });
        await get().fetchRules();
        return true;
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, saving: false });
        return false;
      }
    }

    try {
      await api.updateRule(selectedRuleName, content);
      await get().fetchRules();
      const rule = await api.getRule(selectedRuleName);
      const newEditingContent = { ...get().editingContent };
      delete newEditingContent[selectedRuleName];
      set({
        currentRule: rule,
        saving: false,
        editingContent: newEditingContent,
      });
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, saving: false });
      return false;
    }
  },

  deleteRule: async (name: string) => {
    const { isGroupMode, groupWritable, activeGroupId } = get();

    if (isGroupMode) {
      if (!groupWritable || !activeGroupId) return false;
      set({ loading: true, error: null });
      try {
        await deleteGroupRule(activeGroupId, name);
        await get().fetchRules();
        const { selectedRuleName } = get();
        if (selectedRuleName === name) {
          const rules = get().rules;
          const nextRule = rules.length > 0 ? rules[0].name : null;
          set({ selectedRuleName: nextRule, currentRule: null });
          if (nextRule) await get().selectRule(nextRule);
        }
        return true;
      } catch (e) {
        set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
        return false;
      }
    }

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
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  toggleRule: async (name: string, enabled: boolean) => {
    const { isGroupMode, activeGroupId } = get();
    const previousRules = get().rules;
    set({
      rules: previousRules.map((r) =>
        r.name === name ? { ...r, enabled } : r
      ),
    });
    try {
      if (isGroupMode && activeGroupId) {
        if (enabled) {
          await enableGroupRule(activeGroupId, name);
        } else {
          await disableGroupRule(activeGroupId, name);
        }
        const resp = await fetchGroupRules(activeGroupId);
        set({ rules: resp.rules.map(groupRuleToRuleFile) });
        if (get().currentRule?.name === name) {
          const detail = await getGroupRule(activeGroupId, name);
          set({
            currentRule: {
              name: detail.name,
              content: detail.content,
              enabled: detail.enabled,
              sort_order: detail.sort_order,
              created_at: detail.created_at,
              updated_at: detail.updated_at,
              sync: detail.sync,
            },
          });
        }
      } else {
        if (enabled) {
          await api.enableRule(name);
        } else {
          await api.disableRule(name);
        }
        const rules = await api.getRules();
        set({ rules: sortRulesByManualOrder(rules) });
        if (get().currentRule?.name === name) {
          const rule = await api.getRule(name);
          set({ currentRule: rule });
        }
      }
      return true;
    } catch (e) {
      set({ rules: previousRules, error: isConnectionIssueError(e) ? null : (e as Error).message });
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
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  reorderRules: async (order: string[]) => {
    set({ loading: true, error: null });
    try {
      await api.reorderRules(order);
      await get().fetchRules();
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
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
