import {
  useEffect,
  useState,
  useCallback,
  useRef,
  useMemo,
  type CSSProperties,
} from "react";
import { useSearchParams } from "react-router-dom";
import { message, theme } from "antd";
import { useTrafficStore, filterRecords, type PanelFilters } from "../../stores/useTrafficStore";
import { useProxyStore } from "../../stores/useProxyStore";
import { useFilterPanelStore } from "../../stores/useFilterPanelStore";
import VirtualTrafficTable from "../../components/TrafficTable/VirtualTrafficTable";
import TrafficDetail from "../../components/TrafficDetail";
import Toolbar from "../../components/Toolbar";
import FilterBar from "../../components/FilterBar";
import ThreeSplitPane from "../../components/ThreeSplitPane";
import FilterPanel from "../../components/FilterPanel";
import type {
  TrafficSummary,
  FilterCondition,
  ToolbarFilters,
} from "../../types";

const FILTER_PARAM = "filter";
const TOOLBAR_PARAM = "toolbar";

const serializeFilters = (filters: FilterCondition[]): string => {
  if (filters.length === 0) return "";
  return btoa(JSON.stringify(filters));
};

const deserializeFilters = (str: string): FilterCondition[] => {
  if (!str) return [];
  try {
    return JSON.parse(atob(str));
  } catch {
    return [];
  }
};

const serializeToolbar = (toolbar: ToolbarFilters): string => {
  const hasFilters =
    toolbar.rule.length > 0 ||
    toolbar.protocol.length > 0 ||
    toolbar.type.length > 0 ||
    toolbar.status.length > 0;
  if (!hasFilters) return "";
  return btoa(JSON.stringify(toolbar));
};

const deserializeToolbar = (str: string): ToolbarFilters | null => {
  if (!str) return null;
  try {
    return JSON.parse(atob(str));
  } catch {
    return null;
  }
};

