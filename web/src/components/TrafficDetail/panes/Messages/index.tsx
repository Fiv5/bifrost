import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import {
  Table,
  Typography,
  Tag,
  theme,
  Button,
  Space,
  Tooltip,
  Empty,
  ConfigProvider,
  Modal,
} from "antd";
import type { TableProps } from "antd";
import {
  ArrowUpOutlined,
  ArrowDownOutlined,
  ReloadOutlined,
  CopyOutlined,
  ExpandOutlined,
  FullscreenOutlined,
} from "@ant-design/icons";
import dayjs from "dayjs";
import hljs from "highlight.js/lib/core";
import json from "highlight.js/lib/languages/json";
import plaintext from "highlight.js/lib/languages/plaintext";
import "../../../../styles/hljs-github-theme.css";
import type {
  WebSocketFrame,
  FrameDirection,
  FrameType,
  SSEEvent,
  SessionTargetSearchState,
} from "../../../../types";
import { apiFetch } from "../../../../api/apiFetch";
import { getClientId } from "../../../../services/clientId";
import { SseMessageList } from "./SseMessageList";
import {
  FullscreenMessageViewer,
  normalizeWSFrame,
  type MessageItem,
} from "../../../VirtualMessageViewer";
import { useTrafficStore } from "../../../../stores/useTrafficStore";

hljs.registerLanguage("json", json);
hljs.registerLanguage("plaintext", plaintext);

const { Text } = Typography;

interface MessagesProps {
  recordId: string;
  isWebSocket: boolean;
  frameCount: number;
  isConnectionOpen?: boolean;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  onSseCountChange?: (count: number) => void;
}

const formatSize = (bytes: number) => {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
};

const getFrameTypeColor = (type: FrameType): string => {
  switch (type) {
    case "text":
      return "blue";
    case "binary":
      return "purple";
    case "ping":
      return "cyan";
    case "pong":
      return "geekblue";
    case "close":
      return "red";
    case "sse":
      return "green";
    default:
      return "default";
  }
};

const DirectionIcon = ({ direction }: { direction: FrameDirection }) => {
  return direction === "send" ? (
    <ArrowUpOutlined style={{ color: "#52c41a" }} />
  ) : (
    <ArrowDownOutlined style={{ color: "#1890ff" }} />
  );
};

const formatJson = (text: string): { formatted: string; isJson: boolean } => {
  try {
    const parsed = JSON.parse(text);
    return { formatted: JSON.stringify(parsed, null, 2), isJson: true };
  } catch {
    return { formatted: text, isJson: false };
  }
};

const highlightContent = (text: string): string => {
  const { formatted, isJson } = formatJson(text);
  try {
    const result = hljs.highlight(formatted, {
      language: isJson ? "json" : "plaintext",
    });
    return result.value;
  } catch {
    return formatted;
  }
};

const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

const parseSseChunkToEvent = (
  chunk: string,
  index: number,
  timestamp: number,
): SSEEvent | null => {
  const lines = chunk.split("\n");
  const dataLines: string[] = [];
  let eventId: string | undefined;
  let eventType: string | undefined;

  for (const rawLine of lines) {
    const line = rawLine.trimEnd();
    if (!line) continue;
    if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).replace(/^ /, ""));
      continue;
    }
    if (line.startsWith("event:")) {
      const v = line.slice(6).replace(/^ /, "");
      if (v) eventType = v;
      continue;
    }
    if (line.startsWith("id:")) {
      const v = line.slice(3).replace(/^ /, "");
      if (v) eventId = v;
    }
  }

  const data = dataLines.length > 0 ? dataLines.join("\n") : chunk;
  if (!data && !eventId && !eventType) return null;
  return {
    id: eventId ?? String(index + 1),
    event: eventType ?? "message",
    data,
    timestamp,
  };
};

