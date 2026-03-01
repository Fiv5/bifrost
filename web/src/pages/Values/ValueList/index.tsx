import { useMemo, useState } from 'react';
import {
  Input,
  Button,
  Dropdown,
  Modal,
  message,
  Tooltip,
  Spin,
} from 'antd';
import type { MenuProps } from 'antd';
import {
  PlusOutlined,
  ReloadOutlined,
  SearchOutlined,
  EditOutlined,
  DeleteOutlined,
  CopyOutlined,
} from '@ant-design/icons';
import { useValuesStore } from '../../../stores/useValuesStore';
import styles from './index.module.css';

export default function ValueList() {
  const {
    values,
    selectedValueName,
    searchKeyword,
    loading,
    editingContent,
    fetchValues,
    selectValue,
    createValue,
    deleteValue,
    renameValue,
    setSearchKeyword,
    hasUnsavedChanges,
  } = useValuesStore();

  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [newValueName, setNewValueName] = useState('');
  const [renameModalVisible, setRenameModalVisible] = useState(false);
  const [renameTarget, setRenameTarget] = useState<string | null>(null);
  const [newName, setNewName] = useState('');

  const filteredValues = useMemo(() => {
    if (!searchKeyword) return values;
    const keyword = searchKeyword.toLowerCase();
    return values.filter(
      (v) =>
        v.name.toLowerCase().includes(keyword) ||
        v.value.toLowerCase().includes(keyword)
    );
  }, [values, searchKeyword]);

  const handleCreate = async () => {
    if (!newValueName.trim()) {
      message.error('Value name is required');
      return;
    }
    const success = await createValue(newValueName.trim(), '');
    if (success) {
      message.success('Value created');
      setCreateModalVisible(false);
      setNewValueName('');
    }
  };

  const handleDelete = async (name: string) => {
    Modal.confirm({
      title: 'Delete Value',
      content: `Are you sure to delete "${name}"?`,
      okText: 'Delete',
      okType: 'danger',
      cancelText: 'Cancel',
      onOk: async () => {
        const success = await deleteValue(name);
        if (success) {
          message.success('Value deleted');
        }
      },
    });
  };

  const handleRename = async () => {
    if (!renameTarget || !newName.trim()) return;
    if (newName.trim() === renameTarget) {
      setRenameModalVisible(false);
      return;
    }
    const success = await renameValue(renameTarget, newName.trim());
    if (success) {
      message.success('Value renamed');
      setRenameModalVisible(false);
      setRenameTarget(null);
      setNewName('');
    }
  };

  const handleCopy = async (name: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      message.success(`Copied "${name}" to clipboard`);
    } catch {
      message.error('Failed to copy');
    }
  };

  const getContextMenuItems = (name: string, value: string): MenuProps['items'] => [
    {
      key: 'copy',
      icon: <CopyOutlined />,
      label: 'Copy Value',
      onClick: () => handleCopy(name, value),
    },
    {
      key: 'rename',
      icon: <EditOutlined />,
      label: 'Rename',
      onClick: () => {
        setRenameTarget(name);
        setNewName(name);
        setRenameModalVisible(true);
      },
    },
    {
      type: 'divider',
    },
    {
      key: 'delete',
      icon: <DeleteOutlined />,
      label: 'Delete',
      danger: true,
      onClick: () => handleDelete(name),
    },
  ];

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <div className={styles.toolbar}>
          <Tooltip title="New Value">
            <Button
              type="text"
              size="small"
              icon={<PlusOutlined />}
              onClick={() => setCreateModalVisible(true)}
            />
          </Tooltip>
          <Tooltip title="Refresh">
            <Button
              type="text"
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => fetchValues()}
            />
          </Tooltip>
        </div>
        <Input
          size="small"
          placeholder="Search values..."
          prefix={<SearchOutlined style={{ color: '#999' }} />}
          value={searchKeyword}
          onChange={(e) => setSearchKeyword(e.target.value)}
          allowClear
          className={styles.searchInput}
        />
      </div>

      <div className={styles.listContainer}>
        {loading && values.length === 0 ? (
          <div className={styles.loading}>
            <Spin size="small" />
          </div>
        ) : (
          <div className={styles.list}>
            {filteredValues.map((item) => {
              const isSelected = selectedValueName === item.name;
              const hasChanges = hasUnsavedChanges(item.name) || editingContent[item.name] !== undefined;

              return (
                <Dropdown
                  key={item.name}
                  menu={{ items: getContextMenuItems(item.name, item.value) }}
                  trigger={['contextMenu']}
                >
                  <div
                    className={`${styles.item} ${isSelected ? styles.selected : ''}`}
                    onClick={() => selectValue(item.name)}
                  >
                    <div className={styles.itemContent}>
                      <span className={styles.itemName} title={item.name}>
                        {item.name}
                      </span>
                      <div className={styles.itemMeta}>
                        {hasChanges && (
                          <Tooltip title="Unsaved changes">
                            <span className={styles.unsavedDot} />
                          </Tooltip>
                        )}
                      </div>
                    </div>
                    <div className={styles.itemPreview} title={item.value}>
                      {item.value.length > 30
                        ? `${item.value.slice(0, 30).replace(/\n/g, '↵')}...`
                        : item.value.replace(/\n/g, '↵')}
                    </div>
                  </div>
                </Dropdown>
              );
            })}
            {filteredValues.length === 0 && !loading && (
              <div className={styles.empty}>
                {searchKeyword ? 'No matching values' : 'No values yet'}
              </div>
            )}
          </div>
        )}
      </div>

      <div className={styles.footer}>
        <span className={styles.stats}>{values.length} values</span>
      </div>

      <Modal
        title="New Value"
        open={createModalVisible}
        onCancel={() => {
          setCreateModalVisible(false);
          setNewValueName('');
        }}
        onOk={handleCreate}
        okText="Create"
        cancelText="Cancel"
      >
        <Input
          placeholder="Value name (e.g., api_key, auth_token)"
          value={newValueName}
          onChange={(e) => setNewValueName(e.target.value)}
          onPressEnter={handleCreate}
          autoFocus
        />
      </Modal>

      <Modal
        title="Rename Value"
        open={renameModalVisible}
        onCancel={() => {
          setRenameModalVisible(false);
          setRenameTarget(null);
          setNewName('');
        }}
        onOk={handleRename}
        okText="Rename"
        cancelText="Cancel"
      >
        <Input
          placeholder="New name"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onPressEnter={handleRename}
          autoFocus
        />
      </Modal>
    </div>
  );
}
