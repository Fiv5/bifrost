import type { ReactNode, CSSProperties } from "react";
import { useMemo, useState, useCallback } from "react";
import { theme, Dropdown, Tooltip, message } from "antd";
import {
  CheckOutlined,
  EllipsisOutlined,
  PushpinOutlined,
  LockOutlined,
  UnlockOutlined,
} from "@ant-design/icons";
import type { MenuProps } from "antd";
import type { ItemType } from "antd/es/menu/interface";
import {
  useFilterPanelStore,
  isPinned,
  type FilterType,
} from "../../stores/useFilterPanelStore";
import { useTlsConfigStore } from "../../stores/useTlsConfigStore";

interface FilterItemProps {
  label: string;
  value: string;
  type: FilterType;
  selected: boolean;
  onSelect: () => void;
  onPin: () => void;
  icon?: ReactNode;
  searchKeyword?: string;
}

function HighlightText({
  text,
  keyword,
  highlightColor,
}: {
  text: string;
  keyword?: string;
  highlightColor: string;
}) {
  if (!keyword || !keyword.trim()) {
    return <>{text}</>;
  }

  const lowerText = text.toLowerCase();
  const lowerKeyword = keyword.toLowerCase();
  const index = lowerText.indexOf(lowerKeyword);

  if (index === -1) {
    return <>{text}</>;
  }

  const before = text.slice(0, index);
  const match = text.slice(index, index + keyword.length);
  const after = text.slice(index + keyword.length);

  return (
    <>
      {before}
      <span style={{ backgroundColor: highlightColor, borderRadius: 2 }}>
        {match}
      </span>
      {after}
    </>
  );
}

