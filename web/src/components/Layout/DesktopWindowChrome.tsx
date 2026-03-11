import { useEffect, useState } from "react";
import { theme } from "antd";
import { getCurrentDesktopWindow } from "../../desktop/tauri";
import { getDesktopPlatform, isDesktopShell } from "../../runtime";

export const DESKTOP_CHROME_HEIGHT = 46;

const WINDOWS_BUTTON_WIDTH = 46;
type CaptionButtonKey = "minimize" | "maximize" | "close";

export default function DesktopWindowChrome() {
  const { token } = theme.useToken();
  const [isMaximized, setIsMaximized] = useState(false);
  const [hoveredButton, setHoveredButton] = useState<CaptionButtonKey | null>(null);
  const [pressedButton, setPressedButton] = useState<CaptionButtonKey | null>(null);

  if (!isDesktopShell()) {
    return null;
  }

  const platform = getDesktopPlatform();
  const isMac = platform === "macos";
  const isWindows = platform === "windows";
  const currentWindow = getCurrentDesktopWindow();

  useEffect(() => {
    if (!isWindows) {
      return;
    }

    currentWindow?.isMaximized().then(setIsMaximized).catch(() => undefined);
  }, [currentWindow, isWindows]);

  const styles = {
    shell: {
      position: "absolute" as const,
      top: 0,
      left: 0,
      right: 0,
      height: DESKTOP_CHROME_HEIGHT,
      display: "flex",
      alignItems: "stretch",
      justifyContent: "space-between",
      zIndex: 30,
      pointerEvents: "none" as const,
      background: isMac
        ? "linear-gradient(180deg, rgba(251,252,254,0.92) 0%, rgba(251,252,254,0.64) 62%, rgba(251,252,254,0) 100%)"
        : token.colorBgContainer,
      borderBottom: isWindows ? `1px solid ${token.colorBorderSecondary}` : "none",
      backdropFilter: isMac ? "blur(18px) saturate(1.15)" : undefined,
    },
    macSidebarBlend: {
      position: "absolute" as const,
      inset: 0,
      width: 124,
      background:
        "linear-gradient(90deg, rgba(247,249,252,0.88) 0%, rgba(247,249,252,0.62) 58%, rgba(247,249,252,0) 100%)",
      pointerEvents: "none" as const,
    },
    dragRegion: {
      flex: 1,
      display: "flex",
      alignItems: "center",
      paddingLeft: isMac ? 88 : 16,
      paddingRight: isWindows ? 8 : 24,
      color: token.colorTextSecondary,
      fontSize: 12,
      letterSpacing: isMac ? "0.02em" : "0.08em",
      textTransform: isMac ? "none" : ("uppercase" as const),
      userSelect: "none" as const,
      pointerEvents: "auto" as const,
      position: "relative" as const,
    },
    macTitle: {
      fontSize: 12,
      fontWeight: 500,
      color: "rgba(0,0,0,0.56)",
    },
    controls: {
      display: "flex",
      alignItems: "stretch",
      pointerEvents: "auto" as const,
    },
  };

  const renderCaptionIcon = (kind: Exclude<CaptionButtonKey, "close">) => {
    const strokeWidth = 1.1;
    const common = {
      width: 10,
      height: 10,
      viewBox: "0 0 10 10",
      fill: "none",
      stroke: "currentColor",
      strokeWidth,
      strokeLinecap: "square" as const,
      shapeRendering: "crispEdges" as const,
    };

    if (kind === "minimize") {
      return (
        <svg aria-hidden="true" {...common}>
          <path d="M1.5 5h7" />
        </svg>
      );
    }

    if (isMaximized) {
      return (
        <svg aria-hidden="true" {...common}>
          <path d="M2 1.5h5v5H2z" />
          <path d="M3 3.5h5v5H3z" />
        </svg>
      );
    }

    return (
      <svg aria-hidden="true" {...common}>
        <path d="M2 2h6v6H2z" />
      </svg>
    );
  };

  const renderCloseIcon = () => (
    <svg
      aria-hidden="true"
      width="10"
      height="10"
      viewBox="0 0 10 10"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.1"
      strokeLinecap="square"
      shapeRendering="crispEdges"
    >
      <path d="M2 2l6 6" />
      <path d="M8 2L2 8" />
    </svg>
  );

  const getButtonStyle = (kind: CaptionButtonKey) => {
    const hovered = hoveredButton === kind;
    const pressed = pressedButton === kind;
    const danger = kind === "close";
    return {
      width: WINDOWS_BUTTON_WIDTH,
      height: DESKTOP_CHROME_HEIGHT,
      border: "none",
      background: danger
        ? pressed
          ? "#c50f1f"
          : hovered
            ? "#e81123"
            : "transparent"
        : pressed
          ? "rgba(0, 0, 0, 0.10)"
          : hovered
            ? "rgba(0, 0, 0, 0.06)"
            : "transparent",
      color: danger && (hovered || pressed) ? "#fff" : token.colorText,
      cursor: "pointer",
      display: "grid",
      placeItems: "center",
      transition: "background-color 120ms ease, color 120ms ease",
      outline: "none",
      padding: 0,
    };
  };

  const bindButtonState = (kind: CaptionButtonKey) => ({
    onMouseEnter: () => setHoveredButton(kind),
    onMouseLeave: () => {
      setHoveredButton((current) => (current === kind ? null : current));
      setPressedButton((current) => (current === kind ? null : current));
    },
    onMouseDown: () => setPressedButton(kind),
    onMouseUp: () => {
      setPressedButton((current) => (current === kind ? null : current));
    },
  });

  return (
    <div style={styles.shell}>
      {isMac ? <div style={styles.macSidebarBlend} /> : null}
      <div
        data-tauri-drag-region
        style={styles.dragRegion}
        onDoubleClick={() => {
          if (!isWindows) {
            return;
          }
          currentWindow?.toggleMaximize().then(async () => {
            const next = await currentWindow?.isMaximized();
            setIsMaximized(Boolean(next));
          }).catch(() => undefined);
        }}
      >
        {isMac ? <span style={styles.macTitle}>Bifrost</span> : <span>Bifrost</span>}
      </div>
      {isWindows ? (
        <div style={styles.controls}>
          <button
            type="button"
            aria-label="Minimize window"
            style={getButtonStyle("minimize")}
            {...bindButtonState("minimize")}
            onClick={() => {
              currentWindow?.minimize().catch(() => undefined);
            }}
          >
            {renderCaptionIcon("minimize")}
          </button>
          <button
            type="button"
            aria-label={isMaximized ? "Restore window" : "Maximize window"}
            style={getButtonStyle("maximize")}
            {...bindButtonState("maximize")}
            onClick={() => {
              currentWindow?.toggleMaximize().then(async () => {
                const next = await currentWindow?.isMaximized();
                setIsMaximized(Boolean(next));
              }).catch(() => undefined);
            }}
          >
            {renderCaptionIcon("maximize")}
          </button>
          <button
            type="button"
            aria-label="Close window"
            style={getButtonStyle("close")}
            {...bindButtonState("close")}
            onClick={() => {
              currentWindow?.close().catch(() => undefined);
            }}
          >
            {renderCloseIcon()}
          </button>
        </div>
      ) : null}
    </div>
  );
}
