import { useEffect, useState } from "react";
import {
  Row,
  Col,
  Card,
  Input,
  Select,
  Button,
  Space,
  Drawer,
  Descriptions,
  Typography,
  Tabs,
  message,
  Popconfirm,
} from "antd";
import {
  ReloadOutlined,
  ClearOutlined,
  SearchOutlined,
  ThunderboltOutlined,
} from "@ant-design/icons";
import { useTrafficStore } from "../../stores/useTrafficStore";
import TrafficTable from "../../components/TrafficTable";
import type { TrafficSummary, MatchedRule } from "../../types";
import dayjs from "dayjs";

const { Text, Paragraph } = Typography;

const methodOptions = [
  { value: "", label: "All Methods" },
  { value: "GET", label: "GET" },
  { value: "POST", label: "POST" },
  { value: "PUT", label: "PUT" },
  { value: "DELETE", label: "DELETE" },
  { value: "PATCH", label: "PATCH" },
  { value: "OPTIONS", label: "OPTIONS" },
];

const statusOptions = [
  { value: "", label: "All Status" },
  { value: "2xx", label: "2xx Success" },
  { value: "3xx", label: "3xx Redirect" },
  { value: "4xx", label: "4xx Client Error" },
  { value: "5xx", label: "5xx Server Error" },
];

const rulesOptions = [
  { value: "", label: "All Requests" },
  { value: "true", label: "With Rules" },
  { value: "false", label: "Without Rules" },
];

