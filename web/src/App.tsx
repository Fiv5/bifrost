import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ConfigProvider, theme } from "antd";
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

export default function App() {
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);

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
