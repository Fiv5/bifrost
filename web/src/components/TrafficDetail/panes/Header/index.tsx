import { useMemo, useRef, useCallback, useState } from "react";
import { Table, Typography, theme, ConfigProvider, Button, Modal, message, Tag, Space, Radio } from "antd";
import { LockOutlined, AppstoreOutlined } from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";
import type { SessionTargetSearchState } from "../../../../types";
import { useMarkSearch } from "../../hooks/useMarkSearch";
import { getTlsConfig, updateTlsConfig, disconnectByDomain } from "../../../../api/config";

const { Text } = Typography;

interface HeaderViewProps {
  headers: [string, string][] | null;
  originalHeaders?: [string, string][] | null;
  actualHeaders?: [string, string][] | null;
  testIdPrefix?: string;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  isTunnel?: boolean;
  host?: string;
  clientApp?: string;
}

interface HeaderItem {
  key: string;
  name: string;
  value: string;
}

const areHeadersEqual = (
  left: [string, string][] | null | undefined,
  right: [string, string][] | null | undefined,
): boolean => {
  if (left === right) {
    return true;
  }
  if (!left || !right) {
    return !left && !right;
  }
  if (left.length !== right.length) {
    return false;
  }
  return left.every(([leftName, leftValue], index) => {
    const [rightName, rightValue] = right[index] ?? [];
    return leftName === rightName && leftValue === rightValue;
  });
};

