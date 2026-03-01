import { useCallback, useRef, useEffect, type CSSProperties } from "react";
import { Input, Button, Empty, Tag, Tooltip, theme, Typography } from "antd";
import {
  SendOutlined,
  DeleteOutlined,
  DisconnectOutlined,
  ClockCircleOutlined,
  ArrowUpOutlined,
  ArrowDownOutlined,
} from "@ant-design/icons";
import { useReplayStore } from "../../../stores/useReplayStore";
import type { SSEEvent, WebSocketMessage } from "../../../types";

const { Text } = Typography;

function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString("en-US", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    fractionalSecondDigits: 3,
  } as Intl.DateTimeFormatOptions);
}

function formatDuration(startedAt: number, endedAt?: number): string {
  const duration = (endedAt || Date.now()) - startedAt;
  if (duration < 1000) return `${duration}ms`;
  if (duration < 60000) return `${(duration / 1000).toFixed(1)}s`;
  return `${Math.floor(duration / 60000)}m ${Math.floor((duration % 60000) / 1000)}s`;
}

interface SSEEventItemProps {
  event: SSEEvent;
}

function SSEEventItem({ event }: SSEEventItemProps) {
  const { token } = theme.useToken();

  return (
    <div
      style={{
        padding: "8px 12px",
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        fontSize: 12,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 4,
        }}
      >
        <ArrowDownOutlined style={{ color: token.colorSuccess, fontSize: 10 }} />
        <Text type="secondary" style={{ fontSize: 10 }}>
          {formatTime(event.timestamp)}
        </Text>
        {event.id && (
          <Tag color="blue" style={{ fontSize: 10, margin: 0 }}>
            id: {event.id}
          </Tag>
        )}
        {event.event && (
          <Tag color="purple" style={{ fontSize: 10, margin: 0 }}>
            {event.event}
          </Tag>
        )}
      </div>
      <div
        style={{
          fontFamily: "monospace",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          backgroundColor: token.colorBgLayout,
          padding: "4px 8px",
          borderRadius: 4,
          maxHeight: 200,
          overflow: "auto",
        }}
      >
        {event.data}
      </div>
    </div>
  );
}

interface WebSocketMessageItemProps {
  message: WebSocketMessage;
}

function WebSocketMessageItem({ message }: WebSocketMessageItemProps) {
  const { token } = theme.useToken();
  const isSend = message.direction === "send";

  return (
    <div
      style={{
        padding: "8px 12px",
        borderBottom: `1px solid ${token.colorBorderSecondary}`,
        fontSize: 12,
        backgroundColor: isSend ? token.colorBgLayout : "transparent",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 4,
        }}
      >
        {isSend ? (
          <ArrowUpOutlined style={{ color: token.colorPrimary, fontSize: 10 }} />
        ) : (
          <ArrowDownOutlined style={{ color: token.colorSuccess, fontSize: 10 }} />
        )}
        <Tag
          color={isSend ? "blue" : "green"}
          style={{ fontSize: 10, margin: 0 }}
        >
          {isSend ? "SENT" : "RECEIVED"}
        </Tag>
        <Text type="secondary" style={{ fontSize: 10 }}>
          {formatTime(message.timestamp)}
        </Text>
        <Tag style={{ fontSize: 10, margin: 0 }}>{message.type}</Tag>
      </div>
      <div
        style={{
          fontFamily: "monospace",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          padding: "4px 8px",
          borderRadius: 4,
          border: `1px solid ${token.colorBorderSecondary}`,
          maxHeight: 200,
          overflow: "auto",
        }}
      >
        {message.data}
      </div>
    </div>
  );
}