export default function Traffic() {
  const {
    records,
    currentRecord,
    total,
    loading,
    fetchTraffic,
    fetchTrafficDetail,
    clearTraffic,
    setFilter,
  } = useTrafficStore();

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string>();
  const [urlSearch, setUrlSearch] = useState("");

  useEffect(() => {
    fetchTraffic();
    const interval = setInterval(fetchTraffic, 1000);
    return () => clearInterval(interval);
  }, [fetchTraffic]);

  const handleSelect = async (record: TrafficSummary) => {
    setSelectedId(record.id);
    await fetchTrafficDetail(record.id);
    setDrawerOpen(true);
  };

  const handleSearch = () => {
    setFilter({ url_contains: urlSearch || undefined, offset: 0 });
    fetchTraffic();
  };

  const handleMethodChange = (value: string) => {
    setFilter({ method: value || undefined, offset: 0 });
    fetchTraffic();
  };

  const handleStatusChange = (value: string) => {
    let statusMin: number | undefined;
    let statusMax: number | undefined;
    if (value === "2xx") {
      statusMin = 200;
      statusMax = 299;
    } else if (value === "3xx") {
      statusMin = 300;
      statusMax = 399;
    } else if (value === "4xx") {
      statusMin = 400;
      statusMax = 499;
    } else if (value === "5xx") {
      statusMin = 500;
      statusMax = 599;
    }
    setFilter({ status_min: statusMin, status_max: statusMax, offset: 0 });
    fetchTraffic();
  };

  const handleRulesChange = (value: string) => {
    setFilter({
      has_rules: value === "" ? undefined : value === "true",
      offset: 0,
    });
    fetchTraffic();
  };

  const handleClear = async () => {
    const success = await clearTraffic();
    if (success) {
      message.success("Traffic cleared");
      setDrawerOpen(false);
    }
  };

  const formatHeaders = (headers: [string, string][] | null) => {
    if (!headers || headers.length === 0)
      return <Text type="secondary">No headers</Text>;
    return (
      <div style={{ fontFamily: "monospace", fontSize: 12 }}>
        {headers.map(([key, value], i) => (
          <div key={i}>
            <Text strong>{key}:</Text> {value}
          </div>
        ))}
      </div>
    );
  };

  const formatBody = (body: string | null, contentType: string | null) => {
    if (!body) return <Text type="secondary">No body</Text>;

    const isJson = contentType?.includes("json");
    if (isJson) {
      try {
        const formatted = JSON.stringify(JSON.parse(body), null, 2);
        return (
          <pre
            style={{
              margin: 0,
              fontSize: 12,
              overflow: "auto",
              maxHeight: 400,
            }}
          >
            {formatted}
          </pre>
        );
      } catch {
        // not valid json
      }
    }

    return (
      <Paragraph
        style={{
          fontFamily: "monospace",
          fontSize: 12,
          whiteSpace: "pre-wrap",
        }}
        ellipsis={{ rows: 20, expandable: true }}
      >
        {body}
      </Paragraph>
    );
  };

  const formatMatchedRules = (rules: MatchedRule[] | null) => {
    if (!rules || rules.length === 0)
      return <Text type="secondary">No rules matched</Text>;
    return (
      <div style={{ fontFamily: "monospace", fontSize: 12 }}>
        {rules.map((rule, i) => (
          <div
            key={i}
            style={{
              padding: "8px 12px",
              marginBottom: 8,
              background: "#f5f5f5",
              borderRadius: 4,
              border: "1px solid #e8e8e8",
            }}
          >
            <div style={{ marginBottom: 4 }}>
              <ThunderboltOutlined style={{ color: "#1890ff", marginRight: 8 }} />
              <Text strong style={{ color: "#1890ff" }}>
                {rule.protocol}
              </Text>
            </div>
            <div style={{ marginBottom: 4 }}>
              <Text type="secondary">Pattern: </Text>
              <Text code>{rule.pattern}</Text>
            </div>
            <div>
              <Text type="secondary">Value: </Text>
              <Text>{rule.value || <Text type="secondary">(empty)</Text>}</Text>
            </div>
          </div>
        ))}
      </div>
    );
  };

  return (
    <div>
      <Row gutter={[16, 16]} style={{ marginBottom: 16 }}>
        <Col flex="auto">
          <Space>
            <Input
              placeholder="Search URL..."
              prefix={<SearchOutlined />}
              value={urlSearch}
              onChange={(e) => setUrlSearch(e.target.value)}
              onPressEnter={handleSearch}
              style={{ width: 300 }}
              allowClear
            />
            <Select
              placeholder="Method"
              options={methodOptions}
              onChange={handleMethodChange}
              style={{ width: 120 }}
              defaultValue=""
            />
            <Select
              placeholder="Status"
              options={statusOptions}
              onChange={handleStatusChange}
              style={{ width: 140 }}
              defaultValue=""
            />
            <Select
              placeholder="Rules"
              options={rulesOptions}
              onChange={handleRulesChange}
              style={{ width: 140 }}
              defaultValue=""
            />
            <Button icon={<ReloadOutlined />} onClick={() => fetchTraffic()}>
              Refresh
            </Button>
          </Space>
        </Col>
        <Col>
          <Space>
            <Text type="secondary">{total} records</Text>
            <Popconfirm
              title="Clear all traffic?"
              onConfirm={handleClear}
              okText="Yes"
              cancelText="No"
            >
              <Button danger icon={<ClearOutlined />}>
                Clear
              </Button>
            </Popconfirm>
          </Space>
        </Col>
      </Row>

      <Card bodyStyle={{ padding: 0 }}>
        <TrafficTable
          data={records}
          loading={loading}
          onSelect={handleSelect}
          selectedId={selectedId}
        />
      </Card>

      <Drawer
        title={currentRecord?.url || "Traffic Detail"}
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        width={600}
      >
        {currentRecord && (
          <div>
            <Descriptions column={2} size="small" bordered>
              <Descriptions.Item label="Method">
                {currentRecord.method}
              </Descriptions.Item>
              <Descriptions.Item label="Status">
                {currentRecord.status || "-"}
              </Descriptions.Item>
              <Descriptions.Item label="Host" span={2}>
                {currentRecord.host}
              </Descriptions.Item>
              <Descriptions.Item label="Path" span={2}>
                {currentRecord.path}
              </Descriptions.Item>
              <Descriptions.Item label="Time">
                {dayjs(currentRecord.timestamp).format("YYYY-MM-DD HH:mm:ss")}
              </Descriptions.Item>
              <Descriptions.Item label="Duration">
                {currentRecord.duration_ms}ms
              </Descriptions.Item>
              <Descriptions.Item label="Request Size">
                {currentRecord.request_size} bytes
              </Descriptions.Item>
              <Descriptions.Item label="Response Size">
                {currentRecord.response_size} bytes
              </Descriptions.Item>
            </Descriptions>

            <Tabs
              style={{ marginTop: 16 }}
              items={[
                {
                  key: "matchedRules",
                  label: (
                    <span>
                      <ThunderboltOutlined style={{ marginRight: 4 }} />
                      Matched Rules
                      {currentRecord.matched_rules &&
                        currentRecord.matched_rules.length > 0 && (
                          <span
                            style={{
                              marginLeft: 4,
                              padding: "0 6px",
                              fontSize: 11,
                              background: "#1890ff",
                              color: "#fff",
                              borderRadius: 10,
                            }}
                          >
                            {currentRecord.matched_rules.length}
                          </span>
                        )}
                    </span>
                  ),
                  children: formatMatchedRules(currentRecord.matched_rules),
                },
                {
                  key: "reqHeaders",
                  label: "Request Headers",
                  children: formatHeaders(currentRecord.request_headers),
                },
                {
                  key: "reqBody",
                  label: "Request Body",
                  children: formatBody(
                    currentRecord.request_body,
                    currentRecord.content_type,
                  ),
                },
                {
                  key: "resHeaders",
                  label: "Response Headers",
                  children: formatHeaders(currentRecord.response_headers),
                },
                {
                  key: "resBody",
                  label: "Response Body",
                  children: formatBody(
                    currentRecord.response_body,
                    currentRecord.content_type,
                  ),
                },
              ]}
            />
          </div>
        )}
      </Drawer>
    </div>
  );
}
