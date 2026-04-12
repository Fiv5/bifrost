import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Button, Form, Input, Typography, message } from 'antd';

import { post } from '../api/client';
import {
  clearAdminToken,
  fetchAdminAuthStatus,
  setAdminToken,
  type AdminAuthStatus,
} from '../services/adminAuth';

type LoginResponse = {
  token: string;
  expires_at: string;
  username: string;
};

export default function Login() {
  const [form] = Form.useForm();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [status, setStatus] = useState<AdminAuthStatus | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const nextPath = useMemo(() => {
    const raw = searchParams.get('next');
    if (!raw) {
      return '/traffic';
    }
    try {
      const decoded = decodeURIComponent(raw);
      return decoded.startsWith('/') ? decoded : '/traffic';
    } catch {
      return '/traffic';
    }
  }, [searchParams]);

  useEffect(() => {
    clearAdminToken();
    void fetchAdminAuthStatus()
      .then((s) => {
        setStatus(s);
        form.setFieldsValue({ username: s.username || 'admin' });
      })
      .catch(() => {
        setStatus({ remote_access_enabled: false, auth_required: false, username: 'admin', has_password: false });
        form.setFieldsValue({ username: 'admin' });
      });
  }, [form]);

  return (
    <div className="bifrost-login">
      <div className="bifrost-login__bg" aria-hidden="true" />
      <div className="bifrost-login__card">
        <div className="bifrost-login__brand">
          <div className="bifrost-login__logo" aria-hidden="true" />
          <div>
            <Typography.Title level={3} style={{ margin: 0 }}>
              Bifrost Admin
            </Typography.Title>
            <Typography.Text type="secondary">
              远程管理访问 / 鉴权登录
            </Typography.Text>
          </div>
        </div>

        {status && !status.remote_access_enabled ? (
          <Typography.Paragraph type="warning" style={{ marginTop: 12 }}>
            远程访问未开启。请先在服务器上执行 `bifrost admin remote enable`，并设置登录密码。
          </Typography.Paragraph>
        ) : null}

        <Form
          form={form}
          layout="vertical"
          requiredMark={false}
          style={{ marginTop: 16 }}
          onFinish={async (values: { username: string; password: string }) => {
            setSubmitting(true);
            try {
              const res = await post<LoginResponse>('/auth/login', {
                username: values.username,
                password: values.password,
              });
              if (!res.token) {
                throw new Error('Missing token');
              }
              setAdminToken(res.token);
              message.success('登录成功');
              navigate(nextPath || '/traffic', { replace: true });
            } catch (err) {
              message.error(
                err instanceof Error ? err.message : '登录失败，请检查用户名或密码',
              );
            } finally {
              setSubmitting(false);
            }
          }}
        >
          <Form.Item
            name="username"
            label="用户名"
            rules={[{ required: true, message: '请输入用户名' }]}
          >
            <Input
              size="large"
              autoComplete="username"
              placeholder="admin"
              spellCheck={false}
            />
          </Form.Item>
          <Form.Item
            name="password"
            label="密码"
            rules={[{ required: true, message: '请输入密码' }]}
          >
            <Input.Password
              size="large"
              autoComplete="current-password"
              placeholder="请输入密码"
            />
          </Form.Item>
          <Button
            type="primary"
            htmlType="submit"
            size="large"
            block
            loading={submitting}
            disabled={!!status && !status.remote_access_enabled}
          >
            登录
          </Button>

          <Typography.Paragraph type="secondary" style={{ marginTop: 12, marginBottom: 0 }}>
            Token 生命周期固定为 7 天；如需强制下线，请在服务器执行 `bifrost admin revoke-all`。
          </Typography.Paragraph>
        </Form>
      </div>
    </div>
  );
}

