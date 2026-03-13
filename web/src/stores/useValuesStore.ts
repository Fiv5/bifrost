import { create } from 'zustand';
import type { ValueItem } from '../api/values';
import * as api from '../api';
import { isConnectionIssueError } from '../api/client';

interface ValuesState {
  values: ValueItem[];
  currentValue: ValueItem | null;
  selectedValueName: string | null;
  editingContent: Record<string, string>;
  searchKeyword: string;
  loading: boolean;
  saving: boolean;
  error: string | null;
  fetchValues: () => Promise<void>;
  selectValue: (name: string | null) => void;
  createValue: (name: string, value: string) => Promise<boolean>;
  updateValue: (name: string, value: string) => Promise<boolean>;
  saveCurrentValue: () => Promise<boolean>;
  deleteValue: (name: string) => Promise<boolean>;
  renameValue: (oldName: string, newName: string) => Promise<boolean>;
  setEditingContent: (name: string, content: string) => void;
  setSearchKeyword: (keyword: string) => void;
  hasUnsavedChanges: (name: string) => boolean;
  clearError: () => void;
}

export const useValuesStore = create<ValuesState>((set, get) => ({
  values: [],
  currentValue: null,
  selectedValueName: null,
  editingContent: {},
  searchKeyword: '',
  loading: false,
  saving: false,
  error: null,

  fetchValues: async () => {
    set({ loading: true, error: null });
    try {
      const response = await api.getValues();
      set({ values: response.values, loading: false });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
    }
  },

  selectValue: (name: string | null) => {
    if (!name) {
      set({ selectedValueName: null, currentValue: null });
      return;
    }
    const { values } = get();
    const value = values.find((v) => v.name === name);
    set({ selectedValueName: name, currentValue: value || null });
  },

  createValue: async (name: string, value: string) => {
    set({ loading: true, error: null });
    try {
      await api.createValue(name, value);
      await get().fetchValues();
      set({ selectedValueName: name });
      get().selectValue(name);
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  updateValue: async (name: string, value: string) => {
    set({ loading: true, error: null });
    try {
      await api.updateValue(name, value);
      await get().fetchValues();
      if (get().currentValue?.name === name) {
        get().selectValue(name);
      }
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  saveCurrentValue: async () => {
    const { selectedValueName, editingContent, currentValue } = get();
    if (!selectedValueName) return false;

    const content = editingContent[selectedValueName];
    if (content === undefined || content === currentValue?.value) {
      return true;
    }

    set({ saving: true, error: null });
    try {
      await api.updateValue(selectedValueName, content);
      await get().fetchValues();
      const newEditingContent = { ...get().editingContent };
      delete newEditingContent[selectedValueName];
      set({ editingContent: newEditingContent, saving: false });
      get().selectValue(selectedValueName);
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, saving: false });
      return false;
    }
  },

  deleteValue: async (name: string) => {
    set({ loading: true, error: null });
    try {
      await api.deleteValue(name);
      const { selectedValueName, editingContent } = get();
      const newEditingContent = { ...editingContent };
      delete newEditingContent[name];

      if (selectedValueName === name) {
        await get().fetchValues();
        const values = get().values;
        const nextValue = values.length > 0 ? values[0].name : null;
        set({
          selectedValueName: nextValue,
          currentValue: null,
          editingContent: newEditingContent,
        });
        if (nextValue) {
          get().selectValue(nextValue);
        }
      } else {
        await get().fetchValues();
        set({ editingContent: newEditingContent });
      }
      return true;
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message, loading: false });
      return false;
    }
  },

  renameValue: async (oldName: string, newName: string) => {
    set({ loading: true, error: null });
    try {
      const { values, editingContent } = get();
      const oldValue = values.find((v) => v.name === oldName);
      if (!oldValue) {
        set({ error: 'Value not found', loading: false });
        return false;
      }

      const content = editingContent[oldName] ?? oldValue.value;
      await api.deleteValue(oldName);
      await api.createValue(newName, content);

      const { selectedValueName } = get();
      const newEditingContent = { ...editingContent };
      if (newEditingContent[oldName] !== undefined) {
        newEditingContent[newName] = newEditingContent[oldName];
        delete newEditingContent[oldName];
      }

      await get().fetchValues();
      if (selectedValueName === oldName) {
        set({
          selectedValueName: newName,
          editingContent: newEditingContent,
        });
        get().selectValue(newName);
      } else {
        set({ editingContent: newEditingContent });
      }
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
    const { editingContent, values } = get();
    const edited = editingContent[name];
    if (edited === undefined) return false;
    const value = values.find((v) => v.name === name);
    if (!value) return false;
    return edited !== value.value;
  },

  clearError: () => set({ error: null }),
}));
