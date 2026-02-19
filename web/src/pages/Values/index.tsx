import { useEffect, useState, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import {
  Table,
  Button,
  Space,
  Modal,
  Input,
  message,
  Popconfirm,
  Card,
  Row,
  Col,
  Alert,
  Tooltip,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  ReloadOutlined,
  CopyOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { useValuesStore } from '../../stores/useValuesStore';
import type { ValueItem } from '../../api/values';

const SEARCH_PARAM = 'q';

export default function Values() {
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    values,
    error,
    fetchValues,
    createValue,
    updateValue,
    deleteValue,
    clearError,
    searchText,
    setSearchText,
  } = useValuesStore();

  const [modalVisible, setModalVisible] = useState(false);
  const [editMode, setEditMode] = useState<'create' | 'edit'>('create');
  const [editName, setEditName] = useState('');
  const [editValue, setEditValue] = useState('');
  const [saving, setSaving] = useState(false);

  const initializedRef = useRef(false);
  const isUpdatingUrlRef = useRef(false);

  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;

    const searchFromUrl = searchParams.get(SEARCH_PARAM);
    if (searchFromUrl) {
      setSearchText(searchFromUrl);
    }

    fetchValues();
  }, [searchParams, fetchValues, setSearchText]);

  useEffect(() => {
    if (!initializedRef.current) return;
    if (isUpdatingUrlRef.current) {
      isUpdatingUrlRef.current = false;
      return;
    }

    isUpdatingUrlRef.current = true;
    setSearchParams(
      (prev) => {
        if (searchText) {
          prev.set(SEARCH_PARAM, searchText);
        } else {
          prev.delete(SEARCH_PARAM);
        }
        return prev;
      },
      { replace: true }
    );
  }, [searchText, setSearchParams]);

  useEffect(() => {
    if (error) {
      message.error(error);
      clearError();
    }
  }, [error, clearError]);

  const handleCreate = () => {
    setEditMode('create');
    setEditName('');
    setEditValue('');
    setModalVisible(true);
  };

  const handleEdit = (record: ValueItem) => {
    setEditMode('edit');
    setEditName(record.name);
    setEditValue(record.value);
    setModalVisible(true);
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      if (editMode === 'create') {
        if (!editName.trim()) {
          message.error('Value name is required');
          return;
        }
        const success = await createValue(editName.trim(), editValue);
        if (success) {
          message.success('Value created successfully');
          setModalVisible(false);
        }
      } else {
        const success = await updateValue(editName, editValue);
        if (success) {
          message.success('Value updated successfully');
          setModalVisible(false);
        }
      }
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (name: string) => {
    const success = await deleteValue(name);
    if (success) {
      message.success('Value deleted successfully');
    }
  };

  const handleCopy = (value: string) => {
    navigator.clipboard.writeText(value).then(() => {
      message.success('Copied to clipboard');
    });
  };

  const filteredValues = values.filter(
    (v) =>
      v.name.toLowerCase().includes(searchText.toLowerCase()) ||
      v.value.toLowerCase().includes(searchText.toLowerCase())
  );

  const columns: ColumnsType<ValueItem> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      width: 200,
      sorter: (a, b) => a.name.localeCompare(b.name),
      render: (name: string) => (
        <code style={{ fontFamily: 'monospace', fontSize: 13 }}>{name}</code>
      ),
    },
    {
      title: 'Value',
      dataIndex: 'value',
      key: 'value',
      ellipsis: true,
      render: (value: string) => {
        const displayValue = value.length > 100 ? `${value.slice(0, 100)}...` : value;
        return (
          <Tooltip title={value.length > 100 ? value : undefined}>
            <span
              style={{
                fontFamily: 'monospace',
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
              }}
            >
              {displayValue.replace(/\n/g, '\\n')}
            </span>
          </Tooltip>
        );
      },
    },
    {
      title: 'Actions',
      key: 'actions',
      width: 150,
      align: 'center',
      render: (_, record) => (
        <Space size="small">
          <Tooltip title="Copy">
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={() => handleCopy(record.value)}
            />
          </Tooltip>
          <Tooltip title="Edit">
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={() => handleEdit(record)}
            />
          </Tooltip>
          <Popconfirm
            title="Delete value"
            description={`Are you sure to delete "${record.name}"?`}
            onConfirm={() => handleDelete(record.name)}
            okText="Yes"
            cancelText="No"
          >
            <Tooltip title="Delete">
              <Button type="text" size="small" danger icon={<DeleteOutlined />} />
            </Tooltip>
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <div style={{ padding: 16 }}>
      <Row justify="space-between" align="middle" style={{ marginBottom: 16 }}>
        <Col>
          <Space>
            <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
              New Value
            </Button>
            <Button icon={<ReloadOutlined />} onClick={() => fetchValues()}>
              Refresh
            </Button>
          </Space>
        </Col>
        <Col>
          <Space>
            <Input.Search
              placeholder="Search values..."
              allowClear
              style={{ width: 200 }}
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
            />
            <span style={{ color: '#888' }}>{filteredValues.length} values</span>
          </Space>
        </Col>
      </Row>

      {error && (
        <Alert
          type="error"
          message={error}
          closable
          onClose={clearError}
          style={{ marginBottom: 16 }}
        />
      )}

      <Card bodyStyle={{ padding: 0 }}>
        <Table
          columns={columns}
          dataSource={filteredValues}
          rowKey="name"
          pagination={filteredValues.length > 20 ? { pageSize: 20 } : false}
          size="middle"
        />
      </Card>

      <Modal
        title={editMode === 'create' ? 'New Value' : `Edit: ${editName}`}
        open={modalVisible}
        onCancel={() => setModalVisible(false)}
        onOk={handleSave}
        width={600}
        okText="Save"
        cancelText="Cancel"
        confirmLoading={saving}
      >
        {editMode === 'create' && (
          <Input
            placeholder="Value name (e.g., api_key, auth_token)"
            value={editName}
            onChange={(e) => setEditName(e.target.value)}
            style={{ marginBottom: 16 }}
          />
        )}
        <Input.TextArea
          placeholder="Value content"
          value={editValue}
          onChange={(e) => setEditValue(e.target.value)}
          rows={8}
          style={{ fontFamily: 'monospace' }}
        />
        <div style={{ marginTop: 8, color: '#888', fontSize: 12 }}>
          Use <code>{'{name}'}</code> in rule files to reference this value.
        </div>
      </Modal>
    </div>
  );
}