export default function MessagesPanel() {
  const { token } = theme.useToken();
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const {
    streamingConnection,
    sseEvents,
    wsMessages,
    uiState,
    disconnectSSE,
    disconnectWebSocket,
    sendWebSocketMessage,
    clearStreamingMessages,
    updateUIState,
  } = useReplayStore();

  const isSSE = streamingConnection?.type === "sse";
  const isWebSocket = streamingConnection?.type === "websocket";
  const isConnected = streamingConnection?.status === "connected";
  const messageInput = uiState.wsMessageInput;

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [sseEvents.length, wsMessages.length]);

  const handleDisconnect = useCallback(() => {
    if (isSSE) {
      disconnectSSE();
    } else if (isWebSocket) {
      disconnectWebSocket();
    }
  }, [isSSE, isWebSocket, disconnectSSE, disconnectWebSocket]);

  const handleSendMessage = useCallback(() => {
    if (messageInput.trim()) {
      sendWebSocketMessage(messageInput);
      updateUIState({ wsMessageInput: "" });
    }
  }, [messageInput, sendWebSocketMessage, updateUIState]);

  const handleKeyPress = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSendMessage();
      }
    },
    [handleSendMessage]
  );

  const styles: Record<string, CSSProperties> = {
    container: {
      height: "100%",
      display: "flex",
      flexDirection: "column",
      overflow: "hidden",
    },
    header: {
      padding: "8px 12px",
      borderBottom: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      gap: 8,
      flexShrink: 0,
    },
    connectionInfo: {
      display: "flex",
      alignItems: "center",
      gap: 8,
      flex: 1,
      minWidth: 0,
    },
    messageList: {
      flex: 1,
      overflow: "auto",
    },
    inputArea: {
      padding: "8px 12px",
      borderTop: `1px solid ${token.colorBorderSecondary}`,
      display: "flex",
      gap: 8,
      flexShrink: 0,
    },
  };

  if (!streamingConnection) {
    return (
      <div style={styles.container}>
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="No active connection"
          style={{ margin: "auto" }}
        />
      </div>
    );
  }

  const statusColors: Record<string, string> = {
    connecting: "processing",
    connected: "success",
    disconnected: "default",
    error: "error",
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <div style={styles.connectionInfo}>
          <Tag color={isSSE ? "orange" : "purple"}>
            {isSSE ? "SSE" : "WebSocket"}
          </Tag>
          <Tag color={statusColors[streamingConnection.status]}>
            {streamingConnection.status}
          </Tag>
          <Tooltip title={streamingConnection.url}>
            <Text
              ellipsis
              style={{ fontSize: 11, maxWidth: 200 }}
              type="secondary"
            >
              {streamingConnection.url}
            </Text>
          </Tooltip>
          <Text type="secondary" style={{ fontSize: 11 }}>
            <ClockCircleOutlined style={{ marginRight: 4 }} />
            {formatDuration(
              streamingConnection.startedAt,
              streamingConnection.endedAt
            )}
          </Text>
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          <Tooltip title="Clear messages">
            <Button
              size="small"
              icon={<DeleteOutlined />}
              onClick={clearStreamingMessages}
            />
          </Tooltip>
          {isConnected && (
            <Tooltip title="Disconnect">
              <Button
                size="small"
                danger
                icon={<DisconnectOutlined />}
                onClick={handleDisconnect}
              />
            </Tooltip>
          )}
        </div>
      </div>

      <div style={styles.messageList}>
        {isSSE &&
          sseEvents.map((event, index) => (
            <SSEEventItem key={`${event.timestamp}-${index}`} event={event} />
          ))}
        {isWebSocket &&
          wsMessages.map((msg) => (
            <WebSocketMessageItem key={msg.id} message={msg} />
          ))}
        {((isSSE && sseEvents.length === 0) ||
          (isWebSocket && wsMessages.length === 0)) && (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={
              isConnected ? "Waiting for messages..." : "No messages"
            }
            style={{ marginTop: 40 }}
          />
        )}
        <div ref={messagesEndRef} />
      </div>

      {isWebSocket && isConnected && (
        <div style={styles.inputArea}>
          <Input.TextArea
            placeholder="Type a message..."
            autoSize={{ minRows: 1, maxRows: 4 }}
            value={messageInput}
            onChange={(e) => updateUIState({ wsMessageInput: e.target.value })}
            onKeyPress={handleKeyPress}
            style={{ flex: 1 }}
          />
          <Button
            type="primary"
            icon={<SendOutlined />}
            onClick={handleSendMessage}
            disabled={!messageInput.trim()}
          >
            Send
          </Button>
        </div>
      )}

      {streamingConnection.error && (
        <div
          style={{
            padding: "8px 12px",
            backgroundColor: token.colorErrorBg,
            color: token.colorError,
            fontSize: 12,
          }}
        >
          Error: {streamingConnection.error}
        </div>
      )}
    </div>
  );
}
