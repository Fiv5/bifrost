import { useMemo, useRef } from 'react';
import { Table, Typography, theme, ConfigProvider } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type { SessionTargetSearchState } from '../../../../types';
import { useMarkSearch } from '../../hooks/useMarkSearch';

const { Text } = Typography;

interface QueryViewProps {
  url: string;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

interface QueryItem {
  key: string;
  name: string;
  value: string;
}

export const QueryView = ({ url, searchValue, onSearch }: QueryViewProps) => {
  const { token } = theme.useToken();
  const tableRef = useRef<HTMLDivElement>(null);

  const dataSource = useMemo<QueryItem[]>(() => {
    try {
      const urlObj = new URL(url);
      const items: QueryItem[] = [];
      let index = 0;
      urlObj.searchParams.forEach((value, name) => {
        items.push({
          key: String(index++),
          name,
          value,
        });
      });
      return items;
    } catch {
      return [];
    }
  }, [url]);

  const filteredData = useMemo(() => {
    if (!searchValue.value) return dataSource;
    const searchLower = searchValue.value.toLowerCase();
    return dataSource.filter(
      (item) =>
        item.name.toLowerCase().includes(searchLower) ||
        item.value.toLowerCase().includes(searchLower)
    );
  }, [dataSource, searchValue.value]);

  useMarkSearch(searchValue, () => tableRef.current, onSearch);

  const columns: ColumnsType<QueryItem> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      width: 180,
      render: (text: string) => (
        <Text strong style={{ fontFamily: 'monospace', fontSize: 12 }}>
          {text}
        </Text>
      ),
    },
    {
      title: 'Value',
      dataIndex: 'value',
      key: 'value',
      render: (text: string) => (
        <Text style={{ fontFamily: 'monospace', fontSize: 12 }} copyable={{ text }}>
          {text}
        </Text>
      ),
    },
  ];

  if (dataSource.length === 0) {
    return (
      <Text type="secondary" style={{ padding: 8, display: 'block' }}>
        No query parameters
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
