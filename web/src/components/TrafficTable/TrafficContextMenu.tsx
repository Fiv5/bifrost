import { useCallback, useEffect, useRef, useMemo, useLayoutEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Menu, message, Modal } from 'antd';
import type { MenuProps } from 'antd';
import {
  CopyOutlined,
  CodeOutlined,
  DownloadOutlined,
  UnlockOutlined,
  SendOutlined,
  ExportOutlined,
  AppstoreOutlined,
} from '@ant-design/icons';
import type { TrafficRecord, TrafficSummary } from '../../types';
import { generateCurl } from '../../utils/curl';
import { downloadHAR } from '../../utils/har';
import { getTrafficDetail, getRequestBody, getResponseBody } from '../../api/traffic';
import { useReplayStore } from '../../stores/useReplayStore';
import { useExportBifrost } from '../../hooks/useExportBifrost';
import { useTlsConfigStore } from '../../stores/useTlsConfigStore';
import { showTlsWhitelistChangeSuccess } from '../../utils/tlsInterceptionNotice';

const MENU_WIDTH = 220;

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
  const { exportFile } = useExportBifrost();
  const {
    fetchConfig: fetchTlsConfig,
    addDomainToIntercept,
    addAppToIntercept,
    isDomainInIntercept,
    isAppInIntercept,
    config: tlsConfig,
  } = useTlsConfigStore();

  const isTunnel = record?.is_tunnel || record?.method === 'CONNECT';
  const hasHost = !!record?.host;
  const hasClientApp = !!record?.client_app;

  const hasMultipleSelected = selectedRecords.length > 1;

  useEffect(() => {
    if (visible && !tlsConfig) {
      fetchTlsConfig();
    }
  }, [visible, tlsConfig, fetchTlsConfig]);

  const domainAlreadyInIntercept = record?.host ? isDomainInIntercept(record.host) : false;
  const appAlreadyInIntercept = record?.client_app ? isAppInIntercept(record.client_app) : false;

  useLayoutEffect(() => {
    const menuEl = menuRef.current;
    if (!visible || !menuEl) return;

    const padding = 8;
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const menuWidth = menuEl.offsetWidth || MENU_WIDTH;
    const menuHeight = menuEl.offsetHeight || 0;

    let newX = position.x;
    let newY = position.y;

    if (newX + menuWidth > viewportWidth - padding) {
      newX = Math.max(padding, position.x - menuWidth);
    }

    if (newY + menuHeight > viewportHeight - padding) {
      newY = Math.max(padding, viewportHeight - menuHeight - padding);
    }

    menuEl.style.left = `${newX}px`;
    menuEl.style.top = `${newY}px`;
    menuEl.style.visibility = 'visible';

    return () => {
      menuEl.style.visibility = 'hidden';
    };
  }, [visible, position]);

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

  const addDomainToInterceptList = useCallback(() => {
    if (!record?.host) return;

    const host = record.host;
    onClose();

    Modal.confirm({
      title: 'Add Domain to Intercept List',
      content: `Add "${host}" to TLS intercept list? This will enable HTTPS decryption for this domain.`,
      okText: 'Add',
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          const success = await addDomainToIntercept(host);
          if (success) {
            showTlsWhitelistChangeSuccess(`Added "${host}" to intercept list`);
          } else {
            message.error('Failed to add domain to intercept list');
          }
        } catch (error) {
          message.error('Failed to add domain to intercept list');
          console.error(error);
        }
      },
    });
  }, [record, onClose, addDomainToIntercept]);

  const addAppToInterceptList = useCallback(() => {
    if (!record?.client_app) return;

    const app = record.client_app;
    onClose();

    Modal.confirm({
      title: 'Add App to Intercept List',
      content: `Add "${app}" to app intercept list? This will enable HTTPS decryption for traffic from this app.`,
      okText: 'Add',
      cancelText: 'Cancel',
      onOk: async () => {
        try {
          const success = await addAppToIntercept(app);
          if (success) {
            showTlsWhitelistChangeSuccess(`Added "${app}" to app intercept list`);
          } else {
            message.error('Failed to add app to intercept list');
          }
        } catch (error) {
          message.error('Failed to add app to intercept list');
          console.error(error);
        }
      },
    });
  }, [record, onClose, addAppToIntercept]);

  const replayRequest = useCallback(async () => {
    if (!record) return;
    onClose();
    await importFromTraffic(record.id);
    navigate('/replay');
  }, [record, onClose, importFromTraffic, navigate]);

  const exportAsBifrost = useCallback(async () => {
    const recordsToExport = selectedRecords.length > 1 ? selectedRecords : (record ? [record] : []);
    if (recordsToExport.length === 0) return;

    const recordIds = recordsToExport.map((r) => r.id);
    await exportFile('network', { record_ids: recordIds, include_body: true });
    onClose();
  }, [record, selectedRecords, exportFile, onClose]);

  const interceptMenuItems: MenuProps['items'] = useMemo(() => {
    const items: NonNullable<MenuProps['items']> = [];
    if (hasHost && !domainAlreadyInIntercept) {
      items.push({
        key: 'add-domain-intercept',
        icon: <UnlockOutlined />,
        label: `Intercept ${record?.host}`,
        onClick: addDomainToInterceptList,
      });
    }
    if (hasClientApp && !appAlreadyInIntercept) {
      items.push({
        key: 'add-app-intercept',
        icon: <AppstoreOutlined />,
        label: `Intercept ${record?.client_app}`,
        onClick: addAppToInterceptList,
      });
    }
    return items;
  }, [hasHost, hasClientApp, domainAlreadyInIntercept, appAlreadyInIntercept, record, addDomainToInterceptList, addAppToInterceptList]);

  if (!visible || !record) return null;

  const menuItems: MenuProps['items'] = hasMultipleSelected
    ? [
        {
          key: 'export-bifrost',
          icon: <ExportOutlined />,
          label: `Export ${selectedRecords.length} requests as .bifrost`,
          onClick: exportAsBifrost,
        },
      ]
    : [
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
          label: 'Download as HAR',
          onClick: downloadAsHAR,
        },
        {
          key: 'export-bifrost',
          icon: <ExportOutlined />,
          label: 'Export as .bifrost',
          onClick: exportAsBifrost,
        },
        ...(interceptMenuItems.length > 0 ? [
          { type: 'divider' as const },
          ...interceptMenuItems,
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
        visibility: 'hidden',
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
