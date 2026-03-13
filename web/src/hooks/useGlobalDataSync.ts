import { useEffect, useRef } from 'react';
import { useTrafficStore } from '../stores/useTrafficStore';
import { useProxyStore } from '../stores/useProxyStore';
import { useFilterPanelStore } from '../stores/useFilterPanelStore';
import { useMetricsStore } from '../stores/useMetricsStore';
import { useVersionStore } from '../stores/useVersionStore';
import { syncDynamicData } from './useEditorCompletion';
import pushService from '../services/pushService';
import { useForceRefreshStore } from '../stores/useForceRefreshStore';
import { usePendingAuthStore } from '../stores/usePendingAuthStore';

const VERSION_CHECK_INTERVAL = 60 * 60 * 1000;

interface GlobalDataSyncState {
  initialized: boolean;
  versionCheckIntervalId: number | null;
  visibilityPaused: boolean;
  forceRefresh: boolean;
}

const globalState: GlobalDataSyncState = {
  initialized: false,
  versionCheckIntervalId: null,
  visibilityPaused: false,
  forceRefresh: false,
};

export function useGlobalDataSync() {
  const initRef = useRef(false);

  useEffect(() => {
    if (initRef.current || globalState.initialized) {
      return;
    }
    initRef.current = true;
    globalState.initialized = true;

    const trafficStore = useTrafficStore.getState();
    const proxyStore = useProxyStore.getState();
    const filterPanelStore = useFilterPanelStore.getState();
    const metricsStore = useMetricsStore.getState();
    const versionStore = useVersionStore.getState();

    const pauseRealtime = () => {
      if (globalState.visibilityPaused) return;
      globalState.visibilityPaused = true;
      useTrafficStore.getState().stopPolling();
      useMetricsStore.getState().disablePush();
    };

    const resumeRealtime = () => {
      if (globalState.forceRefresh) {
        return;
      }
      if (!globalState.visibilityPaused) return;
      globalState.visibilityPaused = false;
      useTrafficStore.getState().startPolling();
      useMetricsStore.getState().enablePush({
        needOverview: true,
        needMetrics: true,
      });
    };

    const onVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        pauseRealtime();
      } else {
        resumeRealtime();
      }
    };

    const onPageHide = () => pauseRealtime();
    const onPageShow = () => resumeRealtime();

    const stopAllPolling = () => {
      if (globalState.versionCheckIntervalId) {
        clearInterval(globalState.versionCheckIntervalId);
        globalState.versionCheckIntervalId = null;
      }
    };

    const onForceRefresh = (data: { reason: string }) => {
      if (globalState.forceRefresh) return;
      globalState.forceRefresh = true;
      stopAllPolling();
      pauseRealtime();
      usePendingAuthStore.getState().stopSSE();
      pushService.disableReconnectUntilRefresh();
      useForceRefreshStore.getState().show(data.reason);
    };

    const initializeGlobalData = async () => {
      await Promise.allSettled([
        trafficStore.fetchInitialData(),
        proxyStore.fetchSystemProxy(),
        proxyStore.fetchCliProxy(),
        filterPanelStore.loadFromServer(),
        metricsStore.fetchOverview(),
        versionStore.checkVersion({ skipCache: true }),
      ]);

      if (globalState.forceRefresh) {
        return;
      }

      trafficStore.startPolling();

      metricsStore.enablePush({
        needOverview: true,
        needMetrics: true,
      });

      if (globalState.forceRefresh) {
        return;
      }

      globalState.versionCheckIntervalId = window.setInterval(() => {
        useVersionStore.getState().checkVersion({ skipCache: true });
      }, VERSION_CHECK_INTERVAL);

      syncDynamicData();

      const currentVersionStore = useVersionStore.getState();
      if (currentVersionStore.hasUpdate) {
        currentVersionStore.setModalVisible(true);
      }
    };

    initializeGlobalData();

    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('pagehide', onPageHide);
    window.addEventListener('pageshow', onPageShow);
    const unsubscribeForceRefresh = pushService.onForceRefresh(onForceRefresh);

    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('pagehide', onPageHide);
      window.removeEventListener('pageshow', onPageShow);
      unsubscribeForceRefresh();

      stopAllPolling();

      useTrafficStore.getState().stopPolling();
      useMetricsStore.getState().disablePush();
      globalState.initialized = false;
      globalState.visibilityPaused = false;
      globalState.forceRefresh = false;
    };
  }, []);
}

export function resetGlobalDataSync() {
  if (globalState.versionCheckIntervalId) {
    clearInterval(globalState.versionCheckIntervalId);
    globalState.versionCheckIntervalId = null;
  }
  globalState.initialized = false;
}

export function isGlobalDataInitialized() {
  return globalState.initialized;
}
