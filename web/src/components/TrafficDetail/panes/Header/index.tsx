import { useMemo, useRef } from "react";
import { Table, Typography, theme, ConfigProvider } from "antd";
import type { ColumnsType } from "antd/es/table";
import type { SessionTargetSearchState } from "../../../../types";
import { useMarkSearch } from "../../hooks/useMarkSearch";

const { Text } = Typography;

interface HeaderViewProps {
  headers: [string, string][] | null;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
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
}: HeaderViewProps) => {
  const { token } = theme.useToken();
  const tableRef = useRef<HTMLDivElement>(null);

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
