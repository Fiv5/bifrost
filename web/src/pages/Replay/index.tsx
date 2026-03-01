import { useEffect, type CSSProperties } from "react";
import { Spin, Tooltip, message, theme } from "antd";
import { EditOutlined, HistoryOutlined } from "@ant-design/icons";
import { useReplayStore, type ReplayMode } from "../../stores/useReplayStore";
import SplitPane from "../../components/SplitPane";
import VerticalSplitPane from "../../components/VerticalSplitPane";
import CollectionPanel from "./components/CollectionPanel";
import RequestPanel from "./components/RequestPanel";
import ResponsePanel from "./components/ResponsePanel";
import HistoryView from "./components/HistoryView";

interface ModeButtonProps {
  mode: ReplayMode;
  icon: React.ReactNode;
  label: string;
  isActive: boolean;
  onClick: () => void;
}

const ModeButton = ({ icon, label, isActive, onClick }: ModeButtonProps) => {
  const { token } = theme.useToken();

  return (
    <Tooltip title={label} placement="left">
      <div
        onClick={onClick}
        style={{
          width: 32,
          height: 32,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          cursor: "pointer",
          borderRadius: 4,
          backgroundColor: isActive ? token.colorPrimaryBg : "transparent",
          color: isActive ? token.colorPrimary : token.colorTextSecondary,
          transition: "all 0.2s",
        }}
        onMouseEnter={(e) => {
          if (!isActive) {
            e.currentTarget.style.backgroundColor = token.colorBgTextHover;
          }
        }}
        onMouseLeave={(e) => {
          if (!isActive) {
            e.currentTarget.style.backgroundColor = "transparent";
          }
        }}
      >
        {icon}
      </div>
    </Tooltip>
  );
};

export default function Replay() {
  const { token } = theme.useToken();
  const {
    currentRequest,
    loading,
    executing,
    uiState,
    loadSavedRequests,
    loadRecentHistory,
    loadGroups,
    loadAllHistory,
    updateUIState,
  } = useReplayStore();

  const currentMode = uiState.mode;
  const canSwitchToHistory = currentRequest?.is_saved && currentRequest?.id;

  useEffect(() => {
    loadSavedRequests();
    loadRecentHistory();
    loadGroups();
  }, [loadSavedRequests, loadRecentHistory, loadGroups]);

  const handleModeChange = (mode: ReplayMode) => {
    if (mode === "history") {
      if (!canSwitchToHistory) {
        message.warning("Please select a saved request template first");
        return;
      }
      loadAllHistory(currentRequest!.id);
    }
    updateUIState({ mode });
  };

  const styles: Record<string, CSSProperties> = {
    container: {
      height: "100%",
      width: "100%",
      overflow: "hidden",
      backgroundColor: token.colorBgContainer,
      display: "flex",
    },
    mainContent: {
      flex: 1,
      height: "100%",
      overflow: "hidden",
    },
    toolbar: {
      width: 40,
      height: "100%",
      borderLeft: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      flexDirection: "column",
      alignItems: "center",
      paddingTop: 8,
      gap: 4,
      backgroundColor: token.colorBgLayout,
    },
    collectionPanel: {
      height: "100%",
      overflow: "hidden",
      display: "flex",
      flexDirection: "column",
    },
    centerArea: {
      height: "100%",
      overflow: "hidden",
      display: "flex",
      flexDirection: "column",
    },
    requestPanel: {
      height: "100%",
      overflow: "hidden",
      display: "flex",
      flexDirection: "column",
    },
    responsePanel: {
      height: "100%",
      overflow: "hidden",
      display: "flex",
      flexDirection: "column",
    },
    historyView: {
      height: "100%",
      overflow: "hidden",
    },
  };

  const composerContent = (
    <Spin spinning={loading || executing} style={{ height: "100%" }}>
      <div style={styles.centerArea}>
        <VerticalSplitPane
          defaultTopHeight="55%"
          minTopHeight={200}
          minBottomHeight={150}
          top={
            <div style={styles.requestPanel}>
              <RequestPanel />
            </div>
          }
          bottom={
            <div style={styles.responsePanel}>
              <ResponsePanel />
            </div>
          }
        />
      </div>
    </Spin>
  );

  const historyContent = (
    <div style={styles.historyView}>
      <HistoryView />
    </div>
  );

  return (
    <div style={styles.container}>
      <div style={styles.mainContent}>
        <SplitPane
          defaultLeftWidth="280px"
          minLeftWidth={200}
          minRightWidth={500}
          left={
            <div style={styles.collectionPanel}>
              <CollectionPanel />
            </div>
          }
          right={currentMode === "composer" ? composerContent : historyContent}
        />
      </div>
      <div style={styles.toolbar}>
        <ModeButton
          mode="composer"
          icon={<EditOutlined style={{ fontSize: 16 }} />}
          label="Composer"
          isActive={currentMode === "composer"}
          onClick={() => handleModeChange("composer")}
        />
        <ModeButton
          mode="history"
          icon={<HistoryOutlined style={{ fontSize: 16 }} />}
          label="History"
          isActive={currentMode === "history"}
          onClick={() => handleModeChange("history")}
        />
      </div>
    </div>
  );
}