export default function Traffic() {
  const { token } = theme.useToken();
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    records,
    currentRecord,
    requestBody,
    responseBody,
    detailLoading,
    hasMore,
    toolbarFilters,
    filterConditions,
    autoScroll,
    newRecordsCount,
    scrollTop,
    fetchInitialData,
    startPolling,
    stopPolling,
    fetchTrafficDetail,
    clearTraffic,
    setToolbarFilters,
    setFilterConditions,
    setAutoScroll,
    clearNewRecordsCount,
    initFromUrl,
    setScrollTop,
  } = useTrafficStore();

  const [selectedId, setSelectedId] = useState<string>();
  const [showFilterBar] = useState(true);
  const {
    systemProxy,
    loading: systemProxyLoading,
    fetchSystemProxy,
    toggleSystemProxy,
  } = useProxyStore();

  const {
    panelCollapsed: filterPanelCollapsed,
    setPanelCollapsed: setFilterPanelCollapsed,
    panelWidth: filterPanelWidth,
    setPanelWidth: setFilterPanelWidth,
    detailPanelCollapsed,
    setDetailPanelCollapsed,
    selectedClientIps,
    selectedClientApps,
    selectedDomains,
    loadFromServer: loadFilterPanelConfig,
    initialized: filterPanelInitialized,
  } = useFilterPanelStore();

  const initializedRef = useRef(false);
  const isUpdatingUrlRef = useRef(false);

  const handleSystemProxyToggle = useCallback(
    async (enabled: boolean) => {
      const success = await toggleSystemProxy(enabled);
      if (success) {
        message.success(
          enabled ? "System proxy enabled" : "System proxy disabled",
        );
      } else {
        message.error("Failed to toggle system proxy");
      }
    },
    [toggleSystemProxy],
  );

  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;

    const filterParam = searchParams.get(FILTER_PARAM);
    const toolbarParam = searchParams.get(TOOLBAR_PARAM);

    const filtersFromUrl = deserializeFilters(filterParam || "");
    const toolbarFromUrl = deserializeToolbar(toolbarParam || "");

    if (filtersFromUrl.length > 0 || toolbarFromUrl) {
      initFromUrl(filtersFromUrl, toolbarFromUrl);
    }

    fetchInitialData().then(() => {
      startPolling();
    });
    fetchSystemProxy();
    loadFilterPanelConfig();

    return () => {
      stopPolling();
    };
  }, [
    searchParams,
    fetchInitialData,
    startPolling,
    stopPolling,
    fetchSystemProxy,
    initFromUrl,
    loadFilterPanelConfig,
  ]);

  useEffect(() => {
    if (!initializedRef.current) return;
    if (isUpdatingUrlRef.current) {
      isUpdatingUrlRef.current = false;
      return;
    }

    isUpdatingUrlRef.current = true;
    setSearchParams(
      (prev) => {
        const filterStr = serializeFilters(filterConditions);
        const toolbarStr = serializeToolbar(toolbarFilters);

        if (filterStr) {
          prev.set(FILTER_PARAM, filterStr);
        } else {
          prev.delete(FILTER_PARAM);
        }

        if (toolbarStr) {
          prev.set(TOOLBAR_PARAM, toolbarStr);
        } else {
          prev.delete(TOOLBAR_PARAM);
        }

        return prev;
      },
      { replace: true },
    );
  }, [filterConditions, toolbarFilters, setSearchParams]);

  const handleSelect = useCallback(
    async (record: TrafficSummary) => {
      setSelectedId(record.id);
      await fetchTrafficDetail(record.id);
    },
    [fetchTrafficDetail],
  );

  const handleClear = useCallback(async () => {
    const success = await clearTraffic();
    if (success) {
      message.success("Traffic cleared");
      setSelectedId(undefined);
    }
  }, [clearTraffic]);

  const handleFilterConditionsChange = useCallback(
    (conditions: FilterCondition[]) => {
      setFilterConditions(conditions);
    },
    [setFilterConditions],
  );

  const handleDetailPanelToggle = useCallback(() => {
    setDetailPanelCollapsed(!detailPanelCollapsed);
  }, [detailPanelCollapsed, setDetailPanelCollapsed]);

  const handleFilterPanelToggle = useCallback(() => {
    setFilterPanelCollapsed(!filterPanelCollapsed);
  }, [filterPanelCollapsed, setFilterPanelCollapsed]);

  const handleDoubleClick = useCallback(
    async (record: TrafficSummary) => {
      setSelectedId(record.id);
      await fetchTrafficDetail(record.id);
      if (detailPanelCollapsed) {
        setDetailPanelCollapsed(false);
      }
    },
    [fetchTrafficDetail, detailPanelCollapsed],
  );

  const handleScrollPositionChange = useCallback(
    (isAtBottom: boolean) => {
      setAutoScroll(isAtBottom);
    },
    [setAutoScroll],
  );

  const handleScrollToBottom = useCallback(() => {
    clearNewRecordsCount();
  }, [clearNewRecordsCount]);

  const handleScrollTopChange = useCallback(
    (newScrollTop: number) => {
      setScrollTop(newScrollTop);
    },
    [setScrollTop],
  );

  const panelFilters = useMemo<PanelFilters>(() => ({
    clientIps: selectedClientIps,
    clientApps: selectedClientApps,
    domains: selectedDomains,
  }), [selectedClientIps, selectedClientApps, selectedDomains]);

  const filteredRecords = useMemo(() => {
    return filterRecords(records, toolbarFilters, filterConditions, panelFilters);
  }, [records, toolbarFilters, filterConditions, panelFilters]);

  const clientInfo = useMemo(() => {
    const appSet = new Set<string>();
    const ipSet = new Set<string>();
    const domainSet = new Set<string>();
    for (const record of records) {
      if (record.client_app) appSet.add(record.client_app);
      if (record.client_ip) ipSet.add(record.client_ip);
      if (record.host) domainSet.add(record.host);
    }
    return {
      apps: Array.from(appSet).sort(),
      ips: Array.from(ipSet).sort(),
      domains: Array.from(domainSet).sort(),
    };
  }, [records]);

  const styles = useMemo<Record<string, CSSProperties>>(
    () => ({
      container: {
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
        backgroundColor: token.colorBgContainer,
      },
      filterBarWrapper: {
        padding: "8px 16px",
        backgroundColor: token.colorBgContainer,
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
      },
      mainContent: {
        flex: 1,
        overflow: "hidden",
        backgroundColor: token.colorBgContainer,
      },
      centerWrapper: {
        display: "flex",
        flexDirection: "column",
        height: "100%",
        overflow: "hidden",
      },
      tableWrapper: {
        flex: 1,
        minHeight: 0,
        backgroundColor: token.colorBgContainer,
      },
      detailWrapper: {
        height: "100%",
        padding: 4,
        backgroundColor: token.colorBgContainer,
        overflow: "auto",
      },
    }),
    [token],
  );

  const renderCenter = () => (
    <div style={styles.centerWrapper}>
      {showFilterBar && (
        <div style={styles.filterBarWrapper}>
          <FilterBar
            filters={filterConditions}
            onFiltersChange={handleFilterConditionsChange}
            availableClientApps={clientInfo.apps}
            availableClientIps={clientInfo.ips}
          />
        </div>
      )}
      <div style={styles.tableWrapper}>
        <VirtualTrafficTable
          data={filteredRecords}
          onSelect={handleSelect}
          onDoubleClick={handleDoubleClick}
          selectedId={selectedId}
          hasMore={hasMore}
          autoScroll={autoScroll}
          onScrollPositionChange={handleScrollPositionChange}
          newRecordsCount={newRecordsCount}
          onScrollToBottom={handleScrollToBottom}
          initialScrollTop={scrollTop}
          onScrollTopChange={handleScrollTopChange}
        />
      </div>
    </div>
  );

  const renderDetail = () => (
    <div style={styles.detailWrapper}>
      <TrafficDetail
        record={currentRecord}
        requestBody={requestBody}
        responseBody={responseBody}
        loading={detailLoading}
      />
    </div>
  );

  const renderFilterPanel = () => (
    <FilterPanel
      availableClientIps={clientInfo.ips}
      availableClientApps={clientInfo.apps}
      availableDomains={clientInfo.domains}
    />
  );

  return (
    <div style={styles.container}>
      <Toolbar
        filters={toolbarFilters}
        onClear={handleClear}
        onFilterChange={setToolbarFilters}
        systemProxyEnabled={systemProxy?.enabled}
        systemProxySupported={systemProxy?.supported}
        systemProxyLoading={systemProxyLoading}
        onSystemProxyToggle={handleSystemProxyToggle}
        filterPanelCollapsed={filterPanelCollapsed}
        onFilterPanelToggle={handleFilterPanelToggle}
        detailPanelCollapsed={detailPanelCollapsed}
        onDetailPanelToggle={handleDetailPanelToggle}
      />

      <div style={styles.mainContent}>
        {filterPanelInitialized ? (
          <ThreeSplitPane
            left={renderFilterPanel()}
            center={renderCenter()}
            right={renderDetail()}
            leftWidth={filterPanelWidth}
            minLeftWidth={180}
            maxLeftWidth={350}
            minCenterWidth={400}
            minRightWidth={350}
            leftCollapsed={filterPanelCollapsed}
            rightCollapsed={detailPanelCollapsed}
            onLeftWidthChange={setFilterPanelWidth}
          />
        ) : (
          <div style={{ flex: 1 }} />
        )}
      </div>
    </div>
  );
}
