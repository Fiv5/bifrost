import { useMemo, useRef } from 'react';
import { Table, Typography, theme } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type { SessionTargetSearchState } from '../../../../types';
import { useMarkSearch } from '../../hooks/useMarkSearch';

const { Text } = Typography;

interface CookieViewProps {
  headers: [string, string][] | null;
  type: 'request' | 'response';
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

interface CookieItem {
  key: string;
  name: string;
  value: string;
}

const parseCookies = (
  headers: [string, string][] | null,
  type: 'request' | 'response'
): CookieItem[] => {
  if (!headers) return [];

  if (type === 'request') {
    const cookieHeader = headers.find(
      ([name]) => name.toLowerCase() === 'cookie'
    );
    if (!cookieHeader) return [];

    return cookieHeader[1]
      .split(';')
      .map((part) => part.trim())
      .filter((part) => part)
      .map((part, index) => {
        const [name, ...valueParts] = part.split('=');
        return {
          key: String(index),
          name: name.trim(),
          value: valueParts.join('=').trim(),
        };
      })
      .sort((a, b) => a.name.localeCompare(b.name));
  }

  const setCookieHeaders = headers.filter(
    ([name]) => name.toLowerCase() === 'set-cookie'
  );
  const cookies: CookieItem[] = [];
  let index = 0;

  setCookieHeaders.forEach(([, value]) => {
    value.split(';').forEach((part) => {
      const trimmed = part.trim();
      if (!trimmed) return;

      const [name, ...valueParts] = trimmed.split('=');
      cookies.push({
        key: String(index++),
        name: name.trim(),
        value: valueParts.length > 0 ? valueParts.join('=').trim() : 'true',
      });
    });
    cookies.push({ key: String(index++), name: '---', value: '---' });
  });

  if (cookies.length > 0 && cookies[cookies.length - 1].name === '---') {
    cookies.pop();
  }

  return cookies;
};

export const CookieView = ({
  headers,
  type,
  searchValue,
  onSearch,
}: CookieViewProps) => {
  const { token } = theme.useToken();
  const tableRef = useRef<HTMLDivElement>(null);

  const dataSource = useMemo(() => parseCookies(headers, type), [headers, type]);

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

  const columns: ColumnsType<CookieItem> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      width: 180,
      render: (text: string) =>
        text === '---' ? null : (
          <Text strong style={{ fontFamily: 'monospace', fontSize: 12 }}>
            {text}
          </Text>
        ),
    },
    {
      title: 'Value',
      dataIndex: 'value',
      key: 'value',
      render: (text: string) =>
        text === '---' ? null : (
          <Text style={{ fontFamily: 'monospace', fontSize: 12 }} copyable={{ text }}>
            {text}
          </Text>
        ),
    },
  ];

  if (dataSource.length === 0) {
    return (
      <Text type="secondary" style={{ padding: 8, display: 'block' }}>
        No {type === 'request' ? 'cookies' : 'set-cookies'}
      </Text>
    );
  }

  return (
    <div ref={tableRef} className="compact-table">
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
      <style>{`
        .compact-table .ant-table-small .ant-table-tbody > tr > td {
          padding: 4px 8px;
        }
        .compact-table .ant-table-small .ant-table-thead > tr > th {
          padding: 4px 8px;
        }
      `}</style>
    </div>
  );
};
