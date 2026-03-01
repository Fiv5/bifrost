import { useMemo, useCallback, type CSSProperties } from "react";
import { Tabs, Tag, Typography, Empty, Button, Tooltip, message, Table, theme, ConfigProvider } from "antd";
import type { ColumnsType } from "antd/es/table";
import { CloseCircleOutlined, CopyOutlined, SaveOutlined } from "@ant-design/icons";
import { useReplayStore, type ResponsePanelTab } from "../../../stores/useReplayStore";
import CodeViewer from "./CodeViewer";

const { Text } = Typography;

function getStatusText(status: number): string {
  const statusTexts: Record<number, string> = {
    200: "OK",
    201: "Created",
    204: "No Content",
    301: "Moved Permanently",
    302: "Found",
    304: "Not Modified",
    400: "Bad Request",
    401: "Unauthorized",
    403: "Forbidden",
    404: "Not Found",
    405: "Method Not Allowed",
    500: "Internal Server Error",
    502: "Bad Gateway",
    503: "Service Unavailable",
  };
  return statusTexts[status] || "";
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

interface HeaderItem {
  key: string;
  name: string;
  value: string;
}

interface RuleItem {
  key: string;
  protocol: string;
  rule_name?: string;
  pattern: string;
  value: string;
}

export default function ResponsePanel() {
  const { token } = theme.useToken();
  const { currentResponse, currentTrafficRecord, uiState, updateUIState } = useReplayStore();
  const activeTab = uiState.responsePanelActiveTab;

  const setActiveTab = useCallback((tab: string) => {
    updateUIState({ responsePanelActiveTab: tab as ResponsePanelTab });
  }, [updateUIState]);

  const status = currentResponse?.status || currentTrafficRecord?.status || 0;
  const duration = currentResponse?.duration_ms || currentTrafficRecord?.duration_ms || 0;
  const error = currentResponse?.error;

  const statusColor = useMemo(() => {
    if (!status) return "default";
    if (status >= 500) return "red";
    if (status >= 400) return "orange";
    if (status >= 300) return "blue";
    if (status >= 200) return "green";
    return "default";
  }, [status]);

  const responseHeaders = useMemo(() => {
    return currentResponse?.headers || currentTrafficRecord?.response_headers || [];
  }, [currentResponse, currentTrafficRecord]);

  const responseBody = useMemo(() => {
    return currentResponse?.body || currentTrafficRecord?.response_body;
  }, [currentResponse, currentTrafficRecord]);

  const appliedRules = useMemo(() => {
    return currentResponse?.applied_rules || currentTrafficRecord?.matched_rules || [];
  }, [currentResponse?.applied_rules, currentTrafficRecord?.matched_rules]);
  const isEmpty = !currentResponse && !currentTrafficRecord;

  const handleCopyHeaders = useCallback(async () => {
    if (responseHeaders.length === 0) return;
    const text = responseHeaders.map(([k, v]) => `${k}: ${v}`).join("\n");
    try {
      await navigator.clipboard.writeText(text);
      message.success("Headers copied");
    } catch {
      message.error("Failed to copy");
    }
  }, [responseHeaders]);

  const headerDataSource: HeaderItem[] = useMemo(() => {
    return responseHeaders.map(([name, value], index) => ({
      key: String(index),
      name,
      value,
    }));
  }, [responseHeaders]);

  const headerColumns: ColumnsType<HeaderItem> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      width: 200,
      render: (text: string) => (
        <span style={{ fontFamily: 'monospace', fontSize: 12, fontWeight: 500 }}>{text}</span>
      ),
    },
    {
      title: 'Value',
      dataIndex: 'value',
      key: 'value',
      render: (text: string) => (
        <span style={{ fontFamily: 'monospace', fontSize: 12, wordBreak: 'break-all' }}>{text}</span>
      ),
    },
  ];

  const ruleDataSource: RuleItem[] = useMemo(() => {
    return appliedRules.map((rule, index) => ({
      key: String(index),
      protocol: rule.protocol,
      rule_name: rule.rule_name,
      pattern: rule.pattern,
      value: rule.value,
    }));
  }, [appliedRules]);

  const ruleColumns: ColumnsType<RuleItem> = [
    {
      title: 'Protocol',
      dataIndex: 'protocol',
      key: 'protocol',
      width: 100,
      render: (text: string) => <Tag color="blue">{text}</Tag>,
    },
    {
      title: 'Rule',
      dataIndex: 'rule_name',
      key: 'rule_name',
      width: 150,
      render: (text: string) => <span style={{ fontWeight: 500, fontSize: 12 }}>{text || '-'}</span>,
    },
    {
      title: 'Pattern',
      dataIndex: 'pattern',
      key: 'pattern',
      render: (text: string) => (
        <code style={{
          fontFamily: 'monospace',
          fontSize: 12,
          color: token.colorPrimary,
          backgroundColor: token.colorBgLayout,
          padding: '2px 6px',
          borderRadius: 4,
        }}>
          {text}
        </code>
      ),
    },
    {
      title: 'Value',
      dataIndex: 'value',
      key: 'value',
      render: (text: string) => (
        <span style={{ fontSize: 12, wordBreak: 'break-all' }}>{text}</span>
      ),
    },
  ];

  const styles: Record<string, CSSProperties> = {
    container: {
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
    },
    header: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '8px 12px',
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      backgroundColor: token.colorBgLayout,
    },
    statusBar: {
      display: 'flex',
      alignItems: 'center',
      gap: 16,
      minHeight: 22,
    },
    statusItem: {
      display: 'flex',
      alignItems: 'center',
      gap: 4,
    },
    statusLabel: {
      fontSize: 12,
      color: token.colorTextSecondary,
    },
    statusValue: {
      fontSize: 12,
      fontWeight: 500,
      color: token.colorText,
    },
    content: {
      flex: 1,
      overflow: 'hidden',
      display: 'flex',
      flexDirection: 'column',
    },
    emptyState: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      minHeight: 150,
    },
    errorState: {
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      gap: 12,
      height: '100%',
      minHeight: 150,
      padding: 40,
    },
    errorIcon: {
      fontSize: 32,
      color: token.colorError,
    },
    emptyBody: {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      minHeight: 100,
    },
    bodyContainer: {
      height: '100%',
      padding: 8,
    },
    tabContent: {
      padding: 8,
      height: '100%',
      overflow: 'auto',
    },
    cookieItem: {
      padding: 12,
      backgroundColor: token.colorBgLayout,
      borderRadius: 4,
      marginBottom: 8,
    },
    cookieValue: {
      fontFamily: 'Monaco, Menlo, Ubuntu Mono, Consolas, monospace',
      fontSize: 12,
      wordBreak: 'break-all',
      color: token.colorText,
    },
  };

  const tabItems = [
    {
      key: "body",
      label: "Body",
      children: isEmpty ? (
        <div style={styles.emptyState}>
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description="Send a request to see the response"
          />
        </div>
      ) : error ? (
        <div style={styles.errorState}>
          <CloseCircleOutlined style={styles.errorIcon} />
          <Text type="danger" style={{ fontSize: 13, textAlign: 'center', maxWidth: 400 }}>
            {error}
          </Text>
        </div>
      ) : responseBody ? (
        <div style={styles.bodyContainer}>
          <CodeViewer content={responseBody} showToolbar={true} />
        </div>
      ) : (
        <div style={styles.emptyBody}>
          <Text type="secondary" style={{ fontSize: 12 }}>No response body</Text>
        </div>
      ),
    },
    {
      key: "cookies",
      label: "Cookies",
      children: (
        <div style={styles.tabContent}>
          {(() => {
            const cookieHeaders = responseHeaders.filter(
              ([k]) => k.toLowerCase() === "set-cookie"
            );
            if (cookieHeaders.length === 0) {
              return (
                <div style={styles.emptyBody}>
                  <Text type="secondary" style={{ fontSize: 12 }}>No cookies</Text>
                </div>
              );
            }
            return (
              <div>
                {cookieHeaders.map(([, value], index) => (
                  <div key={index} style={styles.cookieItem}>
                    <code style={styles.cookieValue}>{value}</code>
                  </div>
                ))}
              </div>
            );
          })()}
        </div>
      ),
    },
    {
      key: "headers",
      label: `Headers (${responseHeaders.length})`,
      children:
        responseHeaders.length > 0 ? (
          <div style={styles.tabContent}>
            <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: 8 }}>
              <Tooltip title="Copy all headers">
                <Button
                  type="text"
                  size="small"
                  icon={<CopyOutlined />}
                  onClick={handleCopyHeaders}
                >
                  Copy
                </Button>
              </Tooltip>
            </div>
            <ConfigProvider
              theme={{
                components: {
                  Table: {
                    cellPaddingBlockSM: 8,
                    cellPaddingInlineSM: 12,
                  },
                },
              }}
            >
              <Table
                dataSource={headerDataSource}
                columns={headerColumns}
                rowKey="key"
                pagination={false}
                size="small"
                style={{ backgroundColor: token.colorBgLayout, borderRadius: 4 }}
              />
            </ConfigProvider>
          </div>
        ) : (
          <div style={styles.emptyBody}>
            <Text type="secondary" style={{ fontSize: 12 }}>No headers</Text>
          </div>
        ),
    },
    {
      key: "rules",
      label: `Test Results (${appliedRules.length})`,
      children:
        appliedRules.length > 0 ? (
          <div style={styles.tabContent}>
            <ConfigProvider
              theme={{
                components: {
                  Table: {
                    cellPaddingBlockSM: 8,
                    cellPaddingInlineSM: 12,
                  },
                },
              }}
            >
              <Table
                dataSource={ruleDataSource}
                columns={ruleColumns}
                rowKey="key"
                pagination={false}
                size="small"
                style={{ backgroundColor: token.colorBgLayout, borderRadius: 4 }}
              />
            </ConfigProvider>
          </div>
        ) : (
          <div style={styles.emptyBody}>
            <Text type="secondary" style={{ fontSize: 12 }}>No rules applied</Text>
          </div>
        ),
    },
  ];

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <div style={styles.statusBar}>
          {!isEmpty && (
            <>
              <div style={styles.statusItem}>
                <span style={styles.statusLabel}>Status:</span>
                <Tag color={statusColor} style={{ margin: 0, fontSize: 12 }}>
                  {status} {getStatusText(status)}
                </Tag>
              </div>
              <div style={styles.statusItem}>
                <span style={styles.statusLabel}>Time:</span>
                <span style={styles.statusValue}>
                  {duration < 1000
                    ? `${duration} ms`
                    : `${(duration / 1000).toFixed(2)} s`}
                </span>
              </div>
              {responseBody && (
                <div style={styles.statusItem}>
                  <span style={styles.statusLabel}>Size:</span>
                  <span style={styles.statusValue}>
                    {formatSize(responseBody.length)}
                  </span>
                </div>
              )}
            </>
          )}
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {!isEmpty && responseBody && (
            <Tooltip title="Save as example">
              <Button type="text" size="small" icon={<SaveOutlined />}>
                Save as example
              </Button>
            </Tooltip>
          )}
        </div>
      </div>
      <div style={styles.content}>
        <Tabs
          items={tabItems}
          activeKey={activeTab}
          onChange={setActiveTab}
          size="small"
          style={{ height: '100%' }}
          tabBarStyle={{ margin: 0, padding: '0 12px', backgroundColor: token.colorBgLayout }}
        />
      </div>
    </div>
  );
}
