import { useMemo, useRef, useState, useEffect } from 'react';
import { theme, Button } from 'antd';
import type { SessionTargetSearchState } from '../../../../types';
import { useTextSelection } from '../../hooks/useTextSelection';
import { useMarkSearch } from '../../hooks/useMarkSearch';
import { DEFAULT_SHOW_MAX_SIZE } from '../../helper/contentType';

interface HexViewProps {
  data?: string | null;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
}

const toHexView = (text: string): string => {
  const encoder = new TextEncoder();
  const buffer = encoder.encode(text);
  const lines: string[] = [];

  for (let i = 0; i < buffer.length; i += 16) {
    const slice = buffer.slice(i, i + 16);
    const hex = Array.from(slice)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(' ');
    const ascii = Array.from(slice)
      .map((b) => (b >= 32 && b < 127 ? String.fromCharCode(b) : '.'))
      .join('');
    lines.push(
      `${i.toString(16).padStart(8, '0')}  ${hex.padEnd(48)}  ${ascii}`
    );
  }

  return lines.join('\n');
};

export const HexView = ({ data, searchValue, onSearch }: HexViewProps) => {
  const { token } = theme.useToken();
  const [showAll, setShowAll] = useState(false);
  const wrapperRef = useTextSelection(!!data);
  const contentRef = useRef<HTMLPreElement>(null);

  const truncatedData = useMemo(() => {
    if (!data) return '';
    if (!showAll && data.length > DEFAULT_SHOW_MAX_SIZE) {
      return data.substring(0, DEFAULT_SHOW_MAX_SIZE);
    }
    return data;
  }, [data, showAll]);

  const hexData = useMemo(() => {
    if (!truncatedData) return '';
    return toHexView(truncatedData);
  }, [truncatedData]);

  const shouldShowMore = !showAll && (data?.length ?? 0) > DEFAULT_SHOW_MAX_SIZE;

  const { startMarkSearch } = useMarkSearch(
    searchValue,
    () => contentRef.current,
    onSearch
  );

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    el.textContent = hexData;
  }, [hexData]);

  useEffect(() => {
    if (!searchValue.value) return;
    startMarkSearch();
  }, [hexData, searchValue.value, startMarkSearch]);

  if (!data) {
    return null;
  }

  return (
    <div ref={wrapperRef} style={{ position: 'relative' }}>
      <pre
        ref={contentRef}
        style={{
          margin: 0,
          padding: 8,
          fontSize: 11,
          fontFamily: 'monospace',
          backgroundColor: token.colorBgLayout,
          borderRadius: 4,
          whiteSpace: 'pre',
          overflowX: 'auto',
          lineHeight: 1.4,
        }}
      />
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
