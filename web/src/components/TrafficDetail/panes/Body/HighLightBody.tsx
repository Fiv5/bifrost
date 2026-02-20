import { useMemo, useRef, useState, useCallback } from 'react';
import { theme, Button, Tooltip, message } from 'antd';
import { CopyOutlined, FormatPainterOutlined } from '@ant-design/icons';
import hljs from 'highlight.js/lib/core';
import json from 'highlight.js/lib/languages/json';
import xml from 'highlight.js/lib/languages/xml';
import javascript from 'highlight.js/lib/languages/javascript';
import css from 'highlight.js/lib/languages/css';
import plaintext from 'highlight.js/lib/languages/plaintext';
import 'highlight.js/styles/github.css';

import type { RecordContentType, SessionTargetSearchState } from '../../../../types';
import { useTextSelection } from '../../hooks/useTextSelection';
import { useMarkSearch } from '../../hooks/useMarkSearch';
import {
  getHighlightLanguage,
  DEFAULT_SHOW_MAX_SIZE,
} from '../../helper/contentType';

hljs.registerLanguage('json', json);
hljs.registerLanguage('xml', xml);
hljs.registerLanguage('html', xml);
hljs.registerLanguage('javascript', javascript);
hljs.registerLanguage('css', css);
hljs.registerLanguage('plaintext', plaintext);

const formatJsonContent = (text: string): { formatted: string; isJson: boolean } => {
  try {
    const parsed = JSON.parse(text);
    return { formatted: JSON.stringify(parsed, null, 2), isJson: true };
  } catch {
    return { formatted: text, isJson: false };
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

interface HighLightBodyProps {
  data?: string | null;
  contentType: RecordContentType;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

export const HighLightBody = ({
  data,
  contentType,
  searchValue,
  onSearch,
}: HighLightBodyProps) => {
  const { token } = theme.useToken();
  const [showAll, setShowAll] = useState(false);
  const [isFormatted, setIsFormatted] = useState(true);
  const wrapperRef = useTextSelection(!!data);
  const codeRef = useRef<HTMLElement>(null);

  const isJsonType = contentType === 'JSON';

  const processedData = useMemo(() => {
    if (!data) return '';
    if (isJsonType && isFormatted) {
      const { formatted, isJson } = formatJsonContent(data);
      return isJson ? formatted : data;
    }
    return data;
  }, [data, isJsonType, isFormatted]);

  const truncated = useMemo(() => {
    if (!processedData) return '';
    if (!showAll && processedData.length > DEFAULT_SHOW_MAX_SIZE) {
      return processedData.substring(0, DEFAULT_SHOW_MAX_SIZE);
    }
    return processedData;
  }, [processedData, showAll]);

  const highlighted = useMemo(() => {
    if (!truncated) return '';
    try {
      const lang = getHighlightLanguage(contentType);
      if (lang === 'plaintext' || truncated.length > 200 * 1024) {
        return truncated;
      }
      const result = hljs.highlight(truncated, { language: lang });
      return result.value;
    } catch {
      return truncated;
    }
  }, [truncated, contentType]);

  const shouldShowMore = !showAll && (processedData?.length ?? 0) > DEFAULT_SHOW_MAX_SIZE;

  const handleCopy = useCallback(async () => {
    if (processedData) {
      const success = await copyToClipboard(processedData);
      if (success) {
        message.success('Copied to clipboard');
      } else {
        message.error('Failed to copy');
      }
    }
  }, [processedData]);

  const toggleFormat = useCallback(() => {
    setIsFormatted((prev) => !prev);
  }, []);

  useMarkSearch(
    searchValue,
    () => codeRef.current,
    onSearch
  );

  if (!data) {
    return null;
  }

  return (
    <div ref={wrapperRef} style={{ position: 'relative' }}>
      <div
        style={{
          position: 'absolute',
          top: 4,
          right: 8,
          zIndex: 1,
          display: 'flex',
          gap: 4,
        }}
      >
        {isJsonType && (
          <Tooltip title={isFormatted ? 'Raw' : 'Format'}>
            <Button
              type="text"
              size="small"
              icon={<FormatPainterOutlined />}
              onClick={toggleFormat}
              style={{
                background: token.colorBgContainer,
                opacity: isFormatted ? 1 : 0.6,
              }}
            />
          </Tooltip>
        )}
        <Tooltip title="Copy">
          <Button
            type="text"
            size="small"
            icon={<CopyOutlined />}
            onClick={handleCopy}
            style={{ background: token.colorBgContainer }}
          />
        </Tooltip>
      </div>
      <pre
        style={{
          margin: 0,
          padding: 8,
          paddingTop: 32,
          fontSize: 12,
          fontFamily: 'monospace',
          backgroundColor: token.colorBgLayout,
          borderRadius: 4,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-all',
          lineHeight: 1.4,
        }}
      >
        <code
          ref={codeRef}
          dangerouslySetInnerHTML={{ __html: highlighted }}
          style={{ fontFamily: 'inherit' }}
        />
      </pre>
      {shouldShowMore && (
        <Button
          type="link"
          onClick={() => setShowAll(true)}
          style={{
            position: 'absolute',
            bottom: 8,
            right: 8,
            background: token.colorBgContainer,
          }}
        >
          Show All ({Math.round((processedData.length - DEFAULT_SHOW_MAX_SIZE) / 1024)}KB more)
        </Button>
      )}
    </div>
  );
};
