import { useMemo, useRef, useCallback } from "react";
import { Table, Typography, theme, ConfigProvider, Button, Modal, message } from "antd";
import { LockOutlined } from "@ant-design/icons";
import type { ColumnsType } from "antd/es/table";
import type { SessionTargetSearchState } from "../../../../types";
import { useMarkSearch } from "../../hooks/useMarkSearch";
import { getTlsConfig, updateTlsConfig, disconnectByDomain } from "../../../../api/config";

const { Text } = Typography;

interface HeaderViewProps {
  headers: [string, string][] | null;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  isTunnel?: boolean;
  host?: string;
}

interface HeaderItem {
  key: string;
  name: string;
  value: string;
}

export const HeaderView = ({
  headers,
  searchValue,
  onSearch,
  isTunnel,
  host,
}: HeaderViewProps) => {
  const { token } = theme.useToken();
  const tableRef = useRef<HTMLDivElement>(null);

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

  const dataSource = useMemo<HeaderItem[]>(() => {
    if (!headers) return [];
    return headers.map(([name, value], index) => ({
      key: String(index),
      name,
      value,
    }));
  }, [headers]);

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
    if (isTunnel && host) {
      return (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            minHeight: 200,
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
        >
          <Button
            type="primary"
            icon={<LockOutlined />}
            onClick={handleAddToInterceptList}
            size="large"
          >
            Intercept this domain
          </Button>
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
