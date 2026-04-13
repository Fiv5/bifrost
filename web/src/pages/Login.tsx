import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { Alert, Button, Form, Input, Typography, message } from 'antd';

import { post } from '../api/client';
import { getAdminPrefix } from '../runtime';
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

type LoginErrorResponse = {
  error?: string;
  remaining_attempts?: number;
  failed_attempts?: number;
  max_attempts?: number;
  locked_out?: boolean;
};

export default function Login() {
  const [form] = Form.useForm();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [status, setStatus] = useState<AdminAuthStatus | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [loginError, setLoginError] = useState<LoginErrorResponse | null>(null);

  const nextPath = useMemo(() => {
    const raw = searchParams.get('next');
    if (!raw) {
      return '/traffic';
    }
    try {
      let decoded = decodeURIComponent(raw);
      const prefix = getAdminPrefix();
      if (prefix && decoded.startsWith(prefix)) {
        decoded = decoded.slice(prefix.length) || '/';
      }
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
        setStatus({ remote_access_enabled: false, auth_required: false, username: 'admin', has_password: false, locked_out: false, failed_attempts: 0, max_attempts: 5, min_password_length: 6 });
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
              Remote Management / Authentication
            </Typography.Text>
          </div>
        </div>

        {status && !status.remote_access_enabled ? (
          <Typography.Paragraph type="warning" style={{ marginTop: 12 }}>
            Remote access is not enabled. Please run `bifrost admin remote enable` on the server first and set a login password.
          </Typography.Paragraph>
        ) : null}

        {status?.locked_out ? (
          <Alert
            type="error"
            message="Account Locked"
            description="Remote access has been disabled and the password has been cleared due to multiple failed login attempts. Please reset the password and re-enable remote access on localhost."
            showIcon
            style={{ marginTop: 12 }}
          />
        ) : null}

        {loginError && !loginError.locked_out && loginError.error ? (
          <Alert
            type="warning"
            message={loginError.error}
            showIcon
            style={{ marginTop: 12 }}
          />
        ) : null}

        {loginError?.locked_out ? (
          <Alert
            type="error"
            message="Account Locked"
            description="Too many login attempts. Remote access has been disabled. Please reset the password on localhost."
            showIcon
            style={{ marginTop: 12 }}
          />
        ) : null}

        <Form
          form={form}
          layout="vertical"
          requiredMark={false}
          style={{ marginTop: 16 }}
          onFinish={async (values: { username: string; password: string }) => {
            setSubmitting(true);
            setLoginError(null);
            try {
              const res = await post<LoginResponse>('/auth/login', {
                username: values.username,
                password: values.password,
              });
              if (!res.token) {
                throw new Error('Missing token');
              }
              setAdminToken(res.token);
              message.success('Login successful');
              navigate(nextPath || '/traffic', { replace: true });
            } catch (err) {
              let errData: LoginErrorResponse = {};
              if (err && typeof err === 'object' && 'response' in err) {
                const axiosErr = err as { response?: { data?: LoginErrorResponse } };
                errData = axiosErr.response?.data ?? {};
              }
              setLoginError(errData);
              if (errData.locked_out) {
                void fetchAdminAuthStatus().then(setStatus).catch(() => {});
              } else {
                const errMsg = errData.error
                  || (err instanceof Error ? err.message : 'Login failed. Please check your username or password');
                message.error(errMsg);
              }
            } finally {
              setSubmitting(false);
            }
          }}
        >
          <Form.Item
            name="username"
            label="Username"
            rules={[{ required: true, message: 'Please enter username' }]}
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
            label="Password"
            rules={[{ required: true, message: 'Please enter password' }]}
          >
            <Input.Password
              size="large"
              autoComplete="current-password"
              placeholder="Enter password"
            />
          </Form.Item>
          <Button
            type="primary"
            htmlType="submit"
            size="large"
            block
            loading={submitting}
            disabled={!!status && (!status.remote_access_enabled || status.locked_out)}
          >
            Login
          </Button>

          <Typography.Paragraph type="secondary" style={{ marginTop: 12, marginBottom: 0 }}>
            Token lifetime is fixed at 7 days. To force logout all sessions, run `bifrost admin revoke-all` on the server.
          </Typography.Paragraph>
        </Form>
      </div>
    </div>
  );
}

