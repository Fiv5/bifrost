import { useCallback, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Menu, message, Modal } from 'antd';
import type { MenuProps } from 'antd';
import {
  CopyOutlined,
  CodeOutlined,
  DownloadOutlined,
  LockOutlined,
  SendOutlined,
} from '@ant-design/icons';
import type { TrafficRecord, TrafficSummary } from '../../types';
import { generateCurl } from '../../utils/curl';
import { downloadHAR } from '../../utils/har';
import { getTrafficDetail, getRequestBody, getResponseBody } from '../../api/traffic';
import { getTlsConfig, updateTlsConfig, disconnectByDomain } from '../../api/config';
import { useReplayStore } from '../../stores/useReplayStore';

interface TrafficContextMenuProps {
  record: TrafficSummary | null;
  visible: boolean;
  position: { x: number; y: number };
  onClose: () => void;
  selectedRecords?: TrafficSummary[];
}

export default function TrafficContextMenu({
  record,
  visible,
  position,
  onClose,
  selectedRecords = [],
}: TrafficContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const importFromTraffic = useReplayStore((state) => state.importFromTraffic);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    if (visible) {
      document.addEventListener('mousedown', handleClickOutside);
      document.addEventListener('keydown', handleEscape);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [visible, onClose]);

  const copyUrl = useCallback(async () => {
    if (!record) return;
    try {
      await navigator.clipboard.writeText(record.url);
      message.success('URL copied to clipboard');
    } catch {
      message.error('Failed to copy URL');
    }
    onClose();
  }, [record, onClose]);

  const copyAsCurl = useCallback(async () => {
    if (!record) return;
    try {
      const detail = await getTrafficDetail(record.id);
      const requestBody = await getRequestBody(record.id);
      const fullRecord: TrafficRecord = {
        ...detail,
        request_body: requestBody,
      };
      const curl = generateCurl(fullRecord);
      await navigator.clipboard.writeText(curl);
      message.success('cURL command copied to clipboard');
    } catch (error) {
      message.error('Failed to generate cURL command');
      console.error(error);
    }
    onClose();
  }, [record, onClose]);

  const downloadAsHAR = useCallback(async () => {
    const recordsToExport = selectedRecords.length > 1 ? selectedRecords : (record ? [record] : []);
    if (recordsToExport.length === 0) return;

    try {
      message.loading({ content: 'Generating HAR file...', key: 'har-download' });
      const fullRecords: TrafficRecord[] = [];
      
      for (const r of recordsToExport) {
        const detail = await getTrafficDetail(r.id);
        const [requestBody, responseBody] = await Promise.all([
          getRequestBody(r.id),
          getResponseBody(r.id),
        ]);
        fullRecords.push({
          ...detail,
          request_body: requestBody,
          response_body: responseBody,
        });
      }
      
      downloadHAR(fullRecords);
      message.success({ content: `Downloaded ${fullRecords.length} request(s) as HAR`, key: 'har-download' });
    } catch (error) {
      message.error({ content: 'Failed to download HAR file', key: 'har-download' });
      console.error(error);
    }
    onClose();
  }, [record, selectedRecords, onClose]);

  const addToInterceptList = useCallback(() => {
    if (!record) return;

    const host = record.host;
    if (!host) {
      message.error('No host found for this request');
      onClose();
      return;
    }

    onClose();

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
  }, [record, onClose]);

  const replayRequest = useCallback(async () => {
    if (!record) return;
    onClose();
    await importFromTraffic(record.id);
    navigate('/replay');
  }, [record, onClose, importFromTraffic, navigate]);

  if (!visible || !record) return null;

  const isTunnel = record.is_tunnel || record.method === 'CONNECT';
  const hasMultipleSelected = selectedRecords.length > 1;

  const menuItems: MenuProps['items'] = [
    ...(!isTunnel ? [
      {
        key: 'replay',
        icon: <SendOutlined />,
        label: 'Replay',
        onClick: replayRequest,
      },
      { type: 'divider' as const },
    ] : []),
    {
      key: 'copy-url',
      icon: <CopyOutlined />,
      label: 'Copy URL',
      onClick: copyUrl,
    },
    {
      key: 'copy-curl',
      icon: <CodeOutlined />,
      label: 'Copy as cURL',
      onClick: copyAsCurl,
    },
    {
      key: 'download-har',
      icon: <DownloadOutlined />,
      label: hasMultipleSelected ? `Download ${selectedRecords.length} requests as HAR` : 'Download as HAR',
      onClick: downloadAsHAR,
    },
    ...(isTunnel ? [
      { type: 'divider' as const },
      {
        key: 'add-intercept',
        icon: <LockOutlined />,
        label: 'Add to Intercept List',
        onClick: addToInterceptList,
      },
    ] : []),
  ];

  return (
    <div
      ref={menuRef}
      style={{
        position: 'fixed',
        left: position.x,
        top: position.y,
        zIndex: 1050,
        boxShadow: '0 6px 16px 0 rgba(0, 0, 0, 0.08), 0 3px 6px -4px rgba(0, 0, 0, 0.12), 0 9px 28px 8px rgba(0, 0, 0, 0.05)',
        borderRadius: 8,
      }}
    >
      <Menu
        items={menuItems}
        style={{ borderRadius: 8 }}
        mode="vertical"
      />
    </div>
  );
}
