import { useMemo, useState, useCallback } from 'react';
import { Tree, Typography, theme, ConfigProvider } from 'antd';
import type { DataNode } from 'antd/es/tree';
import type { SessionTargetSearchState } from '../../../../types';

const { Text } = Typography;

interface TreeViewProps {
  data?: string | null;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

const isObject = (val: unknown): val is Record<string, unknown> =>
  val !== null && typeof val === 'object' && !Array.isArray(val);

interface TreeNodeData {
  key: string;
  label: string;
  value: string;
  isExpandable: boolean;
  children?: TreeNodeData[];
}

const buildTreeNodeData = (
  data: unknown,
  path: string = 'root'
): TreeNodeData[] => {
  if (Array.isArray(data)) {
    return data.map((item, index) => {
      const key = `${path}-${index}`;
      const isExpandable = typeof item === 'object' && item !== null;
      return {
        key,
        label: `[${index}]`,
        value: isExpandable
          ? Array.isArray(item)
            ? `Array(${item.length})`
            : `Object(${Object.keys(item).length})`
          : String(item),
        isExpandable,
        children: isExpandable ? buildTreeNodeData(item, key) : undefined,
      };
    });
  }

  if (isObject(data)) {
    return Object.entries(data).map(([k, v]) => {
      const key = `${path}-${k}`;
      const isExpandable = typeof v === 'object' && v !== null;
      return {
        key,
        label: k,
        value: isExpandable
          ? Array.isArray(v)
            ? `Array(${v.length})`
            : `Object(${Object.keys(v as object).length})`
          : String(v),
        isExpandable,
        children: isExpandable ? buildTreeNodeData(v, key) : undefined,
      };
    });
  }

  return [];
};

const convertToDataNode = (
  nodes: TreeNodeData[],
  searchValue?: string
): DataNode[] => {
  return nodes.map((node) => ({
    key: node.key,
    title: (
      <span>
        <Text strong>{node.label}: </Text>
        {node.isExpandable ? (
          <Text type="secondary">{node.value}</Text>
        ) : (
          <Text
            style={{
              background:
                searchValue &&
                node.value.toLowerCase().includes(searchValue.toLowerCase())
                  ? '#ffe58f'
                  : undefined,
            }}
          >
            {node.value}
          </Text>
        )}
      </span>
    ),
    children: node.children
      ? convertToDataNode(node.children, searchValue)
      : undefined,
  }));
};

const parseJsonSafe = (data: string): { parsed: unknown; error: boolean } => {
  try {
    return { parsed: JSON.parse(data), error: false };
  } catch {
    return { parsed: null, error: true };
  }
};

export const TreeView = ({ data, searchValue }: TreeViewProps) => {
  const { token } = theme.useToken();
  const [expandedKeys, setExpandedKeys] = useState<React.Key[]>(['root']);

  const parsedData = useMemo(() => {
    if (!data) return { parsed: null, error: false };
    return parseJsonSafe(data);
  }, [data]);

  const treeNodeData = useMemo<TreeNodeData[]>(() => {
    if (!parsedData.parsed || parsedData.error) return [];
    const parsed = parsedData.parsed;
    const rootLabel = Array.isArray(parsed)
      ? `Array(${parsed.length})`
      : `Object(${Object.keys(parsed as object).length})`;

    return [
      {
        key: 'root',
        label: 'Root',
        value: rootLabel,
        isExpandable: true,
        children: buildTreeNodeData(parsed, 'root'),
      },
    ];
  }, [parsedData]);

  const treeData = useMemo<DataNode[]>(() => {
    return convertToDataNode(treeNodeData, searchValue.value);
  }, [treeNodeData, searchValue.value]);

  const onExpand = useCallback((keys: React.Key[]) => {
    setExpandedKeys(keys);
  }, []);

  if (!data) {
    return null;
  }

  if (parsedData.error || treeData.length === 0) {
    return (
      <div style={{ padding: 12, color: token.colorTextSecondary }}>
        Unable to parse as JSON
      </div>
    );
  }

  return (
    <div
      style={{
        padding: 4,
        backgroundColor: token.colorBgLayout,
        borderRadius: 4,
      }}
    >
      <ConfigProvider
        theme={{
          components: {
            Tree: {
              titleHeight: 20,
              nodeHoverBg: 'transparent',
              nodeSelectedBg: 'transparent',
            },
          },
        }}
      >
        <Tree
          treeData={treeData}
          expandedKeys={expandedKeys}
          onExpand={onExpand}
          showLine
          selectable={false}
          style={{ background: 'transparent' }}
        />
      </ConfigProvider>
    </div>
  );
};
