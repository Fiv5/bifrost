import { useMemo, useState, useCallback, useRef } from "react";
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
import { useAppModal } from "../../../hooks/useAppModal";
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

  const modal = useAppModal();
  const [createModalVisible, setCreateModalVisible] = useState(false);
  const [newValueName, setNewValueName] = useState("");
  const [renameModalVisible, setRenameModalVisible] = useState(false);
  const [renameTarget, setRenameTarget] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [selectedValues, setSelectedValues] = useState<string[]>([]);
  const lastClickedIndexRef = useRef<number | null>(null);
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
    modal.confirm({
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

  const handleBulkDelete = async (names: string[]) => {
    if (names.length === 0) return;
    if (names.length === 1) {
      handleDelete(names[0]);
      return;
    }
    modal.confirm({
      title: "Delete Values",
      content: `Are you sure to delete ${names.length} values?`,
      okText: "Delete",
      okType: "danger",
      cancelText: "Cancel",
      onOk: async () => {
        let successCount = 0;
        for (const name of names) {
          const success = await deleteValue(name);
          if (success) successCount++;
        }
        if (successCount > 0) {
          message.success(
            `${successCount} value${successCount > 1 ? "s" : ""} deleted`,
          );
          setSelectedValues([]);
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
    (name: string, e: React.MouseEvent) => {
      const isCtrl = e.ctrlKey || e.metaKey;
      const isShift = e.shiftKey;
      const currentIndex = filteredValues.findIndex((v) => v.name === name);

      if (isShift && lastClickedIndexRef.current !== null) {
        const start = Math.min(lastClickedIndexRef.current, currentIndex);
        const end = Math.max(lastClickedIndexRef.current, currentIndex);
        const rangeNames = filteredValues
          .slice(start, end + 1)
          .map((v) => v.name);
        setSelectedValues((prev) => {
          const combined = new Set([...prev, ...rangeNames]);
          return Array.from(combined);
        });
      } else if (isCtrl) {
        setSelectedValues((prev) =>
          prev.includes(name)
            ? prev.filter((n) => n !== name)
            : [...prev, name],
        );
        lastClickedIndexRef.current = currentIndex;
      } else {
        setSelectedValues([]);
        lastClickedIndexRef.current = currentIndex;
        selectValue(name);
      }
    },
    [selectValue, filteredValues],
  );

  const getContextMenuItems = (
    name: string,
    value: string,
  ): MenuProps["items"] => {
    const isSelected = selectedValues.includes(name);
    const bulkNames =
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
        label: `Export${bulkNames.length > 1 ? ` (${bulkNames.length})` : ""}`,
        onClick: () => handleExport(bulkNames),
      },
      {
        type: "divider",
      },
      {
        key: "delete",
        icon: <DeleteOutlined />,
        label: `Delete${bulkNames.length > 1 ? ` (${bulkNames.length})` : ""}`,
        danger: true,
        onClick: () => handleBulkDelete(bulkNames),
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
                <Dropdown
                  key={item.name}
                  menu={{
                    items: getContextMenuItems(item.name, item.value),
                  }}
                  trigger={["contextMenu"]}
                >
                  <div
                    className={`${styles.item} ${isSelected ? styles.selected : ""} ${selectedValues.includes(item.name) ? styles.multiSelected : ""}`}
                    onClick={(e) => handleSelect(item.name, e)}
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
                      </div>
                    </div>
                    <div className={styles.itemExtra}>
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
                        />
                      </Dropdown>
                    </div>
                  </div>
                </Dropdown>
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
