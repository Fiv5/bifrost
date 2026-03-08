import { useMemo, useState } from 'react';
import { Typography, Tag, Button, Space, Tooltip, theme } from 'antd';
import {
  CopyOutlined,
  ExpandOutlined,
  CompressOutlined,
  CheckOutlined,
} from '@ant-design/icons';
import dayjs from 'dayjs';
import hljs from 'highlight.js/lib/core';
import json from 'highlight.js/lib/languages/json';
import '../../../../styles/hljs-github-theme.css';
import type { SSEEvent } from '../../../../types';

hljs.registerLanguage('json', json);

const { Text } = Typography;

const parseJson = (text: string): { parsed: unknown; isJson: boolean } => {
  try {
    const parsed = JSON.parse(text);
    return { parsed, isJson: true };
  } catch {
    return { parsed: null, isJson: false };
  }
};

const highlightJson = (text: string): string => {
  try {
    const result = hljs.highlight(text, { language: 'json' });
    return result.value;
  } catch {
    return text;
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

interface SseEventCardProps {
  event: SSEEvent;
  searchValue?: string;
  expanded: boolean;
  onToggle: () => void;
}

export const SseEventCard = ({
  event,
  searchValue,
  expanded,
  onToggle,
}: SseEventCardProps) => {
  const { token } = theme.useToken();
  const [copied, setCopied] = useState(false);

  const eventData = useMemo(() => {
    return {
      id: event.id,
      event: event.event,
      data: event.data || '',
    };
  }, [event]);

  const { parsed, isJson } = useMemo(() => {
    return parseJson(eventData.data);
  }, [eventData.data]);

  const formattedContent = useMemo(() => {
    if (isJson && parsed !== null) {
      return JSON.stringify(parsed, null, 2);
    }
    return eventData.data;
  }, [isJson, parsed, eventData.data]);

  const highlightedContent = useMemo(() => {
    if (!isJson) return null;
    return highlightJson(formattedContent);
  }, [isJson, formattedContent]);

  const handleCopy = async () => {
    const success = await copyToClipboard(formattedContent);
    if (success) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  };

  const shouldTruncate = !expanded && formattedContent.split('\n').length > 8;
  const displayContent = shouldTruncate
    ? formattedContent.split('\n').slice(0, 8).join('\n') + '\n...'
    : formattedContent;

  const highlightSearch = (text: string, search?: string): React.ReactNode => {
    if (!search) return text;
    const lowerText = text.toLowerCase();
    const lowerSearch = search.toLowerCase();
    const index = lowerText.indexOf(lowerSearch);
    if (index === -1) return text;

    return (
      <>
        {text.slice(0, index)}
        <mark style={{ backgroundColor: '#ffe58f', padding: 0 }}>
          {text.slice(index, index + search.length)}
        </mark>
        {text.slice(index + search.length)}
      </>
    );
  };

  const eventName = eventData.event || 'message';

  return (
    <div
      style={{
        borderRadius: 6,
        border: `1px solid ${token.colorBorderSecondary}`,
        backgroundColor: token.colorBgContainer,
        overflow: 'hidden',
        marginBottom: 8,
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 10px',
          backgroundColor: token.colorFillQuaternary,
          borderBottom: `1px solid ${token.colorBorderSecondary}`,
        }}
      >
        <Space size={8} align="center">
          <Text type="secondary" style={{ fontSize: 11, fontFamily: 'monospace' }}>
            {dayjs(event.timestamp).format('HH:mm:ss.SSS')}
          </Text>
          <Tag
            color="green"
            style={{
              margin: 0,
              fontSize: 11,
              lineHeight: '18px',
              padding: '0 6px',
            }}
          >
            {eventName}
          </Tag>
          {eventData.id && (
            <Text type="secondary" style={{ fontSize: 11 }}>
              id: {eventData.id}
            </Text>
          )}
          {isJson && (
            <Tag
              color="blue"
              style={{
                margin: 0,
                fontSize: 10,
                lineHeight: '16px',
                padding: '0 4px',
              }}
            >
              JSON
            </Tag>
          )}
        </Space>
        <Space size={4}>
          <Tooltip title={copied ? 'Copied!' : 'Copy'}>
            <Button
              type="text"
              size="small"
              icon={copied ? <CheckOutlined style={{ color: token.colorSuccess }} /> : <CopyOutlined />}
              onClick={handleCopy}
              disabled={!eventData.data}
              style={{ width: 24, height: 24 }}
            />
          </Tooltip>
          {formattedContent.split('\n').length > 8 && (
            <Tooltip title={expanded ? 'Collapse' : 'Expand'}>
              <Button
                type="text"
                size="small"
                icon={expanded ? <CompressOutlined /> : <ExpandOutlined />}
                onClick={onToggle}
                style={{ width: 24, height: 24 }}
              />
            </Tooltip>
          )}
        </Space>
      </div>
      <div style={{ padding: '8px 10px' }}>
        {eventData.data ? (
          isJson && highlightedContent ? (
            <pre
              style={{
                margin: 0,
                fontSize: 12,
                fontFamily: 'monospace',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                lineHeight: 1.5,
                backgroundColor: token.colorFillQuaternary,
                padding: 8,
                borderRadius: 4,
              }}
            >
              <code
                className="hljs"
                dangerouslySetInnerHTML={{
                  __html: shouldTruncate
                    ? highlightJson(displayContent)
                    : highlightedContent,
                }}
              />
            </pre>
          ) : (
            <pre
              style={{
                margin: 0,
                fontSize: 12,
                fontFamily: 'monospace',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                lineHeight: 1.5,
                color: token.colorText,
              }}
            >
              {searchValue ? highlightSearch(displayContent, searchValue) : displayContent}
            </pre>
          )
        ) : (
          <Text type="secondary" style={{ fontSize: 12, fontStyle: 'italic' }}>
            (empty data)
          </Text>
        )}
      </div>
    </div>
  );
};
