import { useState, useEffect, type CSSProperties } from "react";
import { Avatar } from "antd";
import { AppstoreOutlined } from "@ant-design/icons";

interface AppIconProps {
  appName: string;
  size?: number;
  style?: CSSProperties;
}

const iconCache = new Map<string, string | null>();
const pendingRequests = new Map<string, Promise<string | null>>();

const fetchAppIcon = async (appName: string): Promise<string | null> => {
  try {
    const encodedName = encodeURIComponent(appName);
    const response = await fetch(`/_bifrost/api/app-icon/${encodedName}`);
    
    if (response.ok) {
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      return url;
    }
    return null;
  } catch {
    return null;
  }
};

export const AppIcon: React.FC<AppIconProps> = ({ appName, size = 16, style }) => {
  const [iconUrl, setIconUrl] = useState<string | null>(() => {
    return iconCache.get(appName) ?? null;
  });
  const [loading, setLoading] = useState(() => {
    return !iconCache.has(appName) && Boolean(appName);
  });
  const [error, setError] = useState(() => {
    const cached = iconCache.get(appName);
    return cached === null || !appName;
  });

  useEffect(() => {
    if (!appName) {
      setLoading(false);
      setError(true);
      return;
    }

    if (iconCache.has(appName)) {
      const cached = iconCache.get(appName);
      setIconUrl(cached ?? null);
      setLoading(false);
      setError(cached === null);
      return;
    }

    let cancelled = false;

    const loadIcon = async () => {
      let promise = pendingRequests.get(appName);
      
      if (!promise) {
        promise = fetchAppIcon(appName);
        pendingRequests.set(appName, promise);
      }

      try {
        const url = await promise;
        iconCache.set(appName, url);
        
        if (!cancelled) {
          setIconUrl(url);
          setError(url === null);
          setLoading(false);
        }
      } finally {
        pendingRequests.delete(appName);
      }
    };

    loadIcon();

    return () => {
      cancelled = true;
    };
  }, [appName]);

  if (loading || error || !iconUrl) {
    return (
      <Avatar
        size={size}
        icon={<AppstoreOutlined />}
        style={{
          backgroundColor: "#f0f0f0",
          color: "#999",
          fontSize: size * 0.6,
          ...style,
        }}
      />
    );
  }

  return (
    <Avatar
      size={size}
      src={iconUrl}
      style={{ backgroundColor: "transparent", ...style }}
    />
  );
};

export default AppIcon;