export const Messages = ({
  recordId,
  isWebSocket,
  frameCount,
  isConnectionOpen = false,
  searchValue,
  onSseCountChange,
}: MessagesProps) => {
  const { token } = theme.useToken();
  const [frames, setFrames] = useState<WebSocketFrame[]>([]);
  const [loading, setLoading] = useState(false);
  const [lastFrameId, setLastFrameId] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const tableRef = useRef<HTMLDivElement>(null);
  const [selectedFrame, setSelectedFrame] = useState<WebSocketFrame | null>(
    null,
  );
  const [detailModalOpen, setDetailModalOpen] = useState(false);
  const [wsDetailLoading, setWsDetailLoading] = useState(false);
  const eventSourceRef = useRef<EventSource | null>(null);
  const sseEventSourceRef = useRef<EventSource | null>(null);
  const [sseEvents, setSseEvents] = useState<SSEEvent[]>([]);
  const lastSseSeqRef = useRef<number>(0);
  const [sseReloadToken, setSseReloadToken] = useState(0);
  const [sseConnectionState, setSseConnectionState] = useState<
    "idle" | "connecting" | "open" | "closed" | "error"
  >("idle");
  const [sseLoading, setSseLoading] = useState(false);
  const sseParseTokenRef = useRef(0);
  const ssePendingRef = useRef<SSEEvent[]>([]);
  const sseFlushRef = useRef<number | null>(null);
  const [wsPayloadById, setWsPayloadById] = useState<Record<number, string>>(
    {},
  );
  const inflightWsPayloadIdsRef = useRef<Set<number>>(new Set());
  const responseBody = useTrafficStore((state) => state.responseBody);
  const setResponseBody = useTrafficStore((state) => state.setResponseBody);
  const appendSseResponseBody = useTrafficStore(
    (state) => state.appendSseResponseBody,
  );

  const fetchFrames = useCallback(
    async (after?: number) => {
      setLoading(true);
      try {
        const params = new URLSearchParams();
        if (after !== undefined) {
          params.set("after", String(after));
        }
        params.set("limit", "100");

        const response = await apiFetch(
          `/_bifrost/api/traffic/${recordId}/frames?${params.toString()}`,
        );
        if (!response.ok) {
          throw new Error("Failed to fetch frames");
        }
        const data = await response.json();

        if (after !== undefined) {
          setFrames((prev) => [...prev, ...data.frames]);
        } else {
          setFrames(data.frames);
        }
        setLastFrameId(data.last_frame_id);
        setHasMore(data.has_more);
      } catch (error) {
        console.error("Failed to fetch frames:", error);
      } finally {
        setLoading(false);
      }
    },
    [recordId],
  );

  const fetchFramePayload = useCallback(
    async (frameId: number) => {
      const response = await apiFetch(
        `/_bifrost/api/traffic/${recordId}/frames/${frameId}`,
      );
      if (!response.ok) {
        return "";
      }
      const data = await response.json();
      return data.full_payload || "";
    },
    [recordId],
  );

  useEffect(() => {
    if (isWebSocket && frameCount > 0) {
      fetchFrames();
    }
  }, [fetchFrames, frameCount, isWebSocket]);

  useEffect(() => {
    setFrames([]);
    setWsPayloadById({});
    inflightWsPayloadIdsRef.current.clear();
    setSseEvents([]);
    lastSseSeqRef.current = 0;
    sseParseTokenRef.current += 1;
    if (sseFlushRef.current !== null) {
      cancelAnimationFrame(sseFlushRef.current);
      sseFlushRef.current = null;
    }
    ssePendingRef.current = [];
    setSseConnectionState("idle");
    setSseLoading(false);
  }, [recordId]);

  useEffect(() => {
    return () => {
      if (sseFlushRef.current !== null) {
        cancelAnimationFrame(sseFlushRef.current);
        sseFlushRef.current = null;
      }
      sseParseTokenRef.current += 1;
    };
  }, []);

  useEffect(() => {
    if (!isWebSocket || !isConnectionOpen) {
      return;
    }

    const eventSource = new EventSource(
      `/_bifrost/api/traffic/${recordId}/frames/stream?x_client_id=${encodeURIComponent(getClientId())}`,
    );
    eventSourceRef.current = eventSource;

    eventSource.onmessage = (event) => {
      try {
        const frame = JSON.parse(event.data) as WebSocketFrame;
        setFrames((prev) => {
          if (prev.some((f) => f.frame_id === frame.frame_id)) {
            return prev;
          }
          return [...prev, frame];
        });
        setLastFrameId(frame.frame_id);
      } catch (error) {
        console.error("Failed to parse frame event:", error);
      }
    };

    eventSource.onerror = () => {
      eventSource.close();
      eventSourceRef.current = null;
    };

    return () => {
      eventSource.close();
      eventSourceRef.current = null;
      apiFetch(`/_bifrost/api/traffic/${recordId}/frames/unsubscribe`, {
        method: "DELETE",
      }).catch(() => {});
    };
  }, [isConnectionOpen, isWebSocket, recordId]);

  useEffect(() => {
    if (isWebSocket || !isConnectionOpen) {
      return;
    }
    const eventSource = new EventSource(
      `/_bifrost/api/traffic/${recordId}/sse/stream?from=begin&x_client_id=${encodeURIComponent(getClientId())}`,
    );
    sseEventSourceRef.current = eventSource;
    setSseConnectionState("connecting");
    setSseLoading(true);
    setResponseBody(recordId, "");
    lastSseSeqRef.current = 0;
    setSseEvents([]);

    const flushPending = () => {
      const batch = ssePendingRef.current;
      ssePendingRef.current = [];
      sseFlushRef.current = null;
      if (batch.length > 0) {
        setSseEvents((prev) => prev.concat(batch));
      }
    };

    const enqueueEvent = (ev: SSEEvent) => {
      ssePendingRef.current.push(ev);
      if (sseFlushRef.current === null) {
        sseFlushRef.current = requestAnimationFrame(flushPending);
      }
    };

    eventSource.onopen = () => {
      setSseConnectionState("open");
      setSseLoading(false);
    };

    eventSource.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data) as {
          seq?: number;
          ts?: number;
          id?: string;
          event?: string;
          data?: string;
          raw?: string | null;
        };
        const seq = payload.seq ?? 0;
        if (seq > 0 && seq <= lastSseSeqRef.current) {
          return;
        }
        if (seq > 0) {
          lastSseSeqRef.current = seq;
        }
        const ts = payload.ts ?? Date.now();
        const ev: SSEEvent = {
          id: payload.id ?? String(seq || ts),
          event: payload.event ?? "message",
          data: payload.data ?? "",
          timestamp: ts,
        };
        enqueueEvent(ev);
        if (payload.raw) {
          const raw = payload.raw.replace(/\n+$/, "");
          if (raw.length > 0) {
            appendSseResponseBody(recordId, raw);
          }
        }
      } catch (e) {
        console.error("Failed to parse SSE event:", e);
      }
    };

    eventSource.onerror = () => {
      eventSource.close();
      sseEventSourceRef.current = null;
      setSseConnectionState("error");
      setSseLoading(false);
    };

    return () => {
      eventSource.close();
      sseEventSourceRef.current = null;
      setSseConnectionState("closed");
      setSseLoading(false);
    };
  }, [
    appendSseResponseBody,
    isConnectionOpen,
    isWebSocket,
    recordId,
    setResponseBody,
    sseReloadToken,
  ]);

  useEffect(() => {
    if (isWebSocket || isConnectionOpen) {
      return;
    }
    if (responseBody === null) {
      return;
    }
    const token = ++sseParseTokenRef.current;
    setSseConnectionState("closed");
    setSseLoading(true);
    setSseEvents([]);
    const normalized = responseBody.replace(/\r\n/g, "\n");
    let index = 0;
    let eventIndex = 0;
    const batchSize = 200;

    const run = () => {
      if (sseParseTokenRef.current !== token) return;
      const batch: SSEEvent[] = [];
      let processed = 0;
      while (processed < batchSize && index < normalized.length) {
        const next = normalized.indexOf("\n\n", index);
        if (next === -1) {
          index = normalized.length;
          break;
        }
        const chunk = normalized.slice(index, next).replace(/\n+$/, "");
        index = next + 2;
        if (chunk.trim().length > 0) {
          const ev = parseSseChunkToEvent(chunk, eventIndex, Date.now());
          if (ev) {
            batch.push(ev);
            eventIndex += 1;
          }
        }
        processed += 1;
      }
      if (batch.length > 0) {
        setSseEvents((prev) => prev.concat(batch));
      }
      if (index < normalized.length) {
        setTimeout(run, 0);
      } else {
        setSseLoading(false);
      }
    };

    run();
    return () => {
      if (sseParseTokenRef.current === token) {
        sseParseTokenRef.current += 1;
      }
    };
  }, [isConnectionOpen, isWebSocket, responseBody]);

  useEffect(() => {
    onSseCountChange?.(sseEvents.length);
  }, [onSseCountChange, sseEvents.length]);

  const [sseSearchQuery, setSseSearchQuery] = useState("");
  const [sseSearchMode, setSseSearchMode] = useState<"highlight" | "filter">(
    "highlight",
  );
  const [wsFullscreenOpen, setWsFullscreenOpen] = useState(false);
  const [sseFullscreenOpen, setSseFullscreenOpen] = useState(false);

  const framesForWsDisplay = useMemo<WebSocketFrame[]>(() => {
    if (!isWebSocket) {
      return frames;
    }
    return frames.map((f) => {
      if (f.payload_preview) return f;
      const payload = wsPayloadById[f.frame_id];
      if (!payload) return f;
      return { ...f, payload_preview: payload };
    });
  }, [frames, isWebSocket, wsPayloadById]);

  const filteredWsFrames = useMemo(() => {
    if (!isWebSocket) return [];
    if (!searchValue.value) return framesForWsDisplay;
    const searchLower = searchValue.value.toLowerCase();
    return framesForWsDisplay.filter(
      (frame) =>
        frame.payload_preview?.toLowerCase().includes(searchLower) ||
        frame.frame_type.toLowerCase().includes(searchLower),
    );
  }, [framesForWsDisplay, isWebSocket, searchValue.value]);

  const normalizedWsMessages = useMemo<MessageItem[]>(() => {
    return framesForWsDisplay.map(normalizeWSFrame);
  }, [framesForWsDisplay]);

  const openWsFrameDetail = useCallback(
    async (frame: WebSocketFrame) => {
      setSelectedFrame(frame);
      setDetailModalOpen(true);
      if (frame.payload_size === 0) {
        return;
      }
      if (
        frame.payload_preview &&
        (frame.frame_type === "text" ||
          frame.frame_type === "close" ||
          frame.frame_type === "sse") &&
        frame.payload_preview.length >= frame.payload_size
      ) {
        return;
      }
      if (wsPayloadById[frame.frame_id]) {
        return;
      }
      if (inflightWsPayloadIdsRef.current.has(frame.frame_id)) {
        return;
      }
      inflightWsPayloadIdsRef.current.add(frame.frame_id);
      setWsDetailLoading(true);
      try {
        const payload = await fetchFramePayload(frame.frame_id);
        if (payload) {
          setWsPayloadById((prev) =>
            prev[frame.frame_id]
              ? prev
              : { ...prev, [frame.frame_id]: payload },
          );
        }
      } finally {
        inflightWsPayloadIdsRef.current.delete(frame.frame_id);
        setWsDetailLoading(false);
      }
    },
    [fetchFramePayload, wsPayloadById],
  );

  useEffect(() => {
    if (!isWebSocket) {
      return;
    }
    const previewLimitGuess = 256;
    const missingIds = frames
      .filter(
        (f) =>
          f.payload_size > 0 &&
          !wsPayloadById[f.frame_id] &&
          (!f.payload_preview || f.payload_size > previewLimitGuess),
      )
      .map((f) => f.frame_id)
      .filter((id) => !inflightWsPayloadIdsRef.current.has(id));
    if (missingIds.length === 0) {
      return;
    }
    let cancelled = false;
    const run = async () => {
      const concurrency = 6;
      let idx = 0;
      const worker = async () => {
        while (!cancelled) {
          const current = missingIds[idx++];
          if (current === undefined) return;
          inflightWsPayloadIdsRef.current.add(current);
          const payload = await fetchFramePayload(current);
          inflightWsPayloadIdsRef.current.delete(current);
          if (cancelled || !payload) continue;
          setWsPayloadById((prev) =>
            prev[current] ? prev : { ...prev, [current]: payload },
          );
        }
      };
      await Promise.all(
        Array.from({ length: Math.min(concurrency, missingIds.length) }, () =>
          worker(),
        ),
      );
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [fetchFramePayload, frames, isWebSocket, wsPayloadById]);

  const selectedPayload = useMemo(() => {
    if (!selectedFrame) return "";
    return (
      wsPayloadById[selectedFrame.frame_id] ||
      selectedFrame.payload_preview ||
      ""
    );
  }, [selectedFrame, wsPayloadById]);

  const columns: TableProps<WebSocketFrame>["columns"] = [
    {
      title: "#",
      dataIndex: "frame_id",
      key: "frame_id",
      width: 60,
      render: (id: number) => <Text type="secondary">{id}</Text>,
    },
    {
      title: "",
      dataIndex: "direction",
      key: "direction",
      width: 40,
      render: (direction: FrameDirection) => (
        <DirectionIcon direction={direction} />
      ),
    },
    {
      title: "Type",
      dataIndex: "frame_type",
      key: "frame_type",
      width: 80,
      render: (type: FrameType) => (
        <Tag color={getFrameTypeColor(type)}>{type.toUpperCase()}</Tag>
      ),
    },
    {
      title: "Size",
      dataIndex: "payload_size",
      key: "payload_size",
      width: 80,
      render: (size: number) => formatSize(size),
    },
    {
      title: "Time",
      dataIndex: "timestamp",
      key: "timestamp",
      width: 100,
      render: (ts: number) => dayjs(ts).format("HH:mm:ss.SSS"),
    },
    {
      title: "Preview",
      dataIndex: "payload_preview",
      key: "payload_preview",
      ellipsis: true,
      render: (preview: string | undefined) =>
        preview ? (
          <Text
            style={{ fontFamily: "monospace", fontSize: 12 }}
            ellipsis={{ tooltip: preview }}
          >
            {preview}
          </Text>
        ) : (
          <Text type="secondary">-</Text>
        ),
    },
    {
      title: "",
      key: "actions",
      width: 70,
      render: (_: unknown, record: WebSocketFrame) => (
        <Space size={4}>
          <Tooltip title="Copy">
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={async (e) => {
                e.stopPropagation();
                const payload =
                  wsPayloadById[record.frame_id] ||
                  record.payload_preview ||
                  "";
                if (payload) {
                  await copyToClipboard(payload);
                  return;
                }
                if (record.payload_size === 0) return;
                if (inflightWsPayloadIdsRef.current.has(record.frame_id))
                  return;
                inflightWsPayloadIdsRef.current.add(record.frame_id);
                const full = await fetchFramePayload(record.frame_id);
                inflightWsPayloadIdsRef.current.delete(record.frame_id);
                if (!full) return;
                setWsPayloadById((prev) =>
                  prev[record.frame_id]
                    ? prev
                    : { ...prev, [record.frame_id]: full },
                );
                await copyToClipboard(full);
              }}
              disabled={record.payload_size === 0}
            />
          </Tooltip>
          <Tooltip title="Expand">
            <Button
              type="text"
              size="small"
              icon={<ExpandOutlined />}
              onClick={(e) => {
                e.stopPropagation();
                void openWsFrameDetail(record);
              }}
              disabled={record.payload_size === 0}
            />
          </Tooltip>
        </Space>
      ),
    },
  ];

  if (
    frameCount === 0 &&
    frames.length === 0 &&
    sseEvents.length === 0 &&
    !isConnectionOpen
  ) {
    return (
      <div
        style={{
          padding: 24,
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          height: 200,
        }}
      >
        <Empty
          description={`No ${isWebSocket ? "WebSocket" : "SSE"} messages yet`}
        />
      </div>
    );
  }

  if (!isWebSocket) {
    return (
      <div
        data-testid="sse-message-list"
        style={{ height: "100%", overflow: "hidden" }}
      >
        <SseMessageList
          events={sseEvents}
          loading={sseLoading}
          hasMore={false}
          searchQuery={sseSearchQuery}
          searchMode={sseSearchMode}
          onSearchChange={setSseSearchQuery}
          onSearchModeChange={setSseSearchMode}
          onLoadMore={() => {}}
          onRefresh={() => setSseReloadToken((n) => n + 1)}
          onFullscreenOpen={() => setSseFullscreenOpen(true)}
          connectionState={sseConnectionState}
        />
        <Modal
          open={sseFullscreenOpen}
          onCancel={() => setSseFullscreenOpen(false)}
          footer={null}
          width="80vw"
          styles={{
            body: {
              height: "70vh",
              overflow: "hidden",
            },
          }}
        >
          <SseMessageList
            events={sseEvents}
            loading={sseLoading}
            hasMore={false}
            searchQuery={sseSearchQuery}
            searchMode={sseSearchMode}
            onSearchChange={setSseSearchQuery}
            onSearchModeChange={setSseSearchMode}
            onLoadMore={() => {}}
            onRefresh={() => setSseReloadToken((n) => n + 1)}
            connectionState={sseConnectionState}
          />
        </Modal>
      </div>
    );
  }

  return (
    <div ref={tableRef} data-testid="ws-frames-pane">
      <div
        style={{
          marginBottom: 4,
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <Text type="secondary" data-testid="ws-frames-summary">
          {filteredWsFrames.length} of {frames.length} frames
          {hasMore && " (more available)"}
        </Text>
        <Space>
          <Tooltip title="Fullscreen">
            <Button
              size="small"
              icon={<FullscreenOutlined />}
              onClick={() => setWsFullscreenOpen(true)}
              disabled={frames.length === 0}
            />
          </Tooltip>
          {hasMore && (
            <Button
              size="small"
              onClick={() => fetchFrames(lastFrameId)}
              loading={loading}
            >
              Load More
            </Button>
          )}
          <Tooltip title="Refresh">
            <Button
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => fetchFrames()}
              loading={loading}
            />
          </Tooltip>
        </Space>
      </div>

      <ConfigProvider
        theme={{
          components: {
            Table: {
              cellPaddingBlockSM: 2,
              cellPaddingInlineSM: 4,
            },
          },
        }}
      >
        <Table<WebSocketFrame>
          dataSource={filteredWsFrames}
          columns={columns}
          rowKey="frame_id"
          pagination={false}
          size="small"
          loading={loading}
          data-testid="ws-frames-table"
          style={{
            backgroundColor: token.colorBgLayout,
            borderRadius: 4,
          }}
          rowClassName={(record) =>
            `${record.direction === "send" ? "frame-send" : "frame-receive"} message-row`
          }
          onRow={(record) => ({
            onClick: () => {
              if (record.payload_size === 0) return;
              void openWsFrameDetail(record);
            },
            "data-testid": "ws-frame-row",
            "data-frame-id": record.frame_id,
            "data-payload-size": record.payload_size,
          })}
        />
      </ConfigProvider>

      <style>{`
        .frame-send td:first-child {
          border-left: 3px solid #52c41a;
        }
        .frame-receive td:first-child {
          border-left: 3px solid #1890ff;
        }
        .message-row {
          cursor: pointer;
        }
        .message-row:hover {
          background-color: ${token.colorBgTextHover};
        }
      `}</style>

      <Modal
        title={
          <Space>
            <DirectionIcon direction={selectedFrame?.direction ?? "receive"} />
            <Tag color={getFrameTypeColor(selectedFrame?.frame_type ?? "text")}>
              {selectedFrame?.frame_type?.toUpperCase()}
            </Tag>
            <Text type="secondary">
              #{selectedFrame?.frame_id} -{" "}
              {dayjs(selectedFrame?.timestamp).format(
                "YYYY-MM-DD HH:mm:ss.SSS",
              )}
            </Text>
          </Space>
        }
        open={detailModalOpen}
        onCancel={() => {
          setDetailModalOpen(false);
          setSelectedFrame(null);
          setWsDetailLoading(false);
        }}
        footer={
          <Space>
            <Button
              icon={<CopyOutlined />}
              onClick={() => {
                if (selectedPayload) {
                  const { formatted } = formatJson(selectedPayload);
                  copyToClipboard(formatted);
                }
              }}
              disabled={!selectedPayload || wsDetailLoading}
            >
              Copy
            </Button>
            <Button onClick={() => setDetailModalOpen(false)}>Close</Button>
          </Space>
        }
        width={700}
        styles={{
          body: {
            maxHeight: "60vh",
            overflow: "auto",
          },
        }}
      >
        {wsDetailLoading && <Text type="secondary">Loading...</Text>}
        {!!selectedPayload && (
          <pre
            style={{
              margin: 0,
              padding: 12,
              fontSize: 12,
              fontFamily: "monospace",
              backgroundColor: token.colorBgLayout,
              borderRadius: 4,
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
              lineHeight: 1.5,
            }}
          >
            <code
              className="hljs"
              dangerouslySetInnerHTML={{
                __html: highlightContent(selectedPayload),
              }}
            />
          </pre>
        )}
      </Modal>

      <FullscreenMessageViewer
        open={wsFullscreenOpen}
        onClose={() => setWsFullscreenOpen(false)}
        items={normalizedWsMessages}
        title={
          <Space>
            <Tag color="purple">WebSocket</Tag>
            <Text type="secondary">{frames.length} frames</Text>
          </Space>
        }
        initialQuery={searchValue.value}
        initialMatchMode="highlight"
      />
    </div>
  );
};
