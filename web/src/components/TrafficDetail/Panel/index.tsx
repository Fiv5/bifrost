import { type ReactNode, useCallback, useMemo } from 'react';
import { Typography, Space, Divider, theme } from 'antd';
import { FilterOutlined, DownOutlined, UpOutlined } from '@ant-design/icons';
import type {
  SessionTargetSearchState,
  DisplayFormat,
  RecordContentType,
} from '../../../types';
import { Search } from '../Search';
import { BodyTypeMenu } from '../panes/Body/BodyTypeMenu';

const { Text } = Typography;

interface TabConfig {
  key: string;
  label: string;
  children: ReactNode;
  enable?: boolean;
}

interface PanelProps {
  name: 'Request' | 'Response';
  tabs: TabConfig[];
  activeTab: string;
  onTabChange: (tab: string) => void;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  displayFormat?: DisplayFormat;
  onDisplayFormatChange?: (format: string) => void;
  contentType?: RecordContentType;
  collapsed?: boolean;
  onCollapsedChange?: (collapsed: boolean) => void;
  keepAliveTabs?: string[];
  contentOverflow?: 'auto' | 'hidden';
}

export const Panel = ({
  name,
  tabs,
  activeTab,
  onTabChange,
  searchValue,
  onSearch,
  displayFormat,
  onDisplayFormatChange,
  contentType,
  collapsed = false,
  onCollapsedChange,
  keepAliveTabs,
  contentOverflow = 'auto',
}: PanelProps) => {
  const { token } = theme.useToken();

  const enabledTabs = tabs.filter((tab) => tab.enable !== false);
  const keepAliveSet = useMemo(
    () => new Set(keepAliveTabs ?? []),
    [keepAliveTabs],
  );

  const handleToggleSearch = useCallback(() => {
    onSearch({ show: !searchValue.show });
  }, [onSearch, searchValue.show]);

  const handleToggleCollapsed = useCallback(() => {
    onCollapsedChange?.(!collapsed);
  }, [onCollapsedChange, collapsed]);

  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '4px 0',
          flexShrink: 0,
        }}
      >
        <Space size="middle">
          <Text strong style={{ fontSize: 14 }}>
            {name}
          </Text>
          {enabledTabs.map((tab) => (
            <Text
              key={tab.key}
              type={activeTab === tab.key ? undefined : 'secondary'}
              style={{
                cursor: 'pointer',
                fontWeight: activeTab === tab.key ? 500 : 400,
                color: activeTab === tab.key ? token.colorPrimary : undefined,
              }}
              onClick={() => onTabChange(tab.key)}
              data-testid={`${name.toLowerCase()}-tab-${tab.key.toLowerCase()}`}
            >
              {tab.label}
            </Text>
          ))}
        </Space>

        <Space size="small">
          {activeTab === 'Body' && displayFormat && onDisplayFormatChange && contentType && (
            <BodyTypeMenu
              value={displayFormat}
              onChange={onDisplayFormatChange}
              contentType={contentType}
            />
          )}
          <FilterOutlined
            onClick={handleToggleSearch}
            style={{
              cursor: 'pointer',
              color: searchValue.show ? token.colorPrimary : token.colorTextSecondary,
              fontSize: 14,
            }}
          />
          {collapsed ? (
            <DownOutlined
              onClick={handleToggleCollapsed}
              style={{
                cursor: 'pointer',
                color: token.colorTextSecondary,
                fontSize: 14,
              }}
            />
          ) : (
            <UpOutlined
              onClick={handleToggleCollapsed}
              style={{
                cursor: 'pointer',
                color: token.colorTextSecondary,
                fontSize: 14,
              }}
            />
          )}
        </Space>
      </div>

      <div
        style={{
          display: collapsed ? 'none' : 'flex',
          flex: 1,
          minHeight: 0,
          flexDirection: 'column',
        }}
      >
        <Divider style={{ margin: '0 0 4px 0' }} />

        <Search value={searchValue} onSearch={onSearch} />

        <div
          style={{
            flex: 1,
            minHeight: 0,
            overflow: contentOverflow,
          }}
        >
          {enabledTabs.map((tab) => {
            const isActive = tab.key === activeTab;
            if (!isActive && !keepAliveSet.has(tab.key)) {
              return null;
            }
            return (
              <div
                key={tab.key}
                style={{ display: isActive ? 'block' : 'none', height: '100%' }}
              >
                {tab.children}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
