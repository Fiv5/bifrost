import { useEffect } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ConfigProvider, theme } from "antd";
import AppLayout from "./components/Layout";
import Rules from "./pages/Rules";
import Traffic from "./pages/Traffic";
import Settings from "./pages/Settings";
import Values from "./pages/Values";
import Whitelist from "./pages/Whitelist";
import { useThemeStore, initThemeListener } from "./stores/useThemeStore";

export default function App() {
  const resolvedTheme = useThemeStore((state) => state.resolvedTheme);

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
        <Routes>
          <Route path="/" element={<AppLayout />}>
            <Route index element={<Navigate to="/traffic" replace />} />
            <Route path="traffic" element={<Traffic />} />
            <Route path="rules" element={<Rules />} />
            <Route path="values" element={<Values />} />
            <Route path="whitelist" element={<Whitelist />} />
            <Route path="settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </ConfigProvider>
  );
}
