import { useEffect, useMemo, memo, useCallback, type CSSProperties } from "react";
import { theme, Tooltip } from "antd";
import { ArrowUpOutlined, ArrowDownOutlined } from "@ant-design/icons";
import { useShallow } from "zustand/react/shallow";
import { useMetricsStore } from "../../stores/useMetricsStore";
import { useProxyStore } from "../../stores/useProxyStore";
import { useVersionStore } from "../../stores/useVersionStore";
import VersionModal from "../VersionModal";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function formatBytesRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    return m > 0 ? `${h}h ${m}m` : `${h}h`;
  }
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  return h > 0 ? `${d}d ${h}h` : `${d}d`;
}

const StatusBar = memo(function StatusBar() {
  const { token } = theme.useToken();
  const { overview, current, enablePush, disablePush } = useMetricsStore(
    useShallow((state) => ({
      overview: state.overview,
      current: state.current,
      enablePush: state.enablePush,
      disablePush: state.disablePush,
    })),
  );
  const systemProxy = useProxyStore((state) => state.systemProxy);
  const fetchSystemProxy = useProxyStore((state) => state.fetchSystemProxy);
  
  const hasUpdate = useVersionStore((state) => state.hasUpdate);
  const latestVersion = useVersionStore((state) => state.latestVersion);
  const setModalVisible = useVersionStore((state) => state.setModalVisible);
  const checkVersion = useVersionStore((state) => state.checkVersion);

  useEffect(() => {
    fetchSystemProxy();
    enablePush({ needOverview: true, needMetrics: true });
    return () => {
      disablePush();
    };
  }, [fetchSystemProxy, enablePush, disablePush]);

  const metrics = current || overview?.metrics;

  const totalTraffic = useMemo(() => {
    if (!metrics) return "0 B";
    return formatBytes(metrics.bytes_sent + metrics.bytes_received);
  }, [metrics]);

  const uploadRate = useMemo(() => {
    if (!metrics) return "0 B/s";
    return formatBytesRate(metrics.bytes_sent_rate);
  }, [metrics]);

  const downloadRate = useMemo(() => {
    if (!metrics) return "0 B/s";
    return formatBytesRate(metrics.bytes_received_rate);
  }, [metrics]);

  const memoryUsage = useMemo(() => {
    if (!metrics) return "-";
    return formatBytes(metrics.memory_used);
  }, [metrics]);

  const cpuUsage = useMemo(() => {
    if (!metrics) return "-";
    return `${metrics.cpu_usage.toFixed(1)}%`;
  }, [metrics]);

  const uptime = useMemo(() => {
    if (!overview?.system) return "-";
    return formatUptime(overview.system.uptime_secs);
  }, [overview]);

  const proxyStatus = useMemo(() => {
    if (!systemProxy) return { text: "Unknown", running: false };
    return {
      text: systemProxy.enabled ? "Running" : "Stopped",
      running: systemProxy.enabled,
    };
  }, [systemProxy]);

  const handleVersionClick = useCallback(() => {
    checkVersion({ forceRefresh: true });
    setModalVisible(true);
  }, [checkVersion, setModalVisible]);

  const styles: Record<string, CSSProperties> = {
    container: {
      height: 20,
      backgroundColor: token.colorBgContainer,
      borderTop: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      alignItems: "center",
      padding: "0 12px",
      fontSize: 10,
      color: token.colorTextTertiary,
      gap: 16,
      flexShrink: 0,
      overflow: "hidden",
    },
    item: {
      display: "flex",
      alignItems: "center",
      gap: 4,
      whiteSpace: "nowrap",
    },
    label: {
      opacity: 0.7,
    },
    value: {
      fontFamily: "monospace",
    },
    valueRate: {
      fontFamily: "monospace",
      minWidth: 70,
      textAlign: "right" as const,
    },
    valueTraffic: {
      fontFamily: "monospace",
      minWidth: 58,
      textAlign: "right" as const,
    },
    valueNumber: {
      fontFamily: "monospace",
      minWidth: 40,
      textAlign: "right" as const,
    },
    valueMem: {
      fontFamily: "monospace",
      minWidth: 52,
      textAlign: "right" as const,
    },
    valueCpu: {
      fontFamily: "monospace",
      minWidth: 38,
      textAlign: "right" as const,
    },
    valueUptime: {
      fontFamily: "monospace",
      minWidth: 48,
      textAlign: "right" as const,
    },
    valueStatus: {
      fontFamily: "monospace",
      minWidth: 52,
    },
    statusDot: {
      width: 6,
      height: 6,
      borderRadius: "50%",
      backgroundColor: proxyStatus.running
        ? token.colorSuccess
        : token.colorTextQuaternary,
    },
    rateUp: {
      color: token.colorTextTertiary,
    },
    rateDown: {
      color: token.colorTextTertiary,
    },
    separator: {
      width: 1,
      height: 10,
      backgroundColor: token.colorBorderSecondary,
    },
    versionButton: {
      display: "flex",
      alignItems: "center",
      gap: 4,
      cursor: "pointer",
      padding: "2px 6px",
      borderRadius: 3,
      transition: "background-color 0.2s",
    },
    versionButtonHover: {
      backgroundColor: token.colorFillSecondary,
    },
    updateDot: {
      width: 6,
      height: 6,
      borderRadius: "50%",
      backgroundColor: token.colorError,
    },
    updateArrow: {
      fontSize: 10,
      color: token.colorSuccess,
    },
  };

  const versionTooltip = hasUpdate
    ? `New version available: v${latestVersion}`
    : "Click to view version info";

  return (
    <>
      <div style={styles.container}>
        <div style={styles.item}>
          <div style={styles.statusDot} />
          <span style={styles.label}>Proxy:</span>
          <span style={styles.valueStatus}>{proxyStatus.text}</span>
        </div>

        <div style={styles.separator} />

        <div style={styles.item}>
          <ArrowUpOutlined style={styles.rateUp} />
          <span style={styles.valueRate}>{uploadRate}</span>
        </div>

        <div style={styles.item}>
          <ArrowDownOutlined style={styles.rateDown} />
          <span style={styles.valueRate}>{downloadRate}</span>
        </div>

        <div style={styles.separator} />

        <div style={styles.item}>
          <span style={styles.label}>Total:</span>
          <span style={styles.valueTraffic}>{totalTraffic}</span>
        </div>

        <div style={styles.separator} />

        <div style={styles.item}>
          <span style={styles.label}>Conn:</span>
          <span style={styles.valueNumber}>
            {metrics?.active_connections ?? 0}
          </span>
        </div>

        <div style={styles.item}>
          <span style={styles.label}>Req:</span>
          <span style={styles.valueNumber}>{metrics?.total_requests ?? 0}</span>
        </div>

        <div style={styles.separator} />

        <div style={styles.item}>
          <span style={styles.label}>Mem:</span>
          <span style={styles.valueMem}>{memoryUsage}</span>
        </div>

        <div style={styles.item}>
          <span style={styles.label}>CPU:</span>
          <span style={styles.valueCpu}>{cpuUsage}</span>
        </div>

        <div style={styles.separator} />

        <div style={styles.item}>
          <span style={styles.label}>Uptime:</span>
          <span style={styles.valueUptime}>{uptime}</span>
        </div>

        {overview?.system?.version && (
          <>
            <div style={{ flex: 1 }} />
            <Tooltip title={versionTooltip}>
              <div
                style={styles.versionButton}
                onClick={handleVersionClick}
                onMouseEnter={(e) => {
                  e.currentTarget.style.backgroundColor = token.colorFillSecondary;
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.backgroundColor = "transparent";
                }}
              >
                {hasUpdate && <div style={styles.updateDot} />}
                <span style={styles.value}>v{overview.system.version}</span>
                {hasUpdate && (
                  <ArrowUpOutlined style={styles.updateArrow} />
                )}
              </div>
            </Tooltip>
          </>
        )}
      </div>
      <VersionModal />
    </>
  );
});

export default StatusBar;
