import { Select, Input, Button, Space } from "antd";
import { PlusOutlined, DeleteOutlined } from "@ant-design/icons";
import type { FilterCondition } from "../../types";

interface FilterBarProps {
  filters: FilterCondition[];
  onFiltersChange: (filters: FilterCondition[]) => void;
}

const fieldOptions = [
  { value: "url", label: "URL" },
  { value: "host", label: "Host" },
  { value: "path", label: "Path" },
  { value: "method", label: "Method" },
  { value: "request_header", label: "Request Header" },
  { value: "response_header", label: "Response Header" },
  { value: "request_body", label: "Request Body" },
  { value: "response_body", label: "Response Body" },
  { value: "content_type", label: "Content-Type" },
];

const operatorOptions = [
  { value: "contains", label: "Contains" },
  { value: "equals", label: "Equals" },
  { value: "regex", label: "Regex" },
  { value: "not_contains", label: "Not Contains" },
];

const styles = {
  container: {
    display: "flex",
    flexDirection: "column" as const,
    gap: 8,
  },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 8,
  },
  fieldSelect: {
    width: 140,
  },
  operatorSelect: {
    width: 110,
  },
  valueInput: {
    flex: 1,
    minWidth: 150,
  },
};

export default function FilterBar({
  filters,
  onFiltersChange,
}: FilterBarProps) {
  const generateId = () =>
    `filter_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;

  const handleAdd = () => {
    const newFilter: FilterCondition = {
      id: generateId(),
      field: "url",
      operator: "contains",
      value: "",
    };
    onFiltersChange([...filters, newFilter]);
  };

  const handleRemove = (id: string) => {
    onFiltersChange(filters.filter((f) => f.id !== id));
  };

  const handleChange = (
    id: string,
    key: keyof FilterCondition,
    value: string,
  ) => {
    onFiltersChange(
      filters.map((f) => (f.id === id ? { ...f, [key]: value } : f)),
    );
  };

  return (
    <div style={styles.container}>
      {filters.map((filter, index) => (
        <div key={filter.id} style={styles.row}>
          <Select
            value={filter.field}
            options={fieldOptions}
            onChange={(value) => handleChange(filter.id, "field", value)}
            style={styles.fieldSelect}
            placeholder="Field"
            size="small"
          />
          <Select
            value={filter.operator}
            options={operatorOptions}
            onChange={(value) => handleChange(filter.id, "operator", value)}
            style={styles.operatorSelect}
            placeholder="Operator"
            size="small"
          />
          <Input
            value={filter.value}
            onChange={(e) => handleChange(filter.id, "value", e.target.value)}
            style={styles.valueInput}
            placeholder="Enter value..."
            size="small"
          />
          <Space size={4}>
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={() => handleRemove(filter.id)}
            />
            {index === filters.length - 1 && (
              <Button
                type="primary"
                size="small"
                icon={<PlusOutlined />}
                onClick={handleAdd}
              />
            )}
          </Space>
        </div>
      ))}
      {filters.length === 0 && (
        <Button
          type="dashed"
          size="small"
          icon={<PlusOutlined />}
          onClick={handleAdd}
          style={{ width: "100%" }}
        >
          Add Filter
        </Button>
      )}
    </div>
  );
}
