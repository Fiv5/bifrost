import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ConfigProvider, Modal, theme, Typography } from "antd";
import AppLayout from "./components/Layout";
import BifrostFileDropZone from "./components/BifrostFileDropZone";
import Rules from "./pages/Rules";
import Traffic from "./pages/Traffic";
import Replay from "./pages/Replay";
import Settings from "./pages/Settings";
import Values from "./pages/Values";
import Scripts from "./pages/Scripts";
import { useThemeStore, initThemeListener } from "./stores/useThemeStore";
import { useGlobalDataSync } from "./hooks/useGlobalDataSync";
import { useEditorCompletion } from "./hooks/useEditorCompletion";
import { useForceRefreshStore } from "./stores/useForceRefreshStore";

export default function App() {
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);
  const forceRefreshVisible = useForceRefreshStore((s) => s.visible);
  const forceRefreshReason = useForceRefreshStore((s) => s.reason);

  useGlobalDataSync();
  useEditorCompletion();

  useEffect(() => {
    const cleanup = initThemeListener();
    return cleanup;
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolvedTheme);
  }, [resolvedTheme]);

  return (
    <ConfigProvider
      theme={{
        algorithm:
          resolvedTheme === "dark"
            ? theme.darkAlgorithm
            : theme.defaultAlgorithm,
        token: {
          colorPrimary: "#1677ff",
          borderRadius: 6,
        },
      }}
    >
      <Modal
        open={forceRefreshVisible}
        title="页面已被断开"
        closable={false}
        maskClosable={false}
        keyboard={false}
        okText="刷新页面"
        cancelButtonProps={{ style: { display: "none" } }}
        onOk={() => {
          window.location.reload();
        }}
      >
        <Typography.Paragraph>
          由于打开页面过多，当前页面的连接已被服务端关闭。
        </Typography.Paragraph>
        {forceRefreshReason ? (
          <Typography.Paragraph type="secondary">
            原因：{forceRefreshReason}
          </Typography.Paragraph>
        ) : null}
        <Typography.Paragraph type="secondary">
          请刷新页面后继续使用。
        </Typography.Paragraph>
      </Modal>
      <BrowserRouter basename="/_bifrost">
        <BifrostFileDropZone>
          <Routes>
            <Route path="/" element={<AppLayout />}>
              <Route index element={<Navigate to="/traffic" replace />} />
              <Route path="traffic" element={<Traffic />} />
              <Route path="replay" element={<Replay />} />
              <Route path="rules" element={<Rules />} />
              <Route path="values" element={<Values />} />
              <Route path="scripts" element={<Scripts />} />
              <Route path="settings" element={<Settings />} />
            </Route>
          </Routes>
        </BifrostFileDropZone>
      </BrowserRouter>
    </ConfigProvider>
  );
}
