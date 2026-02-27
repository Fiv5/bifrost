import type { ReactNode, CSSProperties } from "react";
import { useMemo, useState } from "react";
import { theme, Dropdown, Tooltip } from "antd";
import { CheckOutlined, EllipsisOutlined, PushpinOutlined } from "@ant-design/icons";
import type { MenuProps } from "antd";
import { useFilterPanelStore, isPinned, type FilterType } from "../../stores/useFilterPanelStore";

interface FilterItemProps {
  label: string;
  value: string;
  type: FilterType;
  selected: boolean;
  onSelect: () => void;
  onPin: () => void;
  icon?: ReactNode;
}

export default function FilterItem({
  label,
  value,
  type,
  selected,
  onSelect,
  onPin,
  icon,
}: FilterItemProps) {
  const { token } = theme.useToken();
  const [isHovering, setIsHovering] = useState(false);
  const filterPanelState = useFilterPanelStore();
  const alreadyPinned = isPinned(filterPanelState, type, value);

  const styles = useMemo<Record<string, CSSProperties>>(
    () => ({
      container: {
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "5px 8px 5px 20px",
        cursor: "pointer",
        userSelect: "none",
        backgroundColor: selected ? token.colorPrimaryBg : "transparent",
        borderLeft: selected ? `2px solid ${token.colorPrimary}` : "2px solid transparent",
        transition: "all 0.15s",
      },
      containerHover: {
        backgroundColor: selected ? token.colorPrimaryBg : token.colorBgTextHover,
      },
      icon: {
        flexShrink: 0,
      },
      label: {
        flex: 1,
        fontSize: 12,
        color: selected ? token.colorPrimary : token.colorText,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap" as const,
      },
      checkIcon: {
        fontSize: 12,
        color: token.colorPrimary,
        flexShrink: 0,
      },
      moreBtn: {
        fontSize: 14,
        color: token.colorTextSecondary,
        padding: 2,
        borderRadius: 4,
        opacity: isHovering ? 1 : 0,
        transition: "opacity 0.15s",
        cursor: "pointer",
      },
      moreBtnHover: {
        backgroundColor: token.colorBgTextHover,
      },
    }),
    [token, selected, isHovering]
  );

  const menuItems: MenuProps["items"] = [
    {
      key: "pin",
      label: alreadyPinned ? "Already pinned" : "Pin this filter",
      icon: <PushpinOutlined />,
      disabled: alreadyPinned,
    },
  ];

  const handleMenuClick: MenuProps["onClick"] = ({ key }) => {
    if (key === "pin" && !alreadyPinned) {
      onPin();
    }
  };

  return (
    <div
      style={{
        ...styles.container,
        ...(isHovering ? styles.containerHover : {}),
      }}
      onMouseEnter={() => setIsHovering(true)}
      onMouseLeave={() => setIsHovering(false)}
      onClick={onSelect}
    >
      {icon && <span style={styles.icon}>{icon}</span>}
      <Tooltip title={label} placement="right" mouseEnterDelay={0.5}>
        <span style={styles.label}>{label}</span>
      </Tooltip>
      {selected && <CheckOutlined style={styles.checkIcon} />}
      <Dropdown
        menu={{ items: menuItems, onClick: handleMenuClick }}
        trigger={["click"]}
        placement="bottomRight"
      >
        <span
          style={styles.moreBtn}
          onClick={(e) => e.stopPropagation()}
          onMouseEnter={(e) => {
            e.currentTarget.style.backgroundColor = token.colorBgTextHover;
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.backgroundColor = "transparent";
          }}
        >
          <EllipsisOutlined />
        </span>
      </Dropdown>
    </div>
  );
}
