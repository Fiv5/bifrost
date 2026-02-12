import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { Tooltip, theme } from "antd";
import {
  GlobalOutlined,
  FileTextOutlined,
  SettingOutlined,
  SafetyOutlined,
  DatabaseOutlined,
} from "@ant-design/icons";
import type { CSSProperties } from "react";

interface MenuItem {
  key: string;
  icon: React.ReactNode;
  label: string;
}

const menuItems: MenuItem[] = [
  { key: "/traffic", icon: <GlobalOutlined />, label: "Network" },
  { key: "/rules", icon: <FileTextOutlined />, label: "Rules" },
  { key: "/values", icon: <DatabaseOutlined />, label: "Values" },
  { key: "/whitelist", icon: <SafetyOutlined />, label: "Whitelist" },
  { key: "/settings", icon: <SettingOutlined />, label: "Settings" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { token } = theme.useToken();

  const styles: Record<string, CSSProperties> = {
    layout: {
      display: "flex",
      height: "100vh",
      width: "100vw",
      overflow: "hidden",
    },
    sidebar: {
      width: 48,
      height: "100%",
      backgroundColor: token.colorBgContainer,
      borderRight: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      paddingTop: 8,
      flexShrink: 0,
    },
    menuItem: {
      width: 48,
      height: 48,
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      cursor: "pointer",
      fontSize: 18,
      color: token.colorTextSecondary,
      position: "relative",
      transition: "all 0.2s",
    },
    menuItemActive: {
      color: token.colorPrimary,
      backgroundColor: token.colorPrimaryBg,
    },
    activeBorder: {
      position: "absolute",
      left: 0,
      top: 8,
      bottom: 8,
      width: 3,
      backgroundColor: token.colorPrimary,
      borderRadius: "0 2px 2px 0",
    },
    content: {
      flex: 1,
      display: "flex",
      flexDirection: "column",
      overflow: "auto",
      backgroundColor: token.colorBgLayout,
    },
  };

  const handleClick = (key: string) => {
    navigate(key);
  };

  const isActive = (key: string) => {
    if (key === "/traffic" && location.pathname === "/") return true;
    return location.pathname === key;
  };

  return (
    <div style={styles.layout}>
      <div style={styles.sidebar}>
        {menuItems.map((item) => {
          const active = isActive(item.key);
          return (
            <Tooltip key={item.key} title={item.label} placement="right">
              <div
                style={{
                  ...styles.menuItem,
                  ...(active ? styles.menuItemActive : {}),
                }}
                onClick={() => handleClick(item.key)}
              >
                {active && <div style={styles.activeBorder as CSSProperties} />}
                {item.icon}
              </div>
            </Tooltip>
          );
        })}
      </div>
      <div style={styles.content}>
        <Outlet />
      </div>
    </div>
  );
}
