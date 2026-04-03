import { create } from "zustand";
import type { SyncStatus } from "../api/sync";
import { getSyncStatus } from "../api/sync";
import { isConnectionIssueError } from "../api/client";

const SYNC_POLL_INTERVAL_MS = 5000;
const LOGIN_COMPLETE_MESSAGE_TYPE = "bifrost-sync-login-complete";

interface SyncState {
  syncStatus: SyncStatus | null;
  loading: boolean;
  pollTimer: ReturnType<typeof setInterval> | null;
  messageCleanup: (() => void) | null;
  subscriberCount: number;

  fetchSyncStatus: () => Promise<void>;
  startPolling: () => void;
  stopPolling: () => void;
}

export const useSyncStore = create<SyncState>((set, get) => ({
  syncStatus: null,
  loading: false,
  pollTimer: null,
  messageCleanup: null,
  subscriberCount: 0,

  fetchSyncStatus: async () => {
    try {
      const status = await getSyncStatus();
      set({ syncStatus: status });
    } catch (error) {
      if (!isConnectionIssueError(error)) {
        set({ syncStatus: null });
      }
    }
  },

  startPolling: () => {
    const state = get();
    const nextCount = state.subscriberCount + 1;
    set({ subscriberCount: nextCount });

    if (state.pollTimer) return;

    void state.fetchSyncStatus();

    const timer = setInterval(() => {
      void get().fetchSyncStatus();
    }, SYNC_POLL_INTERVAL_MS);

    const onMessage = (event: MessageEvent) => {
      if (
        event.origin === window.location.origin &&
        event.data?.type === LOGIN_COMPLETE_MESSAGE_TYPE
      ) {
        void get().fetchSyncStatus();
      }
    };
    window.addEventListener("message", onMessage);

    set({
      pollTimer: timer,
      messageCleanup: () => window.removeEventListener("message", onMessage),
    });
  },

  stopPolling: () => {
    const state = get();
    const nextCount = Math.max(0, state.subscriberCount - 1);
    set({ subscriberCount: nextCount });

    if (nextCount > 0) return;

    if (state.pollTimer) {
      clearInterval(state.pollTimer);
    }
    if (state.messageCleanup) {
      state.messageCleanup();
    }
    set({ pollTimer: null, messageCleanup: null });
  },
}));
