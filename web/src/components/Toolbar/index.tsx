import { Tag, Switch, Button, Popconfirm, Space, theme, Tooltip } from "antd";
import {
  PauseOutlined,
  PlayCircleOutlined,
  DeleteOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
} from "@ant-design/icons";
import type { ToolbarFilters } from "../../types";

interface ToolbarProps {
  paused: boolean;
  filters: ToolbarFilters;
  onPauseToggle: () => void;
  onClear: () => void;
  onFilterChange: (filters: ToolbarFilters) => void;
  systemProxyEnabled?: boolean;
  systemProxySupported?: boolean;
  systemProxyLoading?: boolean;
  onSystemProxyToggle?: (enabled: boolean) => void;
  detailPanelCollapsed?: boolean;
  onDetailPanelToggle?: () => void;
}

const filterGroups = {
  rule: ["Hit Rule"],
  protocol: ["HTTP", "HTTPS", "H2", "WS", "WSS"],
  type: ["JSON", "Form", "XML", "JS", "CSS", "Font", "Doc", "Media", "SSE"],
  status: ["1xx", "2xx", "3xx", "4xx", "5xx", "error"],
};

export default function Toolbar({
  paused,
  filters,
  onPauseToggle,
  onClear,
  onFilterChange,
  systemProxyEnabled,
  systemProxySupported = true,
  systemProxyLoading,
  onSystemProxyToggle,
  detailPanelCollapsed,
  onDetailPanelToggle,
}: ToolbarProps) {
  const { token } = theme.useToken();

  const handleTagClick = (group: keyof ToolbarFilters, tag: string) => {
    const currentTags = filters[group];
    const newTags = currentTags.includes(tag) ? [] : [tag];
    onFilterChange({ ...filters, [group]: newTags });
  };

  const renderFilterGroup = (group: keyof ToolbarFilters, tags: string[]) => (
    <Space size={4} wrap style={{ marginRight: 8 }}>
      {tags.map((tag) => (
        <Tag.CheckableTag
          key={tag}
          checked={filters[group].includes(tag)}
          onChange={() => handleTagClick(group, tag)}
          style={{ marginInlineEnd: 0, fontSize: 12 }}
        >
          {tag}
        </Tag.CheckableTag>
      ))}
    </Space>
  );

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "6px 12px",
        background: token.colorBgContainer,
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        flexShrink: 0,
      }}
    >
      <Space size={4}>
        <Button
          type="text"
          size="small"
          icon={paused ? <PlayCircleOutlined /> : <PauseOutlined />}
          onClick={onPauseToggle}
        />
        <Popconfirm
          title="Clear all traffic?"
          description="This action cannot be undone."
          onConfirm={onClear}
          okText="Clear"
          cancelText="Cancel"
        >
          <Button type="text" size="small" icon={<DeleteOutlined />} />
        </Popconfirm>
        <div
          style={{
            width: 1,
            height: 16,
            backgroundColor: token.colorBorderSecondary,
            margin: "0 4px",
          }}
        />
      </Space>

      <div
        style={{
          display: "flex",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 4,
          flex: 1,
          marginLeft: 8,
        }}
      >
        {renderFilterGroup("rule", filterGroups.rule)}
        <div
          style={{
            width: 1,
            height: 14,
            backgroundColor: token.colorBorderSecondary,
          }}
        />
        {renderFilterGroup("protocol", filterGroups.protocol)}
        <div
          style={{
            width: 1,
            height: 14,
            backgroundColor: token.colorBorderSecondary,
          }}
        />
        {renderFilterGroup("type", filterGroups.type)}
        <div
          style={{
            width: 1,
            height: 14,
            backgroundColor: token.colorBorderSecondary,
          }}
        />
        {renderFilterGroup("status", filterGroups.status)}
      </div>

      <Space size={8}>
        <span style={{ color: token.colorTextSecondary, fontSize: 12 }}>
          System Proxy
        </span>
        {systemProxySupported ? (
          <Switch
            size="small"
            checked={systemProxyEnabled}
            loading={systemProxyLoading}
            onChange={onSystemProxyToggle}
          />
        ) : (
          <Tooltip title="System proxy is not supported on this platform">
            <Switch size="small" disabled />
          </Tooltip>
        )}
        <div
          style={{
            width: 1,
            height: 16,
            backgroundColor: token.colorBorderSecondary,
          }}
        />
        <Tooltip
          title={
            detailPanelCollapsed ? "Show detail panel" : "Hide detail panel"
          }
        >
          <Button
            type="text"
            size="small"
            icon={
              detailPanelCollapsed ? (
                <MenuUnfoldOutlined />
              ) : (
                <MenuFoldOutlined />
              )
            }
            onClick={onDetailPanelToggle}
          />
        </Tooltip>
      </Space>
    </div>
  );
}
