import { Card, Col, Row, Space, Typography, Tag, Button, Image, List, Divider } from "antd";
import {
  CheckOutlined,
  CloseOutlined,
  DownloadOutlined,
  GlobalOutlined,
  QrcodeOutlined,
  SafetyCertificateOutlined,
} from "@ant-design/icons";
import type { CertInfo } from "../../../api/cert";

const { Text } = Typography;

export interface CertificateTabProps {
  certInfo: CertInfo | null;
  selectedProxyIp: string;
  getCertDownloadUrl: () => string;
  getCertQRCodeUrl: (ip?: string) => string;
}

export default function CertificateTab({
  certInfo,
  selectedProxyIp,
  getCertDownloadUrl,
  getCertQRCodeUrl,
}: CertificateTabProps) {
  const certStatus = certInfo?.status ?? "unknown";
  const certStatusLabel = certInfo?.status_label ?? "Check failed";
  const certStatusColor =
    certStatus === "installed_and_trusted"
      ? "green"
      : certStatus === "installed_not_trusted"
        ? "orange"
        : certStatus === "not_installed"
          ? "red"
          : "default";
  const certStatusIcon =
    certStatus === "installed_and_trusted" ? <CheckOutlined /> : <CloseOutlined />;

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24}>
        <Card
          title={
            <Space>
              <SafetyCertificateOutlined />
              <span>CA Certificate</span>
            </Space>
          }
          size="small"
        >
          <Space direction="vertical" style={{ width: "100%" }} size="middle">
            <Row justify="space-between" align="middle">
              <Col>
                <Text>Certificate Status</Text>
              </Col>
              <Col>
                <Tag color={certStatusColor} icon={certStatusIcon}>
                  {certStatusLabel}
                </Tag>
              </Col>
            </Row>

            <Divider style={{ margin: "8px 0" }} />

            <Text type="secondary" style={{ fontSize: 12 }}>
              {certInfo?.status_message ??
                "Unable to verify whether the CA certificate is installed and trusted."}
            </Text>

            <Button
              type="primary"
              icon={<DownloadOutlined />}
              href={getCertDownloadUrl()}
              download="bifrost-ca.crt"
              disabled={!certInfo?.available}
              block
            >
              Download CA Certificate
            </Button>

            <Text type="secondary" style={{ fontSize: 12 }}>
              {certInfo?.available
                ? "Download the CA certificate file and install it as a trusted root CA on your device to enable HTTPS inspection."
                : "The CA certificate file is not available yet, so it cannot be downloaded or trusted on this device."}
            </Text>
          </Space>
        </Card>
      </Col>

      <Col xs={24}>
        <Card
          title={
            <Space>
              <QrcodeOutlined />
              <span>Mobile Installation</span>
            </Space>
          }
          size="small"
        >
          <Space
            direction="vertical"
            style={{ width: "100%", alignItems: "center" }}
            size="middle"
          >
            {certInfo?.available ? (
              <>
                <Image
                  src={getCertQRCodeUrl(selectedProxyIp || undefined)}
                  alt="Certificate QR Code"
                  width={180}
                  height={180}
                  fallback="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mN8/+F9PQAJpAN4pokyXwAAAABJRU5ErkJggg=="
                />
                <Text
                  type="secondary"
                  style={{ fontSize: 12, textAlign: "center" }}
                >
                  Scan with your mobile device to download and install the CA
                  certificate
                  {selectedProxyIp && (
                    <>
                      <br />
                      <Text code style={{ fontSize: 11 }}>
                        {selectedProxyIp}
                      </Text>
                    </>
                  )}
                </Text>
              </>
            ) : (
              <Text type="secondary">QR code not available</Text>
            )}
          </Space>
        </Card>
      </Col>

      <Col xs={24}>
        <Card
          title={
            <Space>
              <GlobalOutlined />
              <span>Available Download URLs</span>
            </Space>
          }
          size="small"
        >
          {certInfo?.download_urls && certInfo.download_urls.length > 0 ? (
            <List
              size="small"
              dataSource={certInfo.download_urls}
              renderItem={(url) => (
                <List.Item>
                  <a href={url} target="_blank" rel="noreferrer">
                    {url}
                  </a>
                </List.Item>
              )}
            />
          ) : (
            <Text type="secondary">No download URLs available</Text>
          )}
        </Card>
      </Col>
    </Row>
  );
}
