import { create } from "zustand";
import { persist } from "zustand/middleware";

const TRAFFIC_DETAIL_WINDOW_STORAGE_KEY = "bifrost-traffic-detail-window";
const TRAFFIC_DETAIL_WINDOW_SYNC_CHANNEL = "bifrost-traffic-detail-window-sync";

const detailWindowSyncChannel =
  typeof BroadcastChannel !== "undefined"
    ? new BroadcastChannel(TRAFFIC_DETAIL_WINDOW_SYNC_CHANNEL)
    : null;

interface TrafficDetailWindowState {
  detached: boolean;
  popupId: string | null;
  detach: (popupId: string) => void;
  attach: () => void;
}

type PersistedTrafficDetailWindowState = Pick<
  TrafficDetailWindowState,
  "detached" | "popupId"
>;

export const useTrafficDetailWindowStore = create<TrafficDetailWindowState>()(
  persist(
    (set) => ({
      detached: false,
      popupId: null,
      detach: (popupId) => set({ detached: true, popupId }),
      attach: () => set({ detached: false, popupId: null }),
    }),
    {
      name: TRAFFIC_DETAIL_WINDOW_STORAGE_KEY,
      partialize: (state) => ({
        detached: state.detached,
        popupId: state.popupId,
      }),
      version: 1,
    },
  ),
);

function isPersistedState(
  value: unknown,
): value is { state: PersistedTrafficDetailWindowState } {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as { state?: PersistedTrafficDetailWindowState };
  return !!candidate.state && typeof candidate.state === "object";
}

let isApplyingExternalState = false;
let syncInitialized = false;

function applyExternalState(nextState: PersistedTrafficDetailWindowState) {
  isApplyingExternalState = true;
  useTrafficDetailWindowStore.setState({
    detached: nextState.detached,
    popupId: nextState.popupId,
  });
  isApplyingExternalState = false;
}

function initializeTrafficDetailWindowSync() {
  if (syncInitialized || typeof window === "undefined") {
    return;
  }
  syncInitialized = true;

  useTrafficDetailWindowStore.subscribe((state, prevState) => {
    if (
      isApplyingExternalState ||
      (state.detached === prevState.detached && state.popupId === prevState.popupId)
    ) {
      return;
    }

    detailWindowSyncChannel?.postMessage({
      type: "detail-window-state",
      state: {
        detached: state.detached,
        popupId: state.popupId,
      },
    });
  });

  detailWindowSyncChannel?.addEventListener("message", (event: MessageEvent) => {
    const data = event.data as
      | { type?: string; state?: PersistedTrafficDetailWindowState }
      | undefined;
    if (data?.type !== "detail-window-state" || !data.state) {
      return;
    }
    applyExternalState(data.state);
  });

  window.addEventListener("storage", (event: StorageEvent) => {
    if (
      event.key !== TRAFFIC_DETAIL_WINDOW_STORAGE_KEY ||
      !event.newValue
    ) {
      return;
    }

    try {
      const parsed = JSON.parse(event.newValue) as unknown;
      if (!isPersistedState(parsed)) {
        return;
      }
      applyExternalState(parsed.state);
    } catch {
      // Ignore malformed persisted snapshots.
    }
  });
}

initializeTrafficDetailWindowSync();
