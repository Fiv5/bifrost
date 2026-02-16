import { useEffect, useState } from 'react';
import {
  Table,
  Button,
  Switch,
  Space,
  Modal,
  Input,
  message,
  Popconfirm,
  Card,
  Row,
  Col,
  Spin,
  Alert,
  Tooltip,
} from 'antd';
import {
  PlusOutlined,
  EditOutlined,
  DeleteOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';
import { useRulesStore } from '../../stores/useRulesStore';
import RuleEditor from '../../components/RuleEditor';
import type { RuleFile } from '../../types';

export default function Rules() {
  const {
    rules,
    currentRule,
    loading,
    error,
    fetchRules,
    fetchRule,
    createRule,
    updateRule,
    deleteRule,
    toggleRule,
    clearError,
  } = useRulesStore();

  const [modalVisible, setModalVisible] = useState(false);
  const [editMode, setEditMode] = useState<'create' | 'edit'>('create');
  const [editName, setEditName] = useState('');
  const [editContent, setEditContent] = useState('');

  useEffect(() => {
    fetchRules();
  }, [fetchRules]);

  useEffect(() => {
    if (error) {
      message.error(error);
      clearError();
    }
  }, [error, clearError]);

  const handleCreate = () => {
    setEditMode('create');
    setEditName('');
    setEditContent('# Rule file\n# pattern protocol://value\n');
    setModalVisible(true);
  };

  const handleEdit = async (name: string) => {
    await fetchRule(name);
    setEditMode('edit');
    setEditName(name);
    setModalVisible(true);
  };

  useEffect(() => {
    if (currentRule && editMode === 'edit') {
      setEditContent(currentRule.content);
    }
  }, [currentRule, editMode]);

  const handleSave = async () => {
    if (editMode === 'create') {
      if (!editName.trim()) {
        message.error('Rule name is required');
        return;
      }
      const success = await createRule(editName.trim(), editContent);
      if (success) {
        message.success('Rule created successfully');
        setModalVisible(false);
      }
    } else {
      const success = await updateRule(editName, editContent);
      if (success) {
        message.success('Rule updated successfully');
        setModalVisible(false);
      }
    }
  };

  const handleDelete = async (name: string) => {
    const success = await deleteRule(name);
    if (success) {
      message.success('Rule deleted successfully');
    }
  };

  const handleToggle = async (name: string, enabled: boolean) => {
    const success = await toggleRule(name, enabled);
    if (success) {
      message.success(`Rule ${enabled ? 'enabled' : 'disabled'}`);
    }
  };

  const columns: ColumnsType<RuleFile> = [
    {
      title: 'Name',
      dataIndex: 'name',
      key: 'name',
      sorter: (a, b) => a.name.localeCompare(b.name),
    },
    {
      title: 'Rules',
      dataIndex: 'rule_count',
      key: 'rule_count',
      width: 100,
      align: 'center',
    },
    {
      title: 'Enabled',
      dataIndex: 'enabled',
      key: 'enabled',
      width: 100,
      align: 'center',
      render: (enabled: boolean, record) => (
        <Switch
          checked={enabled}
          onChange={(checked) => handleToggle(record.name, checked)}
          size="small"
        />
      ),
    },
    {
      title: 'Actions',
      key: 'actions',
      width: 150,
      align: 'center',
      render: (_, record) => (
        <Space size="small">
          <Tooltip title="Edit">
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={() => handleEdit(record.name)}
            />
          </Tooltip>
          <Popconfirm
            title="Delete rule"
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

  if (loading && rules.length === 0) {
    return <Spin size="large" style={{ display: 'block', margin: '100px auto' }} />;
  }

  return (
    <div style={{ padding: 16 }}>
      <Row justify="space-between" align="middle" style={{ marginBottom: 16 }}>
        <Col>
          <Space>
            <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>
              New Rule
            </Button>
            <Button icon={<ReloadOutlined />} onClick={() => fetchRules()}>
              Refresh
            </Button>
          </Space>
        </Col>
        <Col>
          <span style={{ color: '#888' }}>
            {rules.length} rules, {rules.filter((r) => r.enabled).length} enabled
          </span>
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
          dataSource={rules}
          rowKey="name"
          loading={loading}
          pagination={false}
          size="middle"
        />
      </Card>

      <Modal
        title={editMode === 'create' ? 'New Rule' : `Edit: ${editName}`}
        open={modalVisible}
        onCancel={() => setModalVisible(false)}
        onOk={handleSave}
        width={800}
        okText="Save"
        cancelText="Cancel"
        confirmLoading={loading}
      >
        {editMode === 'create' && (
          <Input
            placeholder="Rule name"
            value={editName}
            onChange={(e) => setEditName(e.target.value)}
            style={{ marginBottom: 16 }}
          />
        )}
        <RuleEditor value={editContent} onChange={setEditContent} height="400px" />
      </Modal>
    </div>
  );
}
