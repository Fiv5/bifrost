import { create } from 'zustand';
import type { WhitelistStatus, AccessMode } from '../types';
import * as api from '../api';

interface WhitelistState {
  status: WhitelistStatus | null;
  loading: boolean;
  error: string | null;
  fetchStatus: () => Promise<void>;
  addToWhitelist: (ipOrCidr: string) => Promise<boolean>;
  removeFromWhitelist: (ipOrCidr: string) => Promise<boolean>;
  setMode: (mode: AccessMode) => Promise<boolean>;
  setAllowLan: (allow: boolean) => Promise<boolean>;
  addTemporary: (ip: string) => Promise<boolean>;
  removeTemporary: (ip: string) => Promise<boolean>;
  clearError: () => void;
}

export const useWhitelistStore = create<WhitelistState>((set, get) => ({
  status: null,
  loading: false,
  error: null,

  fetchStatus: async () => {
    set({ loading: true, error: null });
    try {
      const status = await api.getWhitelistStatus();
      set({ status, loading: false });
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
    }
  },

  addToWhitelist: async (ipOrCidr: string) => {
    set({ loading: true, error: null });
    try {
      await api.addToWhitelist(ipOrCidr);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  removeFromWhitelist: async (ipOrCidr: string) => {
    set({ loading: true, error: null });
    try {
      await api.removeFromWhitelist(ipOrCidr);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setMode: async (mode: AccessMode) => {
    set({ loading: true, error: null });
    try {
      await api.setAccessMode(mode);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  setAllowLan: async (allow: boolean) => {
    set({ loading: true, error: null });
    try {
      await api.setAllowLan(allow);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  addTemporary: async (ip: string) => {
    set({ loading: true, error: null });
    try {
      await api.addTemporary(ip);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  removeTemporary: async (ip: string) => {
    set({ loading: true, error: null });
    try {
      await api.removeTemporary(ip);
      await get().fetchStatus();
      return true;
    } catch (e) {
      set({ error: (e as Error).message, loading: false });
      return false;
    }
  },

  clearError: () => set({ error: null }),
}));
