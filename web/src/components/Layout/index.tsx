import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { theme, Badge } from "antd";
import {
  GlobalOutlined,
  FileTextOutlined,
  SettingOutlined,
  DatabaseOutlined,
  CodeOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import type { CSSProperties } from "react";
import { useEffect } from "react";
import { usePendingAuthStore } from "../../stores/usePendingAuthStore";
import StatusBar from "../StatusBar";
import { setNavigateCallback, type ReferenceLocation } from "../BifrostEditor";
import DesktopWindowChrome, { DESKTOP_CHROME_HEIGHT } from "./DesktopWindowChrome";
import { getDesktopPlatform, isDesktopShell } from "../../runtime";

interface MenuItem {
  key: string;
  icon: React.ReactNode;
  label: string;
}

const menuItems: MenuItem[] = [
  { key: "/traffic", icon: <GlobalOutlined />, label: "Network" },
  { key: "/replay", icon: <ThunderboltOutlined />, label: "Replay" },
  { key: "/rules", icon: <FileTextOutlined />, label: "Rules" },
  { key: "/values", icon: <DatabaseOutlined />, label: "Values" },
  { key: "/scripts", icon: <CodeOutlined />, label: "Scripts" },
  { key: "/settings", icon: <SettingOutlined />, label: "Settings" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const { token } = theme.useToken();
  const {
    pendingCount,
    startSSE,
    stopSSE,
    fetchPendingList,
    requestNotificationPermission,
  } = usePendingAuthStore();
  const desktopEnabled = isDesktopShell();
  const desktopPlatform = getDesktopPlatform();

  useEffect(() => {
    fetchPendingList();
    startSSE();
    requestNotificationPermission();
    return () => {
      stopSSE();
    };
  }, [fetchPendingList, startSSE, stopSSE, requestNotificationPermission]);

  useEffect(() => {
    const handleNavigate = (location: ReferenceLocation) => {
      if (location.uri) {
        navigate(location.uri);
      }
    };
    setNavigateCallback(handleNavigate);
    return () => {
      setNavigateCallback(null);
    };
  }, [navigate]);

  const styles: Record<string, CSSProperties> = {
    layout: {
      display: "flex",
      flexDirection: "column",
      height: "100vh",
      width: "100vw",
      overflow: "hidden",
      position: "relative",
      backgroundColor: token.colorBgLayout,
    },
    macTopWash: {
      position: "absolute",
      top: 0,
      left: 0,
      right: 0,
      height: 124,
      background:
        "linear-gradient(180deg, rgba(248,250,253,0.96) 0%, rgba(248,250,253,0.72) 52%, rgba(248,250,253,0) 100%)",
      pointerEvents: "none",
      zIndex: 1,
    },
    macSidebarWash: {
      position: "absolute",
      top: 0,
      left: 0,
      width: 112,
      bottom: 20,
      background:
        "linear-gradient(180deg, rgba(246,248,251,0.98) 0%, rgba(246,248,251,0.92) 72px, rgba(246,248,251,0.82) 100%)",
      borderRight: `1px solid rgba(15, 23, 42, 0.06)`,
      pointerEvents: "none",
      zIndex: 1,
    },
    main: {
      display: "flex",
      flex: 1,
      overflow: "hidden",
      paddingTop: desktopEnabled ? DESKTOP_CHROME_HEIGHT : 0,
      position: "relative",
      zIndex: 2,
    },
    sidebar: {
      width: 50,
      height: "100%",
      background:
        desktopEnabled && desktopPlatform === "macos"
          ? "linear-gradient(180deg, rgba(249,250,252,0.92) 0%, rgba(249,250,252,0.84) 72px, rgba(255,255,255,0.88) 100%)"
          : token.colorBgContainer,
      borderRight: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      paddingTop: desktopEnabled && desktopPlatform === "macos" ? 10 : 8,
      flexShrink: 0,
      backdropFilter:
        desktopEnabled && desktopPlatform === "macos"
          ? "blur(14px) saturate(1.08)"
          : undefined,
    },
    menuItem: {
      width: 50,
      height: 64,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      justifyContent: "center",
      cursor: "pointer",
      fontSize: 18,
      color: token.colorTextSecondary,
      position: "relative",
      transition: "all 0.2s",
    },
    menuItemLabel: {
      marginTop: 4,
      fontSize: 9,
      lineHeight: "9px",
      whiteSpace: "nowrap",
      color: "inherit",
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
      background:
        desktopEnabled && desktopPlatform === "macos"
          ? "linear-gradient(180deg, rgba(249,251,253,0.84) 0%, rgba(249,251,253,0.32) 88px, transparent 160px), linear-gradient(90deg, rgba(246,248,251,0.42) 0%, rgba(246,248,251,0) 120px), " +
            token.colorBgLayout
          : token.colorBgLayout,
    },
  };

  const handleClick = (key: string) => {
    navigate(key);
  };

  const isActive = (key: string) => {
    if (key === "/traffic" && location.pathname === "/") return true;
    return location.pathname === key;
  };

  const renderMenuIcon = (item: MenuItem) => {
    if (item.key === "/settings" && pendingCount > 0) {
      return (
        <Badge count={pendingCount} size="small" offset={[4, -4]}>
          {item.icon}
        </Badge>
      );
    }
    return item.icon;
  };

  return (
    <div style={styles.layout}>
      {desktopEnabled && desktopPlatform === "macos" ? (
        <>
          <div style={styles.macTopWash} />
          <div style={styles.macSidebarWash} />
        </>
      ) : null}
      <DesktopWindowChrome />
      <div style={styles.main}>
        <div style={styles.sidebar}>
          {menuItems.map((item) => {
            const active = isActive(item.key);
            return (
              <div
                style={{
                  ...styles.menuItem,
                  ...(active ? styles.menuItemActive : {}),
                }}
                onClick={() => handleClick(item.key)}
              >
                {active && <div style={styles.activeBorder as CSSProperties} />}
                {renderMenuIcon(item)}
                <div style={styles.menuItemLabel}>{item.label}</div>
              </div>
            );
          })}
        </div>
        <div style={styles.content}>
          <Outlet />
        </div>
      </div>
      <StatusBar />
    </div>
  );
}
