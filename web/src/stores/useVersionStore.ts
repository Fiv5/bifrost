import { create } from "zustand";
import { persist } from "zustand/middleware";
import { checkVersion as checkVersionApi } from "../api/version";
import type { VersionCheckResponse } from "../types";

const SEEN_VERSIONS_STORAGE_KEY = "bifrost-seen-versions";
const CHECK_INTERVAL_MS = 60 * 60 * 1000;

interface VersionState {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string | null;
  releaseHighlights: string[];
  releaseUrl: string | null;
  loading: boolean;
  lastChecked: number | null;
  seenVersions: string[];
  modalVisible: boolean;

  checkVersion: (forceRefresh?: boolean) => Promise<void>;
  markVersionSeen: (version: string) => void;
  isVersionSeen: (version: string) => boolean;
  setModalVisible: (visible: boolean) => void;
  shouldShowAutoModal: () => boolean;
}

export const useVersionStore = create<VersionState>()(
  persist(
    (set, get) => ({
      hasUpdate: false,
      currentVersion: "",
      latestVersion: null,
      releaseHighlights: [],
      releaseUrl: null,
      loading: false,
      lastChecked: null,
      seenVersions: [],
      modalVisible: false,

      checkVersion: async (forceRefresh = false) => {
        const state = get();

        if (!forceRefresh && state.lastChecked) {
          const elapsed = Date.now() - state.lastChecked;
          if (elapsed < CHECK_INTERVAL_MS) {
            return;
          }
        }

        set({ loading: true });

        try {
          const response: VersionCheckResponse = await checkVersionApi(forceRefresh);
          set({
            hasUpdate: response.has_update,
            currentVersion: response.current_version,
            latestVersion: response.latest_version,
            releaseHighlights: response.release_highlights,
            releaseUrl: response.release_url,
            lastChecked: Date.now(),
            loading: false,
          });
        } catch (error) {
          console.error("Failed to check version:", error);
          set({ loading: false });
        }
      },

      markVersionSeen: (version: string) => {
        const state = get();
        if (!state.seenVersions.includes(version)) {
          set({ seenVersions: [...state.seenVersions, version] });
        }
      },

      isVersionSeen: (version: string) => {
        return get().seenVersions.includes(version);
      },

      setModalVisible: (visible: boolean) => {
        set({ modalVisible: visible });
      },

      shouldShowAutoModal: () => {
        const state = get();
        if (!state.hasUpdate || !state.latestVersion) {
          return false;
        }
        return !state.seenVersions.includes(state.latestVersion);
      },
    }),
    {
      name: SEEN_VERSIONS_STORAGE_KEY,
      partialize: (state) => ({
        seenVersions: state.seenVersions,
        lastChecked: state.lastChecked,
      }),
    },
  ),
);
