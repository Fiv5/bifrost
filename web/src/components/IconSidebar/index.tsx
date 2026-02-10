import { useNavigate, useLocation } from "react-router-dom";
import { Tooltip, theme } from "antd";
import {
  GlobalOutlined,
  FileTextOutlined,
  TeamOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import type { CSSProperties } from "react";

interface MenuItem {
  key: string;
  icon: React.ReactNode;
  label: string;
}

const menuItems: MenuItem[] = [
  { key: "/network", icon: <GlobalOutlined />, label: "Network" },
  { key: "/rules", icon: <FileTextOutlined />, label: "Rules" },
  { key: "/group", icon: <TeamOutlined />, label: "Group" },
  { key: "/settings", icon: <SettingOutlined />, label: "Settings" },
];

export default function IconSidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { token } = theme.useToken();

  const styles: Record<string, CSSProperties> = {
    sidebar: {
      width: 48,
      height: "100vh",
      backgroundColor: token.colorBgContainer,
      borderRight: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      paddingTop: 8,
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
  };

  const handleClick = (key: string) => {
    navigate(key);
  };

  return (
    <div style={styles.sidebar}>
      {menuItems.map((item) => {
        const isActive = location.pathname === item.key;
        return (
          <Tooltip key={item.key} title={item.label} placement="right">
            <div
              style={{
                ...styles.menuItem,
                ...(isActive ? styles.menuItemActive : {}),
              }}
              onClick={() => handleClick(item.key)}
            >
              {isActive && <div style={styles.activeBorder as CSSProperties} />}
              {item.icon}
            </div>
          </Tooltip>
        );
      })}
    </div>
  );
}
