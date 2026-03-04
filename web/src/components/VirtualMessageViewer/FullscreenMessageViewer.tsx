import { type ReactNode, useEffect, type CSSProperties, useMemo } from "react";
import {
  Modal,
  Input,
  Button,
  Space,
  Tooltip,
  Tag,
  Switch,
  theme,
  Typography,
} from "antd";
import {
  CloseOutlined,
  SearchOutlined,
  ArrowUpOutlined,
  ArrowDownOutlined,
  FilterOutlined,
  HighlightOutlined,
  VerticalAlignBottomOutlined,
} from "@ant-design/icons";
import type { MessageItem } from "./types";
import { VirtualMessageList } from "./VirtualMessageList";
import { useMessageSearch } from "./useMessageSearch";
import { MessageItemCard } from "./MessageItemCard";

const { Search } = Input;
const { Text } = Typography;

export interface FullscreenMessageViewerProps {
  open: boolean;
  onClose: () => void;
  items: MessageItem[];
  title?: ReactNode;
  initialQuery?: string;
  initialMatchMode?: "highlight" | "filter";
  followTail?: boolean;
  onFollowTailChange?: (following: boolean) => void;
}

export function FullscreenMessageViewer({
  open,
  onClose,
  items,
  title,
  initialQuery = "",
  initialMatchMode = "highlight",
  followTail = true,
  onFollowTailChange,
}: FullscreenMessageViewerProps) {
  const { token } = theme.useToken();

  const {
    searchState,
    setQuery,
    setMatchMode,
    filteredItems,
    highlightedIndices,
    goToNext,
    goToPrev,
    clearSearch,
    matchTokens,
  } = useMessageSearch({
    items,
    initialQuery,
    initialMatchMode,
  });

  useEffect(() => {
    if (!open) {
      clearSearch();
    }
  }, [open, clearSearch]);

  const handleMatchModeChange = (mode: "highlight" | "filter") => {
    setMatchMode(mode);
  };

  const matchInfo =
    searchState.total > 0
      ? `${searchState.currentIndex >= 0 ? searchState.currentIndex + 1 : 0}/${searchState.total}`
      : "0/0";

  const currentHighlightIndex = useMemo(() => {
    if (searchState.currentIndex < 0) return -1;
    if (searchState.matchMode === "filter") return searchState.currentIndex;
    return searchState.matchedIndices[searchState.currentIndex] ?? -1;
  }, [
    searchState.currentIndex,
    searchState.matchMode,
    searchState.matchedIndices,
  ]);

  const headerStyle: CSSProperties = {
    padding: "12px 16px",
    borderBottom: `1px solid ${token.colorBorderSecondary}`,
    display: "flex",
    alignItems: "center",
    gap: 12,
    flexWrap: "wrap",
  };

  const searchBarStyle: CSSProperties = {
    flex: "1 1 300px",
    minWidth: 200,
  };

  return (
    <Modal
      title={title || "Messages"}
      open={open}
      onCancel={onClose}
      footer={null}
      width="100vw"
      style={{ top: 0, margin: 0, paddingBottom: 0, maxWidth: "none" }}
      styles={{
        wrapper: {
          padding: 0,
        },
        container: {
          height: "100vh",
          display: "flex",
          flexDirection: "column",
        },
        body: {
          flex: 1,
          padding: 0,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        },
      }}
      closeIcon={<CloseOutlined />}
      destroyOnClose
    >
      <div style={headerStyle}>
        <Search
          placeholder="Search messages (space-separated tokens)..."
          allowClear
          value={searchState.query}
          onChange={(e) => setQuery(e.target.value)}
          onSearch={setQuery}
          style={searchBarStyle}
          prefix={<SearchOutlined />}
        />

        <Space size="small">
          <Tooltip title={`Current mode: ${searchState.matchMode}`}>
            <Button.Group>
              <Button
                size="small"
                type={
                  searchState.matchMode === "highlight" ? "primary" : "default"
                }
                icon={<HighlightOutlined />}
                onClick={() => handleMatchModeChange("highlight")}
              >
                Highlight
              </Button>
              <Button
                size="small"
                type={
                  searchState.matchMode === "filter" ? "primary" : "default"
                }
                icon={<FilterOutlined />}
                onClick={() => handleMatchModeChange("filter")}
              >
                Filter
              </Button>
            </Button.Group>
          </Tooltip>

          {searchState.total > 0 && (
            <>
              <Tag color="blue">{matchInfo} matched</Tag>
              <Button.Group size="small">
                <Button
                  icon={<ArrowUpOutlined />}
                  onClick={goToPrev}
                  disabled={searchState.total === 0}
                >
                  Prev
                </Button>
                <Button
                  icon={<ArrowDownOutlined />}
                  onClick={goToNext}
                  disabled={searchState.total === 0}
                >
                  Next
                </Button>
              </Button.Group>
            </>
          )}

          <Tooltip title="Follow latest messages">
            <Space size={4}>
              <VerticalAlignBottomOutlined style={{ fontSize: 12 }} />
              <Switch
                size="small"
                checked={followTail}
                onChange={onFollowTailChange}
              />
            </Space>
          </Tooltip>

          <Text type="secondary" style={{ fontSize: 12 }}>
            {searchState.matchMode === "filter"
              ? filteredItems.length
              : items.length}{" "}
            messages
          </Text>
        </Space>
      </div>

      <div style={{ flex: 1, overflow: "hidden" }}>
        <VirtualMessageList
          items={searchState.matchMode === "filter" ? filteredItems : items}
          renderItem={(item) => (
            <MessageItemCard
              message={item}
              searchTokens={
                searchState.matchMode === "highlight" ? matchTokens : []
              }
              caseSensitive={searchState.caseSensitive}
            />
          )}
          highlightedIndices={
            searchState.matchMode === "highlight" ? highlightedIndices : []
          }
          currentHighlightIndex={currentHighlightIndex}
          estimateSize={120}
          overscan={3}
          followTail={followTail}
          onFollowTailChange={onFollowTailChange}
          emptyContent={
            <div
              style={{
                padding: 40,
                textAlign: "center",
                color: token.colorTextSecondary,
              }}
            >
              {searchState.matchMode === "filter" && searchState.query
                ? "No messages match your search"
                : "No messages"}
            </div>
          }
          style={{ padding: "8px 16px" }}
        />
      </div>
    </Modal>
  );
}
