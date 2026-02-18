import { useEffect, useState, useCallback, type CSSProperties } from "react";
import { message, theme } from "antd";
import { useTrafficStore } from "../../stores/useTrafficStore";
import VirtualTrafficTable from "../../components/TrafficTable/VirtualTrafficTable";
import TrafficDetail from "../../components/TrafficDetail";
import Toolbar from "../../components/Toolbar";
import FilterBar from "../../components/FilterBar";
import SplitPane from "../../components/SplitPane";
import type { TrafficSummary, FilterCondition } from "../../types";
import {
  getSystemProxyStatus,
  setSystemProxy,
  type SystemProxyStatus,
} from "../../api/proxy";

export default function Traffic() {
  const { token } = theme.useToken();
  const {
    records,
    currentRecord,
    requestBody,
    responseBody,
    loading,
    detailLoading,
    paused,
    hasMore,
    toolbarFilters,
    filterConditions,
    autoScroll,
    newRecordsCount,
    fetchInitialData,
    startPolling,
    stopPolling,
    fetchTrafficDetail,
    clearTraffic,
    setToolbarFilters,
    setFilterConditions,
    setPaused,
    setAutoScroll,
    clearNewRecordsCount,
  } = useTrafficStore();

  const [selectedId, setSelectedId] = useState<string>();
  const [showFilterBar, setShowFilterBar] = useState(false);
  const [systemProxy, setSystemProxyState] = useState<SystemProxyStatus | null>(
    null
  );
  const [systemProxyLoading, setSystemProxyLoading] = useState(false);

  const fetchSystemProxy = useCallback(async () => {
    try {
      const status = await getSystemProxyStatus();
      setSystemProxyState(status);
    } catch {
      console.error("Failed to fetch system proxy status");
    }
  }, []);

  const handleSystemProxyToggle = useCallback(
    async (enabled: boolean) => {
      setSystemProxyLoading(true);
      try {
        const result = await setSystemProxy({ enabled });
        setSystemProxyState(result);
        message.success(
          enabled ? "System proxy enabled" : "System proxy disabled"
        );
      } catch {
        message.error("Failed to toggle system proxy");
      } finally {
        setSystemProxyLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    fetchInitialData().then(() => {
      startPolling();
    });
    fetchSystemProxy();
    return () => {
      stopPolling();
    };
  }, [fetchInitialData, startPolling, stopPolling, fetchSystemProxy]);

  const handleSelect = useCallback(async (record: TrafficSummary) => {
    setSelectedId(record.id);
    await fetchTrafficDetail(record.id);
  }, [fetchTrafficDetail]);

  const handleClear = useCallback(async () => {
    const success = await clearTraffic();
    if (success) {
      message.success("Traffic cleared");
      setSelectedId(undefined);
    }
  }, [clearTraffic]);

  const handlePauseToggle = useCallback(() => {
    setPaused(!paused);
  }, [paused, setPaused]);

  const handleFilterConditionsChange = useCallback((conditions: FilterCondition[]) => {
    setFilterConditions(conditions);
    setShowFilterBar(conditions.length > 0);
  }, [setFilterConditions]);

  const handleAddFilter = useCallback(() => {
    if (filterConditions.length === 0) {
      const newCondition: FilterCondition = {
        id: `filter_${Date.now()}`,
        field: 'url',
        operator: 'contains',
        value: '',
      };
      setFilterConditions([newCondition]);
      setShowFilterBar(true);
    }
  }, [filterConditions, setFilterConditions]);

  const handleScrollPositionChange = useCallback((isAtBottom: boolean) => {
    setAutoScroll(isAtBottom);
  }, [setAutoScroll]);

  const handleScrollToBottom = useCallback(() => {
    clearNewRecordsCount();
  }, [clearNewRecordsCount]);

  const styles: Record<string, CSSProperties> = {
    container: {
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
      backgroundColor: token.colorBgContainer,
    },
    filterBarWrapper: {
      padding: '8px 16px',
      backgroundColor: token.colorBgContainer,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
    },
    mainContent: {
      flex: 1,
      overflow: 'hidden',
      backgroundColor: token.colorBgContainer,
    },
    tableWrapper: {
      height: '100%',
      backgroundColor: token.colorBgContainer,
    },
    detailWrapper: {
      height: '100%',
      padding: 16,
      backgroundColor: token.colorBgContainer,
      overflow: 'auto',
    },
  };

  return (
    <div style={styles.container}>
      <Toolbar
        paused={paused}
        filters={toolbarFilters}
        onPauseToggle={handlePauseToggle}
        onClear={handleClear}
        onFilterChange={setToolbarFilters}
        onAddFilter={handleAddFilter}
        systemProxyEnabled={systemProxy?.enabled}
        systemProxySupported={systemProxy?.supported}
        systemProxyLoading={systemProxyLoading}
        onSystemProxyToggle={handleSystemProxyToggle}
      />

      {showFilterBar && (
        <div style={styles.filterBarWrapper}>
          <FilterBar
            filters={filterConditions}
            onFiltersChange={handleFilterConditionsChange}
          />
        </div>
      )}

      <div style={styles.mainContent}>
        <SplitPane
          defaultLeftWidth="55%"
          minLeftWidth={400}
          minRightWidth={350}
          left={
            <div style={styles.tableWrapper}>
              <VirtualTrafficTable
                data={records}
                loading={loading}
                onSelect={handleSelect}
                selectedId={selectedId}
                hasMore={hasMore}
                autoScroll={autoScroll}
                onScrollPositionChange={handleScrollPositionChange}
                newRecordsCount={newRecordsCount}
                onScrollToBottom={handleScrollToBottom}
              />
            </div>
          }
          right={
            <div style={styles.detailWrapper}>
              <TrafficDetail
                record={currentRecord}
                requestBody={requestBody}
                responseBody={responseBody}
                loading={detailLoading}
              />
            </div>
          }
        />
      </div>
    </div>
  );
}
