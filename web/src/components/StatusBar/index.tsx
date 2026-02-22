import { useEffect, useMemo, type CSSProperties } from "react";
import { theme } from "antd";
import { ArrowUpOutlined, ArrowDownOutlined } from "@ant-design/icons";
import { useMetricsStore } from "../../stores/useMetricsStore";
import { useProxyStore } from "../../stores/useProxyStore";

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

export default function StatusBar() {
  const { token } = theme.useToken();
  const { overview, current, enablePush, disablePush } = useMetricsStore();
  const { systemProxy, fetchSystemProxy } = useProxyStore();

  useEffect(() => {
    fetchSystemProxy();
    enablePush({ needOverview: true, needMetrics: true });
    return () => {
      disablePush();
    };
  }, []);

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
  };

  return (
    <div style={styles.container}>
      <div style={styles.item}>
        <div style={styles.statusDot} />
        <span style={styles.label}>Proxy:</span>
        <span style={styles.value}>{proxyStatus.text}</span>
      </div>

      <div style={styles.separator} />

      <div style={styles.item}>
        <ArrowUpOutlined style={styles.rateUp} />
        <span style={styles.value}>{uploadRate}</span>
      </div>

      <div style={styles.item}>
        <ArrowDownOutlined style={styles.rateDown} />
        <span style={styles.value}>{downloadRate}</span>
      </div>

      <div style={styles.separator} />

      <div style={styles.item}>
        <span style={styles.label}>Total:</span>
        <span style={styles.value}>{totalTraffic}</span>
      </div>

      <div style={styles.separator} />

      <div style={styles.item}>
        <span style={styles.label}>Conn:</span>
        <span style={styles.value}>{metrics?.active_connections ?? 0}</span>
      </div>

      <div style={styles.item}>
        <span style={styles.label}>Req:</span>
        <span style={styles.value}>{metrics?.total_requests ?? 0}</span>
      </div>

      <div style={styles.separator} />

      <div style={styles.item}>
        <span style={styles.label}>Mem:</span>
        <span style={styles.value}>{memoryUsage}</span>
      </div>

      <div style={styles.item}>
        <span style={styles.label}>CPU:</span>
        <span style={styles.value}>{cpuUsage}</span>
      </div>

      <div style={styles.separator} />

      <div style={styles.item}>
        <span style={styles.label}>Uptime:</span>
        <span style={styles.value}>{uptime}</span>
      </div>

      {overview?.system?.version && (
        <>
          <div style={{ flex: 1 }} />
          <div style={styles.item}>
            <span style={styles.value}>v{overview.system.version}</span>
          </div>
        </>
      )}
    </div>
  );
}
