import { useMemo, useRef, useState, useCallback } from 'react';
import { theme, Button, Modal, message } from 'antd';
import { LockOutlined } from '@ant-design/icons';
import type { SessionTargetSearchState } from '../../../../types';
import { useTextSelection } from '../../hooks/useTextSelection';
import { useMarkSearch } from '../../hooks/useMarkSearch';
import { DEFAULT_SHOW_MAX_SIZE } from '../../helper/contentType';
import { getTlsConfig, updateTlsConfig, disconnectByDomain } from '../../../../api/config';

interface RawProps {
  type: 'request' | 'response';
  method?: string;
  url?: string;
  protocol?: string;
  status?: number;
  headers: [string, string][] | null;
  body?: string | null;
  searchValue: SessionTargetSearchState;
  onSearch: (v: Partial<SessionTargetSearchState>) => void;
  isTunnel?: boolean;
  host?: string;
}

const STATUS_CODES: Record<number, string> = {
  200: 'OK',
  201: 'Created',
  204: 'No Content',
  301: 'Moved Permanently',
  302: 'Found',
  304: 'Not Modified',
  400: 'Bad Request',
  401: 'Unauthorized',
  403: 'Forbidden',
  404: 'Not Found',
  500: 'Internal Server Error',
  502: 'Bad Gateway',
  503: 'Service Unavailable',
  504: 'Gateway Timeout',
};

const encodeHtml = (str: string): string => {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
};

export const Raw = ({
  type,
  method,
  url,
  protocol,
  status,
  headers,
  body,
  searchValue,
  onSearch,
  isTunnel,
  host,
}: RawProps) => {
  const { token } = theme.useToken();
  const [showAll, setShowAll] = useState(false);
  const wrapperRef = useTextSelection(true);
  const contentRef = useRef<HTMLDivElement>(null);

  const handleAddToInterceptList = useCallback(() => {
    if (!host) {
      message.error('No host found for this request');
      return;
    }

    Modal.confirm({
      title: 'Add to Intercept List',
      content: `Add "${host}" to TLS intercept list? This will enable HTTPS inspection for this domain and disconnect existing tunnel connections.`,
      okText: 'Add',
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          const currentConfig = await getTlsConfig();
          if (currentConfig.intercept_include.includes(host)) {
            message.info(`"${host}" is already in the intercept list`);
            return;
          }

          const newIncludeList = [...currentConfig.intercept_include, host];
          await updateTlsConfig({ intercept_include: newIncludeList });

          await disconnectByDomain(host);

          message.success(`Added "${host}" to intercept list and disconnected existing connections`);
        } catch (error) {
          message.error('Failed to add domain to intercept list');
          console.error(error);
        }
      },
    });
  }, [host]);

  const showInterceptButton = type === 'response' && isTunnel && host;
  const isTunnelResponse = type === 'response' && isTunnel;

  useMarkSearch(
    searchValue,
    () => contentRef.current,
    onSearch
  );

  const shouldShowMore = !showAll && (body?.length ?? 0) > DEFAULT_SHOW_MAX_SIZE;

  const headersHtml = useMemo(() => {
    if (!headers || headers.length === 0) return '';
    return headers
      .map(
        ([key, value]) =>
          `<span style="color: ${token.colorPrimary}">${encodeHtml(key)}</span>: ${encodeHtml(value)}`
      )
      .join('<br/>');
  }, [headers, token.colorPrimary]);

  const statusLine = useMemo(() => {
    if (type === 'request') {
      return `<span style="color: ${token.colorSuccess}">${method}</span> ${encodeHtml(url || '')} <span style="color: ${token.colorTextSecondary}">${protocol}</span>`;
    }
    const statusColor =
      (status ?? 0) >= 400
        ? token.colorError
        : (status ?? 0) >= 300
          ? token.colorWarning
          : token.colorSuccess;
    return `<span style="color: ${token.colorTextSecondary}">${protocol}</span> <span style="color: ${statusColor}">${status} ${STATUS_CODES[status ?? 0] || ''}</span>`;
  }, [type, method, url, protocol, status, token]);

  let bodyText = body || '';
  if (!showAll && bodyText.length > DEFAULT_SHOW_MAX_SIZE) {
    bodyText = bodyText.substring(0, DEFAULT_SHOW_MAX_SIZE);
  }

  const hasNoData = (!headers || headers.length === 0) && !body && !status;
  if (isTunnelResponse && hasNoData) {
    return (
      <div
        ref={wrapperRef}
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: 200,
          backgroundColor: token.colorBgLayout,
          borderRadius: 4,
        }}
      >
        {showInterceptButton ? (
          <Button
            type="primary"
            icon={<LockOutlined />}
            onClick={handleAddToInterceptList}
            size="large"
          >
            Intercept this domain
          </Button>
        ) : (
          <div style={{ color: token.colorTextSecondary }}>
            No response data for tunnel connection
          </div>
        )}
      </div>
    );
  }

  return (
    <div ref={wrapperRef} style={{ position: 'relative' }}>
      <div
        ref={contentRef}
        style={{
          fontFamily: 'monospace',
          fontSize: 12,
          padding: 8,
          backgroundColor: token.colorBgLayout,
          borderRadius: 4,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-all',
          lineHeight: 1.4,
        }}
      >
        <div dangerouslySetInnerHTML={{ __html: statusLine }} />
        {headersHtml && (
          <div
            dangerouslySetInnerHTML={{ __html: headersHtml }}
            style={{ marginTop: 4 }}
          />
        )}
        {bodyText && (
          <>
            <br />
            <div style={{ color: token.colorTextSecondary }}>{bodyText}</div>
          </>
        )}
      </div>
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
          Show All ({Math.round(((body?.length ?? 0) - DEFAULT_SHOW_MAX_SIZE) / 1024)}KB more)
        </Button>
      )}
    </div>
  );
};
