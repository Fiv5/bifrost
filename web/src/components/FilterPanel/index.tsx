import { useMemo, type CSSProperties } from "react";
import { theme, Typography, Button, Empty, Tooltip } from "antd";
import { ClearOutlined } from "@ant-design/icons";
import { useFilterPanelStore } from "../../stores/useFilterPanelStore";
import FilterSection from "./FilterSection";
import PinnedFilters from "./PinnedFilters";
import FilterItem from "./FilterItem";
import AppIcon from "../AppIcon";

const { Text } = Typography;

interface FilterPanelProps {
  availableClientIps: string[];
  availableClientApps: string[];
  availableDomains: string[];
}

export default function FilterPanel({
  availableClientIps,
  availableClientApps,
  availableDomains,
}: FilterPanelProps) {
  const { token } = theme.useToken();
  const {
    pinnedFilters,
    selectedClientIps,
    selectedClientApps,
    selectedDomains,
    collapsedSections,
    toggleClientIp,
    toggleClientApp,
    toggleDomain,
    addPinnedFilter,
    setCollapsedSection,
    clearAllSelections,
  } = useFilterPanelStore();

  const hasSelections =
    selectedClientIps.length > 0 ||
    selectedClientApps.length > 0 ||
    selectedDomains.length > 0;

  const styles = useMemo<Record<string, CSSProperties>>(
    () => ({
      container: {
        display: "flex",
        flexDirection: "column",
        height: "100%",
        minHeight: 0,
        backgroundColor: token.colorBgContainer,
        borderRight: `1px solid ${token.colorBorderSecondary}`,
      },
      header: {
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "8px 12px",
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        backgroundColor: token.colorBgLayout,
        flexShrink: 0,
      },
      title: {
        fontSize: 13,
        fontWeight: 600,
        color: token.colorText,
        margin: 0,
      },
      content: {
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        overflowX: "hidden",
        padding: "4px 0",
      },
      emptyText: {
        color: token.colorTextSecondary,
        fontSize: 12,
        padding: "8px 12px",
      },
    }),
    [token]
  );

  const sortedClientIps = useMemo(() => {
    return [...availableClientIps].sort((a, b) => {
      if (a === "127.0.0.1") return -1;
      if (b === "127.0.0.1") return 1;
      if (a.startsWith("192.168.") && !b.startsWith("192.168.")) return -1;
      if (!a.startsWith("192.168.") && b.startsWith("192.168.")) return 1;
      return a.localeCompare(b);
    });
  }, [availableClientIps]);

  const sortedClientApps = useMemo(() => {
    return [...availableClientApps].sort((a, b) => a.localeCompare(b));
  }, [availableClientApps]);

  const sortedDomains = useMemo(() => {
    return [...availableDomains].sort((a, b) => a.localeCompare(b));
  }, [availableDomains]);

  const getIpLabel = (ip: string) => {
    if (ip === "127.0.0.1") return "Local (127.0.0.1)";
    return ip;
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <Text style={styles.title}>Filters</Text>
        {hasSelections && (
          <Tooltip title="Clear all selections">
            <Button
              type="text"
              size="small"
              icon={<ClearOutlined />}
              onClick={clearAllSelections}
            />
          </Tooltip>
        )}
      </div>
      <div style={styles.content}>
        {pinnedFilters.length > 0 && (
          <FilterSection
            title="Pinned"
            icon="📌"
            collapsed={collapsedSections.pinned}
            onToggle={() => setCollapsedSection("pinned", !collapsedSections.pinned)}
          >
            <PinnedFilters />
          </FilterSection>
        )}

        <FilterSection
          title="Client IP"
          collapsed={collapsedSections.clientIp}
          onToggle={() => setCollapsedSection("clientIp", !collapsedSections.clientIp)}
          count={sortedClientIps.length}
        >
          {sortedClientIps.length === 0 ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No clients"
              style={{ margin: "12px 0" }}
            />
          ) : (
            sortedClientIps.map((ip) => (
              <FilterItem
                key={ip}
                label={getIpLabel(ip)}
                value={ip}
                type="client_ip"
                selected={selectedClientIps.includes(ip)}
                onSelect={() => toggleClientIp(ip)}
                onPin={() =>
                  addPinnedFilter({
                    type: "client_ip",
                    value: ip,
                    label: getIpLabel(ip),
                  })
                }
              />
            ))
          )}
        </FilterSection>

        <FilterSection
          title="Applications"
          collapsed={collapsedSections.clientApp}
          onToggle={() => setCollapsedSection("clientApp", !collapsedSections.clientApp)}
          count={sortedClientApps.length}
        >
          {sortedClientApps.length === 0 ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No applications"
              style={{ margin: "12px 0" }}
            />
          ) : (
            sortedClientApps.map((app) => (
              <FilterItem
                key={app}
                label={app}
                value={app}
                type="client_app"
                selected={selectedClientApps.includes(app)}
                onSelect={() => toggleClientApp(app)}
                onPin={() =>
                  addPinnedFilter({
                    type: "client_app",
                    value: app,
                    label: app,
                  })
                }
                icon={<AppIcon appName={app} size={16} />}
              />
            ))
          )}
        </FilterSection>

        <FilterSection
          title="Domains"
          collapsed={collapsedSections.domain}
          onToggle={() => setCollapsedSection("domain", !collapsedSections.domain)}
          count={sortedDomains.length}
        >
          {sortedDomains.length === 0 ? (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description="No domains"
              style={{ margin: "12px 0" }}
            />
          ) : (
            sortedDomains.map((domain) => (
              <FilterItem
                key={domain}
                label={domain}
                value={domain}
                type="domain"
                selected={selectedDomains.includes(domain)}
                onSelect={() => toggleDomain(domain)}
                onPin={() =>
                  addPinnedFilter({
                    type: "domain",
                    value: domain,
                    label: domain,
                  })
                }
              />
            ))
          )}
        </FilterSection>
      </div>
    </div>
  );
}
