import { useEffect, useState, useCallback, type CSSProperties } from "react";
import { message, theme } from "antd";
import { useTrafficStore } from "../../stores/useTrafficStore";
import TrafficTable from "../../components/TrafficTable";
import TrafficDetail from "../../components/TrafficDetail";
import Toolbar from "../../components/Toolbar";
import FilterBar from "../../components/FilterBar";
import SplitPane from "../../components/SplitPane";
import type { TrafficSummary, FilterCondition } from "../../types";

export default function Traffic() {
  const { token } = theme.useToken();
  const {
    records,
    currentRecord,
    requestBody,
    responseBody,
    loading,
    paused,
    toolbarFilters,
    filterConditions,
    fetchTraffic,
    fetchTrafficDetail,
    clearTraffic,
    setToolbarFilters,
    setFilterConditions,
    setPaused,
  } = useTrafficStore();

  const [selectedId, setSelectedId] = useState<string>();
  const [showFilterBar, setShowFilterBar] = useState(false);

  useEffect(() => {
    fetchTraffic();
    const interval = setInterval(fetchTraffic, 1000);
    return () => clearInterval(interval);
  }, [fetchTraffic]);

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

  const styles: Record<string, CSSProperties> = {
    container: {
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
    },
    filterBarWrapper: {
      padding: '8px 16px',
      backgroundColor: token.colorBgContainer,
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
    },
    mainContent: {
      flex: 1,
      overflow: 'hidden',
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
              <TrafficTable
                data={records}
                loading={loading}
                onSelect={handleSelect}
                selectedId={selectedId}
              />
            </div>
          }
          right={
            <div style={styles.detailWrapper}>
              <TrafficDetail
                record={currentRecord}
                requestBody={requestBody}
                responseBody={responseBody}
              />
            </div>
          }
        />
      </div>
    </div>
  );
}
