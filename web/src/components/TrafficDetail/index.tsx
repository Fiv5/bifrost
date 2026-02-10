import { useMemo } from 'react';
import { Descriptions, Tabs, Typography, Empty, theme } from 'antd';
import type { CSSProperties } from 'react';
import dayjs from 'dayjs';
import type { TrafficRecord } from '../../types';

const { Text, Paragraph } = Typography;

interface TrafficDetailProps {
  record: TrafficRecord | null;
  requestBody: string | null;
  responseBody: string | null;
}

const styles: Record<string, CSSProperties> = {
  container: {
    height: '100%',
    display: 'flex',
    flexDirection: 'column',
  },
  emptyContainer: {
    height: '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  },
  summarySection: {
    marginBottom: 16,
  },
  tabContent: {
    flex: 1,
    overflow: 'auto',
  },
  headerItem: {
    fontFamily: 'monospace',
    fontSize: 12,
    lineHeight: 1.6,
  },
  bodyContent: {
    fontFamily: 'monospace',
    fontSize: 12,
    margin: 0,
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    maxHeight: 400,
    overflow: 'auto',
    padding: 12,
    borderRadius: 4,
  },
  rawContent: {
    fontFamily: 'monospace',
    fontSize: 11,
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-all',
    maxHeight: 500,
    overflow: 'auto',
    padding: 12,
    borderRadius: 4,
  },
};

function getStatusMessage(status: number): string {
  const messages: Record<number, string> = {
    100: 'Continue',
    101: 'Switching Protocols',
    200: 'OK',
    201: 'Created',
    202: 'Accepted',
    204: 'No Content',
    206: 'Partial Content',
    301: 'Moved Permanently',
    302: 'Found',
    303: 'See Other',
    304: 'Not Modified',
    307: 'Temporary Redirect',
    308: 'Permanent Redirect',
    400: 'Bad Request',
    401: 'Unauthorized',
    403: 'Forbidden',
    404: 'Not Found',
    405: 'Method Not Allowed',
    408: 'Request Timeout',
    409: 'Conflict',
    413: 'Payload Too Large',
    414: 'URI Too Long',
    415: 'Unsupported Media Type',
    429: 'Too Many Requests',
    500: 'Internal Server Error',
    501: 'Not Implemented',
    502: 'Bad Gateway',
    503: 'Service Unavailable',
    504: 'Gateway Timeout',
  };
  return messages[status] || '';
}

export default function TrafficDetail({ record, requestBody, responseBody }: TrafficDetailProps) {
  const { token } = theme.useToken();

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '-';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  };

  const formatBody = (body: string | null) => {
    if (!body) return <Text type="secondary">No body content</Text>;

    try {
      const parsed = JSON.parse(body);
      const formatted = JSON.stringify(parsed, null, 2);
      return (
        <pre style={{ ...styles.bodyContent, background: token.colorBgLayout }}>
          {formatted}
        </pre>
      );
    } catch {
      return (
        <pre style={{ ...styles.bodyContent, background: token.colorBgLayout }}>
          {body}
        </pre>
      );
    }
  };

  const formatHeaders = (headers: [string, string][] | null) => {
    if (!headers || headers.length === 0) {
      return <Text type="secondary">No headers</Text>;
    }
    return (
      <div style={styles.headerItem}>
        {headers.map(([key, value], index) => (
          <div key={index}>
            <Text strong>{key}:</Text> <Text>{value}</Text>
          </div>
        ))}
      </div>
    );
  };

  const buildRawRequest = useMemo(() => {
    if (!record) return '';
    const lines: string[] = [];
    lines.push(`${record.method} ${record.path} ${record.protocol}`);
    lines.push(`Host: ${record.host}`);
    if (record.request_headers) {
      record.request_headers.forEach(([key, value]) => {
        if (key.toLowerCase() !== 'host') {
          lines.push(`${key}: ${value}`);
        }
      });
    }
    lines.push('');
    if (requestBody) {
      lines.push(requestBody);
    }
    return lines.join('\n');
  }, [record, requestBody]);

  const buildRawResponse = useMemo(() => {
    if (!record) return '';
    const lines: string[] = [];
    lines.push(`${record.protocol} ${record.status} ${getStatusMessage(record.status)}`);
    if (record.response_headers) {
      record.response_headers.forEach(([key, value]) => {
        lines.push(`${key}: ${value}`);
      });
    }
    lines.push('');
    if (responseBody) {
      lines.push(responseBody);
    }
    return lines.join('\n');
  }, [record, responseBody]);

  const requestOverview = useMemo(() => {
    if (!record) return null;
    return (
      <Descriptions column={1} size="small" bordered>
        <Descriptions.Item label="URL">{record.url}</Descriptions.Item>
        <Descriptions.Item label="Method">{record.method}</Descriptions.Item>
        <Descriptions.Item label="Protocol">{record.protocol}</Descriptions.Item>
        <Descriptions.Item label="Host">{record.host}</Descriptions.Item>
        <Descriptions.Item label="Path">{record.path}</Descriptions.Item>
        <Descriptions.Item label="Request Size">{formatSize(record.request_size)}</Descriptions.Item>
        <Descriptions.Item label="Time">{dayjs(record.timestamp).format('YYYY-MM-DD HH:mm:ss.SSS')}</Descriptions.Item>
      </Descriptions>
    );
  }, [record]);

  if (!record) {
    return (
      <div style={styles.emptyContainer}>
        <Empty description="Select a request to view details" />
      </div>
    );
  }

  const requestTabs = [
    {
      key: 'overview',
      label: 'Overview',
      children: requestOverview,
    },
    {
      key: 'headers',
      label: 'Headers',
      children: formatHeaders(record.request_headers),
    },
    {
      key: 'body',
      label: 'Body',
      children: formatBody(requestBody),
    },
    {
      key: 'raw',
      label: 'Raw',
      children: (
        <pre style={{ ...styles.rawContent, background: token.colorBgLayout }}>
          {buildRawRequest}
        </pre>
      ),
    },
  ];

  const responseTabs = [
    {
      key: 'headers',
      label: 'Headers',
      children: formatHeaders(record.response_headers),
    },
    {
      key: 'body',
      label: 'Body',
      children: formatBody(responseBody),
    },
    {
      key: 'raw',
      label: 'Raw',
      children: (
        <pre style={{ ...styles.rawContent, background: token.colorBgLayout }}>
          {buildRawResponse}
        </pre>
      ),
    },
  ];

  const mainTabs = [
    {
      key: 'request',
      label: 'Request',
      children: <Tabs items={requestTabs} size="small" />,
    },
    {
      key: 'response',
      label: 'Response',
      children: <Tabs items={responseTabs} size="small" />,
    },
  ];

  return (
    <div style={styles.container}>
      <div style={styles.summarySection}>
        <Descriptions column={2} size="small" bordered>
          <Descriptions.Item label="Client">{record.client_ip || '-'}</Descriptions.Item>
          <Descriptions.Item label="Response Code">{record.status || '-'}</Descriptions.Item>
          <Descriptions.Item label="URL" span={2}>
            <Paragraph
              style={{ margin: 0, maxWidth: '100%' }}
              ellipsis={{ rows: 2, expandable: true }}
            >
              {record.url}
            </Paragraph>
          </Descriptions.Item>
          <Descriptions.Item label="Response Status">{getStatusMessage(record.status)}</Descriptions.Item>
          <Descriptions.Item label="Method">{record.method}</Descriptions.Item>
          <Descriptions.Item label="HTTP Version">{record.protocol}</Descriptions.Item>
          <Descriptions.Item label="Content Type">{record.content_type || '-'}</Descriptions.Item>
          <Descriptions.Item label="Size" span={2}>
            Req: {formatSize(record.request_size)} / Res: {formatSize(record.response_size)}
          </Descriptions.Item>
        </Descriptions>
      </div>

      <div style={styles.tabContent}>
        <Tabs items={mainTabs} />
      </div>
    </div>
  );
}
