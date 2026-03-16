import { useMemo, useState, useCallback } from "react";
import { Input, Button, Dropdown, Modal, message, Tooltip, Spin, Select } from "antd";
import type { MenuProps } from "antd";
import {
  PlusOutlined,
  ReloadOutlined,
  SearchOutlined,
  EditOutlined,
  DeleteOutlined,
  CopyOutlined,
  ExportOutlined,
  MoreOutlined,
} from "@ant-design/icons";
import { useValuesStore } from "../../../stores/useValuesStore";
import { ImportBifrostButton } from "../../../components/ImportBifrostButton";
import { useExportBifrost } from "../../../hooks/useExportBifrost";
import styles from "./index.module.css";

type ValueSortMode = "created_desc" | "updated_desc" | "name_asc";

const valueSortOptions = [
  { label: "Newest", value: "created_desc" },
  { label: "Updated", value: "updated_desc" },
  { label: "Name", value: "name_asc" },
];

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
  const [newValueName, setNewValueName] = useState("");
  const [renameModalVisible, setRenameModalVisible] = useState(false);
  const [renameTarget, setRenameTarget] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [selectedValues, setSelectedValues] = useState<string[]>([]);
  const [sortMode, setSortMode] = useState<ValueSortMode>("created_desc");
  const { exportFile } = useExportBifrost();

  const filteredValues = useMemo(() => {
    const sortedValues = [...values].sort((left, right) => {
      if (sortMode === "updated_desc") {
        return (
          Date.parse(right.updated_at) - Date.parse(left.updated_at) ||
          left.name.localeCompare(right.name)
        );
      }
      if (sortMode === "name_asc") {
        return left.name.localeCompare(right.name);
      }
      return (
        Date.parse(right.created_at) - Date.parse(left.created_at) ||
        left.name.localeCompare(right.name)
      );
    });
    if (!searchKeyword) return sortedValues;
    const keyword = searchKeyword.toLowerCase();
    return sortedValues.filter(
      (v) =>
        v.name.toLowerCase().includes(keyword) ||
        v.value.toLowerCase().includes(keyword),
    );
  }, [values, searchKeyword, sortMode]);

  const handleCreate = async () => {
    if (!newValueName.trim()) {
      message.error("Value name is required");
      return;
    }
    const success = await createValue(newValueName.trim(), "");
    if (success) {
      message.success("Value created");
      setCreateModalVisible(false);
      setNewValueName("");
    }
  };

  const handleDelete = async (name: string) => {
    Modal.confirm({
      title: "Delete Value",
      content: `Are you sure to delete "${name}"?`,
      okText: "Delete",
      okType: "danger",
      cancelText: "Cancel",
      onOk: async () => {
        const success = await deleteValue(name);
        if (success) {
          message.success("Value deleted");
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
      message.success("Value renamed");
      setRenameModalVisible(false);
      setRenameTarget(null);
      setNewName("");
    }
  };

  const handleCopy = async (name: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      message.success(`Copied "${name}" to clipboard`);
    } catch {
      message.error("Failed to copy");
    }
  };

  const handleExport = useCallback(
    async (names: string[]) => {
      if (names.length === 0) return;
      await exportFile("values", { value_names: names });
    },
    [exportFile],
  );

  const handleExportAll = useCallback(async () => {
    await exportFile("values", {});
  }, [exportFile]);

  const handleImportSuccess = useCallback(() => {
    fetchValues();
  }, [fetchValues]);

  const handleSelect = useCallback(
    (name: string, isMultiSelect: boolean) => {
      if (isMultiSelect) {
        setSelectedValues((prev) =>
          prev.includes(name)
            ? prev.filter((n) => n !== name)
            : [...prev, name],
        );
      } else {
        setSelectedValues([]);
        selectValue(name);
      }
    },
    [selectValue],
  );

  const getContextMenuItems = (
    name: string,
    value: string,
  ): MenuProps["items"] => {
    const isSelected = selectedValues.includes(name);
    const exportNames =
      isSelected && selectedValues.length > 0 ? selectedValues : [name];

    return [
      {
        key: "copy",
        icon: <CopyOutlined />,
        label: "Copy Value",
        onClick: () => handleCopy(name, value),
      },
      {
        key: "rename",
        icon: <EditOutlined />,
        label: "Rename",
        onClick: () => {
          setRenameTarget(name);
          setNewName(name);
          setRenameModalVisible(true);
        },
      },
      {
        type: "divider",
      },
      {
        key: "export",
        icon: <ExportOutlined />,
        label: `Export${exportNames.length > 1 ? ` (${exportNames.length})` : ""}`,
        onClick: () => handleExport(exportNames),
      },
      {
        type: "divider",
      },
      {
        key: "delete",
        icon: <DeleteOutlined />,
        label: "Delete",
        danger: true,
        onClick: () => handleDelete(name),
      },
    ];
  };

  return (
    <div className={styles.container} data-testid="values-list">
      <div className={styles.header}>
        <span className={styles.headerTitle}>Values</span>
        <div className={styles.headerActions}>
          <Tooltip title="New Value">
            <Button
              type="text"
              size="small"
              icon={<PlusOutlined />}
              onClick={() => setCreateModalVisible(true)}
              data-testid="value-new-button"
            />
          </Tooltip>
          <Tooltip title="Refresh">
            <Button
              type="text"
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => fetchValues()}
              data-testid="value-refresh-button"
            />
          </Tooltip>
          <ImportBifrostButton
            expectedType="values"
            onImportSuccess={handleImportSuccess}
            buttonText=""
            buttonType="text"
            size="small"
          />
          <Tooltip title="Export All">
            <Button
              type="text"
              size="small"
              icon={<ExportOutlined />}
              onClick={handleExportAll}
              data-testid="value-export-all-button"
            />
          </Tooltip>
        </div>
      </div>
      <div className={styles.searchBox}>
        <Input
          size="small"
          placeholder="Search values..."
          prefix={<SearchOutlined style={{ color: "#999" }} />}
          value={searchKeyword}
          onChange={(e) => setSearchKeyword(e.target.value)}
          allowClear
          data-testid="value-search-input"
        />
        <Select
          size="small"
          value={sortMode}
          onChange={(value: ValueSortMode) => setSortMode(value)}
          options={valueSortOptions}
          className={styles.sortControl}
          popupMatchSelectWidth={false}
          data-testid="value-sort-select"
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
              const hasChanges =
                hasUnsavedChanges(item.name) ||
                editingContent[item.name] !== undefined;

              return (
                <div
                  key={item.name}
                  className={`${styles.item} ${isSelected ? styles.selected : ""} ${selectedValues.includes(item.name) ? styles.multiSelected : ""}`}
                  onClick={(e) =>
                    handleSelect(item.name, e.ctrlKey || e.metaKey)
                  }
                  data-testid="value-item"
                  data-value-name={item.name}
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
                      <Dropdown
                        menu={{
                          items: getContextMenuItems(item.name, item.value),
                        }}
                        trigger={["click"]}
                      >
                        <Button
                          type="text"
                          size="small"
                          icon={<MoreOutlined />}
                          onClick={(e) => e.stopPropagation()}
                          className={styles.moreBtn}
                          data-testid="value-item-menu"
                        />
                      </Dropdown>
                    </div>
                  </div>
                  <div className={styles.itemPreview} title={item.value}>
                    {item.value.length > 30
                      ? `${item.value.slice(0, 30).replace(/\n/g, "↵")}...`
                      : item.value.replace(/\n/g, "↵")}
                  </div>
                </div>
              );
            })}
            {filteredValues.length === 0 && !loading && (
              <div className={styles.empty}>
                {searchKeyword ? "No matching values" : "No values yet"}
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
          setNewValueName("");
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
          setNewName("");
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
