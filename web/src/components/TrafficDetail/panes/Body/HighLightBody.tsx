import { useMemo, useRef, useState } from 'react';
import { theme, Button } from 'antd';
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
  const wrapperRef = useTextSelection(!!data);
  const codeRef = useRef<HTMLElement>(null);

  const truncated = useMemo(() => {
    if (!data) return '';
    if (!showAll && data.length > DEFAULT_SHOW_MAX_SIZE) {
      return data.substring(0, DEFAULT_SHOW_MAX_SIZE);
    }
    return data;
  }, [data, showAll]);

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

  const shouldShowMore = !showAll && (data?.length ?? 0) > DEFAULT_SHOW_MAX_SIZE;

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
      <pre
        style={{
          margin: 0,
          padding: 8,
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
          Show All ({Math.round((data.length - DEFAULT_SHOW_MAX_SIZE) / 1024)}KB more)
        </Button>
      )}
    </div>
  );
};
