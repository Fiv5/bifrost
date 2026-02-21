import { useState, useEffect } from "react";
import { Avatar } from "antd";
import { AppstoreOutlined } from "@ant-design/icons";

interface AppIconProps {
  appName: string;
  size?: number;
}

const iconCache = new Map<string, string | null>();

export const AppIcon: React.FC<AppIconProps> = ({ appName, size = 16 }) => {
  const [iconUrl, setIconUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    if (!appName) {
      setLoading(false);
      setError(true);
      return;
    }

    const cached = iconCache.get(appName);
    if (cached !== undefined) {
      setIconUrl(cached);
      setLoading(false);
      setError(cached === null);
      return;
    }

    const fetchIcon = async () => {
      try {
        const encodedName = encodeURIComponent(appName);
        const response = await fetch(`/_bifrost/api/app-icon/${encodedName}`);
        
        if (response.ok) {
          const blob = await response.blob();
          const url = URL.createObjectURL(blob);
          iconCache.set(appName, url);
          setIconUrl(url);
          setError(false);
        } else {
          iconCache.set(appName, null);
          setError(true);
        }
      } catch {
        iconCache.set(appName, null);
        setError(true);
      } finally {
        setLoading(false);
      }
    };

    fetchIcon();
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
        }}
      />
    );
  }

  return (
    <Avatar
      size={size}
      src={iconUrl}
      style={{ backgroundColor: "transparent" }}
    />
  );
};

export default AppIcon;