export const HeaderView = ({
  headers,
  originalHeaders,
  actualHeaders,
  testIdPrefix = "header-view",
  searchValue,
  onSearch,
  isTunnel,
  host,
  clientApp,
}: HeaderViewProps) => {
  const { token } = theme.useToken();
  const tableRef = useRef<HTMLDivElement>(null);
  const [viewMode, setViewMode] = useState<'current' | 'original' | 'actual'>('current');

  const showOriginalTab = !!originalHeaders && !areHeadersEqual(headers, originalHeaders);
  const showActualTab = !!actualHeaders && !areHeadersEqual(headers, actualHeaders);
  const hasModifications = showOriginalTab || showActualTab;
  const resolvedViewMode = useMemo(() => {
    if (viewMode === 'original' && !showOriginalTab) {
      return 'current';
    }
    if (viewMode === 'actual' && !showActualTab) {
      return 'current';
    }
    return viewMode;
  }, [showActualTab, showOriginalTab, viewMode]);

  const handleAddToInterceptList = useCallback(() => {
    if (!host) {
      message.error("No host found for this request");
      return;
    }

    Modal.confirm({
      title: "Add to Intercept List",
      content: `Add "${host}" to TLS intercept list? This will enable HTTPS inspection for this domain and disconnect existing tunnel connections.`,
      okText: "Add",
      cancelText: "Cancel",
      onOk: async () => {
        try {
          const currentConfig = await getTlsConfig();
          if (currentConfig.intercept_include.includes(host)) {
            message.info(`"${host}" is already in the intercept list`);
            return;
          }

          const newIncludeList = [...currentConfig.intercept_include, host];
          await updateTlsConfig({ intercept_include: newIncludeList });

          await disconnectByDomain(host);

          message.success(
            `Added "${host}" to intercept list and disconnected existing connections`
          );
        } catch (error) {
          message.error("Failed to add domain to intercept list");
          console.error(error);
        }
      },
    });
  }, [host]);

  const handleAddAppToInterceptList = useCallback(() => {
    if (!clientApp) {
      message.error("No app found for this request");
      return;
    }

    Modal.confirm({
      title: "Add App to Intercept List",
      content: `Add "${clientApp}" to app intercept list? This will enable HTTPS inspection for this app.`,
      okText: "Add",
      cancelText: "Cancel",
      onOk: async () => {
        try {
          const currentConfig = await getTlsConfig();
          if (currentConfig.app_intercept_include.includes(clientApp)) {
            message.info(`"${clientApp}" is already in the app intercept list`);
            return;
          }

          const newIncludeList = [...currentConfig.app_intercept_include, clientApp];
          await updateTlsConfig({ app_intercept_include: newIncludeList });

          message.success(`Added "${clientApp}" to app intercept list`);
        } catch (error) {
          message.error("Failed to add app to intercept list");
          console.error(error);
        }
      },
    });
  }, [clientApp]);

  const displayHeaders = useMemo(() => {
    if (resolvedViewMode === 'original' && originalHeaders) {
      return originalHeaders;
    }
    if (resolvedViewMode === 'actual' && actualHeaders) {
      return actualHeaders;
    }
    return headers;
  }, [resolvedViewMode, headers, originalHeaders, actualHeaders]);

  const dataSource = useMemo<HeaderItem[]>(() => {
    if (!displayHeaders) return [];
    return displayHeaders.map(([name, value], index) => ({
      key: String(index),
      name,
      value,
    }));
  }, [displayHeaders]);

  const filteredData = useMemo(() => {
    if (!searchValue.value) return dataSource;
    const searchLower = searchValue.value.toLowerCase();
    return dataSource.filter(
      (item) =>
        item.name.toLowerCase().includes(searchLower) ||
        item.value.toLowerCase().includes(searchLower),
    );
  }, [dataSource, searchValue.value]);

  useMarkSearch(searchValue, () => tableRef.current, onSearch);

  const columns: ColumnsType<HeaderItem> = [
    {
      title: "Name",
      dataIndex: "name",
      key: "name",
      width: 180,
      render: (text: string) => (
        <Text strong style={{ fontFamily: "monospace", fontSize: 12 }}>
          {text}
        </Text>
      ),
    },
    {
      title: "Value",
      dataIndex: "value",
      key: "value",
      render: (text: string) => (
        <Text
          style={{ fontFamily: "monospace", fontSize: 12 }}
          copyable={{ text }}
        >
          {text}
        </Text>
      ),
    },
  ];

  if (!headers || headers.length === 0) {
    const showInterceptButton = isTunnel && host;
    const showAppInterceptButton = isTunnel && clientApp;
    const hasAnyButton = showInterceptButton || showAppInterceptButton;

    if (hasAnyButton) {
      return (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: 12,
            minHeight: 200,
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
        >
          {showInterceptButton && (
            <Button
              type="primary"
              icon={<LockOutlined />}
              onClick={handleAddToInterceptList}
              size="large"
            >
              Intercept this domain
            </Button>
          )}
          {showAppInterceptButton && (
            <Button
              type="primary"
              icon={<AppstoreOutlined />}
              onClick={handleAddAppToInterceptList}
              size="large"
            >
              Intercept this app
            </Button>
          )}
        </div>
      );
    }
    return (
      <Text type="secondary" style={{ padding: 8, display: "block" }}>
        No headers
      </Text>
    );
  }

  return (
    <div ref={tableRef}>
      {hasModifications && (
        <div style={{ marginBottom: 8 }}>
          <Space>
            <Radio.Group
              value={resolvedViewMode}
              onChange={(e) => setViewMode(e.target.value)}
              size="small"
              data-testid={`${testIdPrefix}-mode-tabs`}
            >
              <Radio.Button value="current" data-testid={`${testIdPrefix}-tab-current`}>
                Current
              </Radio.Button>
              {showOriginalTab && (
                <Radio.Button value="original" data-testid={`${testIdPrefix}-tab-original`}>
                  <Tag color="blue" style={{ margin: 0, fontSize: 11 }}>Original</Tag>
                </Radio.Button>
              )}
              {showActualTab && (
                <Radio.Button value="actual" data-testid={`${testIdPrefix}-tab-actual`}>
                  <Tag color="orange" style={{ margin: 0, fontSize: 11 }}>Actual</Tag>
                </Radio.Button>
              )}
            </Radio.Group>
          </Space>
        </div>
      )}
      <ConfigProvider
        theme={{
          components: {
            Table: {
              cellPaddingBlockSM: 2,
              cellPaddingInlineSM: 4,
            },
          },
        }}
      >
        <Table
          dataSource={filteredData}
          columns={columns}
          pagination={false}
          size="small"
          style={{
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
        />
      </ConfigProvider>
    </div>
  );
};
