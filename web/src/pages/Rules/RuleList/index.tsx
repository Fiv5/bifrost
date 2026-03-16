import { useMemo, useState, useCallback } from 'react';
import {
  Input,
  Button,
  Dropdown,
  Modal,
  message,
  Switch,
  Tooltip,
  Spin,
  Select,
} from 'antd';
import type { MenuProps } from 'antd';
import {
  PlusOutlined,
  ReloadOutlined,
  SearchOutlined,
  CheckOutlined,
  EditOutlined,
  DeleteOutlined,
  PoweroffOutlined,
  ExportOutlined,
} from '@ant-design/icons';
import { useRulesStore } from '../../../stores/useRulesStore';
import { ImportBifrostButton } from '../../../components/ImportBifrostButton';
import { useExportBifrost } from '../../../hooks/useExportBifrost';
import styles from './index.module.css';

type RuleSortMode = 'created_desc' | 'updated_desc' | 'name_asc';

const ruleSortOptions = [
  { label: 'Newest', value: 'created_desc' },
  { label: 'Updated', value: 'updated_desc' },
  { label: 'Name', value: 'name_asc' },
];

export default function RuleList() {
  const {
    rules,
    selectedRuleName,
    searchKeyword,
    loading,
    editingContent,
    fetchRules,
    selectRule,
    createRule,
    deleteRule,
    toggleRule,
    renameRule,
    setSearchKeyword,
    hasUnsavedChanges,
  } = useRulesStore();

  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [newRuleName, setNewRuleName] = useState('');
  const [renameModalVisible, setRenameModalVisible] = useState(false);
  const [renameTarget, setRenameTarget] = useState<string | null>(null);
  const [newName, setNewName] = useState('');
  const [selectedRules, setSelectedRules] = useState<string[]>([]);
  const [sortMode, setSortMode] = useState<RuleSortMode>('created_desc');
  const { exportFile } = useExportBifrost();

  const filteredRules = useMemo(() => {
    const sortedRules = [...rules].sort((left, right) => {
      if (sortMode === 'updated_desc') {
        return (
          Date.parse(right.updated_at) - Date.parse(left.updated_at) ||
          left.name.localeCompare(right.name)
        );
      }
      if (sortMode === 'name_asc') {
        return left.name.localeCompare(right.name);
      }
      return (
        Date.parse(right.created_at) - Date.parse(left.created_at) ||
        left.name.localeCompare(right.name)
      );
    });
    if (!searchKeyword) return sortedRules;
    const keyword = searchKeyword.toLowerCase();
    return sortedRules.filter((rule) => rule.name.toLowerCase().includes(keyword));
  }, [rules, searchKeyword, sortMode]);

  const handleCreate = async () => {
    if (!newRuleName.trim()) {
      message.error('Rule name is required');
      return;
    }
    const success = await createRule(newRuleName.trim(), '# New rule\n');
    if (success) {
      message.success('Rule created');
      setCreateModalVisible(false);
      setNewRuleName('');
    }
  };

  const handleDelete = async (name: string) => {
    Modal.confirm({
      title: 'Delete Rule',
      content: `Are you sure to delete "${name}"?`,
      okText: 'Delete',
      okType: 'danger',
      cancelText: 'Cancel',
      onOk: async () => {
        const success = await deleteRule(name);
        if (success) {
          message.success('Rule deleted');
        }
      },
    });
  };

  const handleToggle = async (name: string, enabled: boolean) => {
    const success = await toggleRule(name, enabled);
    if (success) {
      message.success(`Rule ${enabled ? 'enabled' : 'disabled'}`);
    }
  };

  const handleRename = async () => {
    if (!renameTarget || !newName.trim()) return;
    if (newName.trim() === renameTarget) {
      setRenameModalVisible(false);
      return;
    }
    const success = await renameRule(renameTarget, newName.trim());
    if (success) {
      message.success('Rule renamed');
      setRenameModalVisible(false);
      setRenameTarget(null);
      setNewName('');
    }
  };

  const handleExport = useCallback(
    async (names: string[]) => {
      if (names.length === 0) return;
      const filename =
        names.length === 1
          ? `${names[0]}.bifrost`
          : `bifrost-rules-${names.length}.bifrost`;
      await exportFile('rules', { rule_names: names }, filename);
    },
    [exportFile]
  );

  const handleImportSuccess = useCallback(() => {
    fetchRules();
  }, [fetchRules]);

  const handleSelect = useCallback(
    (name: string, isMultiSelect: boolean) => {
      if (isMultiSelect) {
        setSelectedRules((prev) =>
          prev.includes(name) ? prev.filter((n) => n !== name) : [...prev, name]
        );
      } else {
        setSelectedRules([]);
        selectRule(name);
      }
    },
    [selectRule]
  );

  const getContextMenuItems = (name: string, enabled: boolean): MenuProps['items'] => {
    const isSelected = selectedRules.includes(name);
    const exportNames = isSelected && selectedRules.length > 0 ? selectedRules : [name];

    return [
      {
        key: 'toggle',
        icon: enabled ? <PoweroffOutlined /> : <CheckOutlined />,
        label: enabled ? 'Disable' : 'Enable',
        onClick: () => handleToggle(name, !enabled),
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
        key: 'export',
        icon: <ExportOutlined />,
        label: `Export${exportNames.length > 1 ? ` (${exportNames.length})` : ''}`,
        onClick: () => handleExport(exportNames),
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
  };

  return (
    <div className={styles.container} data-testid="rules-list">
      <div className={styles.header}>
        <span className={styles.headerTitle}>Rules</span>
        <div className={styles.headerActions}>
          <Tooltip title="New Rule">
            <Button
              type="text"
              size="small"
              icon={<PlusOutlined />}
              onClick={() => setCreateModalVisible(true)}
              data-testid="rule-new-button"
            />
          </Tooltip>
          <Tooltip title="Refresh">
            <Button
              type="text"
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => fetchRules()}
              data-testid="rule-refresh-button"
            />
          </Tooltip>
          <ImportBifrostButton
            expectedType="rules"
            onImportSuccess={handleImportSuccess}
            buttonText=""
            buttonType="text"
            size="small"
          />
        </div>
      </div>
      <div className={styles.searchBox}>
        <Input
          size="small"
          placeholder="Search rules..."
          prefix={<SearchOutlined style={{ color: '#999' }} />}
          value={searchKeyword}
          onChange={(e) => setSearchKeyword(e.target.value)}
          allowClear
          data-testid="rule-search-input"
        />
        <Select
          size="small"
          value={sortMode}
          onChange={(value: RuleSortMode) => setSortMode(value)}
          options={ruleSortOptions}
          className={styles.sortControl}
          popupMatchSelectWidth={false}
          data-testid="rule-sort-select"
        />
      </div>

      <div className={styles.listContainer}>
        {loading && rules.length === 0 ? (
          <div className={styles.loading}>
            <Spin size="small" />
          </div>
        ) : (
          <div className={styles.list}>
            {filteredRules.map((rule) => {
              const isSelected = selectedRuleName === rule.name;
              const hasChanges = hasUnsavedChanges(rule.name) || editingContent[rule.name] !== undefined;

              return (
                <Dropdown
                  key={rule.name}
                  menu={{ items: getContextMenuItems(rule.name, rule.enabled) }}
                  trigger={['contextMenu']}
                >
                  <div
                    className={`${styles.item} ${isSelected ? styles.selected : ''} ${selectedRules.includes(rule.name) ? styles.multiSelected : ''}`}
                    onClick={(e) => handleSelect(rule.name, e.ctrlKey || e.metaKey)}
                    onDoubleClick={() => handleToggle(rule.name, !rule.enabled)}
                    data-testid="rule-item"
                    data-rule-name={rule.name}
                    data-rule-enabled={rule.enabled ? "true" : "false"}
                  >
                    <div className={styles.itemContent}>
                      <span className={styles.itemName} title={rule.name}>
                        {rule.name}
                      </span>
                      <div className={styles.itemMeta}>
                        {hasChanges && (
                          <Tooltip title="Unsaved changes">
                            <span className={styles.unsavedDot} />
                          </Tooltip>
                        )}
                        {rule.enabled && (
                          <Tooltip title="Enabled">
                            <CheckOutlined className={styles.enabledIcon} />
                          </Tooltip>
                        )}
                      </div>
                    </div>
                    <div className={styles.itemExtra}>
                      <Switch
                        size="small"
                        checked={rule.enabled}
                        onChange={(checked, e) => {
                          e.stopPropagation();
                          handleToggle(rule.name, checked);
                        }}
                      />
                    </div>
                  </div>
                </Dropdown>
              );
            })}
            {filteredRules.length === 0 && !loading && (
              <div className={styles.empty}>
                {searchKeyword ? 'No matching rules' : 'No rules yet'}
              </div>
            )}
          </div>
        )}
      </div>

      <div className={styles.footer}>
        <span className={styles.stats}>
          {rules.length} rules, {rules.filter((r) => r.enabled).length} enabled
        </span>
      </div>

      <Modal
        title="New Rule"
        open={createModalVisible}
        onCancel={() => {
          setCreateModalVisible(false);
          setNewRuleName('');
        }}
        onOk={handleCreate}
        okText="Create"
        cancelText="Cancel"
      >
        <Input
          placeholder="Rule name"
          value={newRuleName}
          onChange={(e) => setNewRuleName(e.target.value)}
          onPressEnter={handleCreate}
          autoFocus
        />
      </Modal>

      <Modal
        title="Rename Rule"
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