export default function FilterItem({
  label,
  value,
  type,
  selected,
  onSelect,
  onPin,
  icon,
  searchKeyword,
}: FilterItemProps) {
  const { token } = theme.useToken();
  const [isHovering, setIsHovering] = useState(false);
  const filterPanelState = useFilterPanelStore();
  const alreadyPinned = isPinned(filterPanelState, type, value);

  const {
    isAppInIntercept,
    isAppInPassthrough,
    isDomainInIntercept,
    isDomainInPassthrough,
    addAppToIntercept,
    removeAppFromIntercept,
    addAppToPassthrough,
    removeAppFromPassthrough,
    addDomainToIntercept,
    removeDomainFromIntercept,
    addDomainToPassthrough,
    removeDomainFromPassthrough,
    config: tlsConfig,
  } = useTlsConfigStore();

  const isAppType = type === "client_app";
  const isDomainType = type === "domain";

  const inIntercept = isAppType
    ? isAppInIntercept(value)
    : isDomainType
      ? isDomainInIntercept(value)
      : false;

  const inPassthrough = isAppType
    ? isAppInPassthrough(value)
    : isDomainType
      ? isDomainInPassthrough(value)
      : false;

  const handleEnableTlsIntercept = useCallback(async () => {
    let success = false;
    if (isAppType) {
      success = await addAppToIntercept(value);
      if (success) {
        message.success(
          `Enabled TLS interception for "${value}" and disconnected active connections`
        );
      }
    } else if (isDomainType) {
      success = await addDomainToIntercept(value);
      if (success) {
        message.success(
          `Enabled TLS interception for "${value}" and disconnected active connections`
        );
      }
    }
    if (!success) {
      message.error("Failed to enable TLS interception");
    }
  }, [isAppType, isDomainType, value, addAppToIntercept, addDomainToIntercept]);

  const handleDisableTlsIntercept = useCallback(async () => {
    let success = false;
    if (isAppType) {
      success = await removeAppFromIntercept(value);
    } else if (isDomainType) {
      success = await removeDomainFromIntercept(value);
    }
    if (success) {
      message.success(`Removed "${value}" from TLS interception list`);
    } else {
      message.error("Failed to disable TLS interception");
    }
  }, [
    isAppType,
    isDomainType,
    value,
    removeAppFromIntercept,
    removeDomainFromIntercept,
  ]);

  const handleEnablePassthrough = useCallback(async () => {
    let success = false;
    if (isAppType) {
      success = await addAppToPassthrough(value);
    } else if (isDomainType) {
      success = await addDomainToPassthrough(value);
    }
    if (success) {
      message.success(`Added "${value}" to TLS passthrough list`);
    } else {
      message.error("Failed to enable TLS passthrough");
    }
  }, [isAppType, isDomainType, value, addAppToPassthrough, addDomainToPassthrough]);

  const handleDisablePassthrough = useCallback(async () => {
    let success = false;
    if (isAppType) {
      success = await removeAppFromPassthrough(value);
    } else if (isDomainType) {
      success = await removeDomainFromPassthrough(value);
    }
    if (success) {
      message.success(`Removed "${value}" from TLS passthrough list`);
    } else {
      message.error("Failed to disable TLS passthrough");
    }
  }, [
    isAppType,
    isDomainType,
    value,
    removeAppFromPassthrough,
    removeDomainFromPassthrough,
  ]);

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
        borderLeft: selected
          ? `2px solid ${token.colorPrimary}`
          : "2px solid transparent",
        transition: "all 0.15s",
      },
      containerHover: {
        backgroundColor: selected
          ? token.colorPrimaryBg
          : token.colorBgTextHover,
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
      tlsIcon: {
        fontSize: 10,
        flexShrink: 0,
        marginLeft: -4,
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

  const menuItems: MenuProps["items"] = useMemo(() => {
    const items: ItemType[] = [
      {
        key: "pin",
        label: alreadyPinned ? "Already pinned" : "Pin this filter",
        icon: <PushpinOutlined />,
        disabled: alreadyPinned,
      },
    ];

    if ((isAppType || isDomainType) && tlsConfig) {
      items.push({ type: "divider" });

      if (inIntercept) {
        items.push({
          key: "disable-intercept",
          label: "Remove from TLS Intercept",
          icon: <LockOutlined style={{ color: token.colorWarning }} />,
        });
      } else {
        items.push({
          key: "enable-intercept",
          label: "Enable TLS Intercept",
          icon: <UnlockOutlined style={{ color: token.colorSuccess }} />,
        });
      }

      if (inPassthrough) {
        items.push({
          key: "disable-passthrough",
          label: "Remove from Passthrough",
          icon: <UnlockOutlined style={{ color: token.colorSuccess }} />,
        });
      } else {
        items.push({
          key: "enable-passthrough",
          label: "Add to Passthrough (No Intercept)",
          icon: <LockOutlined style={{ color: token.colorWarning }} />,
        });
      }
    }

    return items;
  }, [
    alreadyPinned,
    isAppType,
    isDomainType,
    tlsConfig,
    inIntercept,
    inPassthrough,
    token,
  ]);

  const handleMenuClick: MenuProps["onClick"] = useCallback(
    ({ key }: { key: string }) => {
      switch (key) {
        case "pin":
          if (!alreadyPinned) {
            onPin();
          }
          break;
        case "enable-intercept":
          handleEnableTlsIntercept();
          break;
        case "disable-intercept":
          handleDisableTlsIntercept();
          break;
        case "enable-passthrough":
          handleEnablePassthrough();
          break;
        case "disable-passthrough":
          handleDisablePassthrough();
          break;
      }
    },
    [
      alreadyPinned,
      onPin,
      handleEnableTlsIntercept,
      handleDisableTlsIntercept,
      handleEnablePassthrough,
      handleDisablePassthrough,
    ]
  );

  const tlsIndicator = useMemo(() => {
    if (inIntercept) {
      return (
        <Tooltip title="TLS Interception Enabled (Decrypted)">
          <UnlockOutlined
            style={{ ...styles.tlsIcon, color: token.colorSuccess }}
          />
        </Tooltip>
      );
    }
    if (inPassthrough) {
      return (
        <Tooltip title="TLS Passthrough (Encrypted)">
          <LockOutlined
            style={{ ...styles.tlsIcon, color: token.colorWarning }}
          />
        </Tooltip>
      );
    }
    return null;
  }, [inIntercept, inPassthrough, styles.tlsIcon, token]);

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
        <span style={styles.label}>
          <HighlightText
            text={label}
            keyword={searchKeyword}
            highlightColor={token.colorWarningBg}
          />
        </span>
      </Tooltip>
      {tlsIndicator}
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
