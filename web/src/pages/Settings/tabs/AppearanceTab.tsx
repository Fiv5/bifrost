import { Card, Col, Row, Segmented, Space, Typography } from "antd";
import { BgColorsOutlined } from "@ant-design/icons";
import type { ThemeMode } from "../../../stores/useThemeStore";

const { Text } = Typography;

export interface AppearanceTabProps {
  themeMode: ThemeMode;
  setThemeMode: (mode: ThemeMode) => void;
}

export default function AppearanceTab({
  themeMode,
  setThemeMode,
}: AppearanceTabProps) {
  return (
    <Row gutter={[16, 16]}>
      <Col xs={24}>
        <Card
          title={
            <Space>
              <BgColorsOutlined />
              <span>Theme</span>
            </Space>
          }
          size="small"
        >
          <Space direction="vertical" style={{ width: "100%" }}>
            <Row justify="space-between" align="middle">
              <Col>
                <Text>Color Mode</Text>
              </Col>
              <Col>
                <Segmented
                  value={themeMode}
                  onChange={(value) => setThemeMode(value as ThemeMode)}
                  options={[
                    { label: "Light", value: "light" },
                    { label: "Dark", value: "dark" },
                    { label: "System", value: "system" },
                  ]}
                />
              </Col>
            </Row>
            <Text type="secondary" style={{ fontSize: 12 }}>
              Choose your preferred color theme. System mode will automatically
              follow your operating system settings.
            </Text>
          </Space>
        </Card>
      </Col>
    </Row>
  );
}
