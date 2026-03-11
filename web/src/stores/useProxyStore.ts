import { create } from "zustand";
import type { CliProxyStatus, SystemProxyStatus } from "../api/proxy";
import { getCliProxyStatus, getSystemProxyStatus, setSystemProxy } from "../api/proxy";
import { isConnectionIssueError } from "../api/client";

interface ProxyState {
  systemProxy: SystemProxyStatus | null;
  cliProxy: CliProxyStatus | null;
  loading: boolean;
  error: string | null;
  fetchSystemProxy: () => Promise<void>;
  fetchCliProxy: () => Promise<void>;
  toggleSystemProxy: (enabled: boolean) => Promise<boolean>;
  clearError: () => void;
}

export const useProxyStore = create<ProxyState>((set, get) => ({
  systemProxy: null,
  cliProxy: null,
  loading: false,
  error: null,

  fetchSystemProxy: async () => {
    try {
      const status = await getSystemProxyStatus();
      set({ systemProxy: status, error: null });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message });
    }
  },

  fetchCliProxy: async () => {
    try {
      const status = await getCliProxyStatus();
      set({ cliProxy: status, error: null });
    } catch (e) {
      set({ error: isConnectionIssueError(e) ? null : (e as Error).message });
    }
  },

  toggleSystemProxy: async (enabled: boolean) => {
    const currentState = get().systemProxy;
    set({ loading: true, error: null });
    try {
      const result = await setSystemProxy({ enabled });
      set({ systemProxy: result, loading: false });
      return true;
    } catch (e) {
      set({
        error: isConnectionIssueError(e) ? null : (e as Error).message,
        loading: false,
        systemProxy: currentState,
      });
      return false;
    }
  },

  clearError: () => set({ error: null }),
}));
