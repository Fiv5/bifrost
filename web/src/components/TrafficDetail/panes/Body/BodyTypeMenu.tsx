import { useMemo } from 'react';
import { Dropdown, Typography, Space } from 'antd';
import { DownOutlined, CheckOutlined } from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { DisplayFormat } from '../../../../types';
import type { RecordContentType } from '../../../../types';

interface BodyTypeMenuProps {
  value: string;
  onChange: (v: string) => void;
  contentType: RecordContentType;
}

export const BodyTypeMenu = ({
  value,
  onChange,
  contentType,
}: BodyTypeMenuProps) => {
  const menuItems = useMemo<MenuProps['items']>(() => {
    const items: MenuProps['items'] = [];

    if (contentType === 'Media') {
      items.push({
        key: DisplayFormat.Media,
        label: (
          <Space>
            {value === DisplayFormat.Media && <CheckOutlined />}
            Media
          </Space>
        ),
      });
    }

    if (contentType !== 'Media') {
      items.push({
        key: DisplayFormat.HighLight,
        label: (
          <Space>
            {value === DisplayFormat.HighLight && <CheckOutlined />}
            {contentType === 'JSON' ? 'JSON' : contentType}
          </Space>
        ),
      });
    }

    if (contentType === 'JSON') {
      items.push({
        key: DisplayFormat.Tree,
        label: (
          <Space>
            {value === DisplayFormat.Tree && <CheckOutlined />}
            Tree
          </Space>
        ),
      });
    }

    items.push({
      key: DisplayFormat.Hex,
      label: (
        <Space>
          {value === DisplayFormat.Hex && <CheckOutlined />}
          Hex
        </Space>
      ),
    });

    return items;
  }, [contentType, value]);

  const activeLabel = useMemo(() => {
    switch (value) {
      case DisplayFormat.HighLight:
        return contentType === 'JSON' ? 'JSON' : contentType;
      case DisplayFormat.Hex:
        return 'Hex';
      case DisplayFormat.Tree:
        return 'Tree';
      case DisplayFormat.Media:
        return 'Media';
      default:
        return 'View';
    }
  }, [value, contentType]);

  const handleClick: MenuProps['onClick'] = ({ key }) => {
    onChange(key);
  };

  return (
    <Dropdown menu={{ items: menuItems, onClick: handleClick }} trigger={['click']}>
      <Typography.Text
        type="secondary"
        style={{ cursor: 'pointer', fontSize: 12 }}
      >
        {activeLabel} <DownOutlined style={{ fontSize: 10 }} />
      </Typography.Text>
    </Dropdown>
  );
};
