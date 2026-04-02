import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import { sendJson, sendUnauthorized, requireAuth, parseJsonBody, sendRateLimited, setHtmlSecurityHeaders } from '../http';
import { nanoid } from 'nanoid';
import type { AccountLockManager } from '../security';
import { validateUsername, validatePassword } from '../security';

export async function handleSso(
  ctx: RequestContext,
  storage: IStorage,
  accountLock: AccountLockManager,
): Promise<boolean> {
  const { url, req } = ctx;
  const method = req.method ?? 'GET';

  if (url.pathname === '/v4/sso/check') {
    return handleCheck(ctx, storage);
  }
  if (url.pathname === '/v4/sso/info') {
    return handleInfo(ctx, storage);
  }
  if (url.pathname === '/v4/sso/login' && method === 'GET') {
    return handleLoginPage(ctx, storage);
  }
  if (url.pathname === '/v4/sso/login' && method === 'POST') {
    return handleLogin(ctx, storage, accountLock);
  }
  if (url.pathname === '/v4/sso/logout') {
    return handleLogout(ctx, storage);
  }
  if (url.pathname === '/v4/sso/register' && method === 'POST') {
    return handleRegister(ctx, storage);
  }
  if (url.pathname === '/v4/sso/register-page' && method === 'GET') {
    return handleRegisterPage(ctx);
  }

  return false;
}

async function handleCheck(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const token =
    (ctx.req.headers['x-bifrost-token'] as string | undefined) ??
    ctx.url.searchParams.get('token') ??
    undefined;
  if (!token) {
    sendUnauthorized(ctx.res);
    return true;
  }
  const user = await storage.user.findByToken(token);
  if (!user) {
    sendUnauthorized(ctx.res);
    return true;
  }
  sendJson(ctx.res, 200, {
    code: 0,
    message: 'authorized',
    data: { user_id: user.user_id, token },
  });
  return true;
}

async function handleInfo(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  if (!(await requireAuth(ctx, storage))) return true;
  const user = ctx.user!;
  sendJson(ctx.res, 200, {
    code: 0,
    message: 'success',
    data: {
      user_id: user.user_id,
      nickname: user.nickname,
      avatar: user.avatar,
      email: user.email,
    },
  });
  return true;
}

async function handleLoginPage(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const next = ctx.url.searchParams.get('next') ?? '/v4/sso/check';

  const token = ctx.req.headers['x-bifrost-token'] as string | undefined;
  if (token) {
    const user = await storage.user.findByToken(token);
    if (user) {
      const separator = next.includes('?') ? '&' : '?';
      ctx.res.writeHead(302, { Location: `${next}${separator}token=${encodeURIComponent(token)}` });
      ctx.res.end();
      return true;
    }
  }

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Bifrost Sync - Login</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f5f5f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; }
    .card { background: #fff; border-radius: 12px; box-shadow: 0 2px 12px rgba(0,0,0,0.08); padding: 40px; width: 100%; max-width: 400px; }
    .card h1 { font-size: 24px; margin-bottom: 8px; color: #1a1a1a; }
    .card p { color: #666; margin-bottom: 24px; font-size: 14px; }
    .field { margin-bottom: 16px; }
    .field label { display: block; margin-bottom: 6px; font-size: 14px; font-weight: 500; color: #333; }
    .field input { width: 100%; padding: 10px 12px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; transition: border-color 0.2s; }
    .field input:focus { outline: none; border-color: #4f8ff7; }
    .btn { width: 100%; padding: 12px; background: #4f8ff7; color: #fff; border: none; border-radius: 8px; font-size: 15px; font-weight: 500; cursor: pointer; transition: background 0.2s; }
    .btn:hover { background: #3a7de8; }
    .btn:disabled { background: #a0c4fd; cursor: not-allowed; }
    .error { color: #e53e3e; font-size: 13px; margin-top: 12px; display: none; }
    .footer { text-align: center; margin-top: 20px; font-size: 13px; color: #999; }
    .footer a { color: #4f8ff7; text-decoration: none; }
  </style>
</head>
<body>
  <div class="card">
    <h1>Bifrost Sync</h1>
    <p>Sign in to sync your proxy rules</p>
    <form id="form">
      <div class="field">
        <label for="user_id">Username</label>
        <input type="text" id="user_id" name="user_id" required autocomplete="username" maxlength="64" />
      </div>
      <div class="field">
        <label for="password">Password</label>
        <input type="password" id="password" name="password" required autocomplete="current-password" maxlength="128" />
      </div>
      <button type="submit" class="btn" id="submit">Sign In</button>
      <div class="error" id="error"></div>
    </form>
    <div class="footer">
      Don't have an account? <a href="/v4/sso/register-page">Register</a>
    </div>
  </div>
  <script>
    const form = document.getElementById('form');
    const errEl = document.getElementById('error');
    const btn = document.getElementById('submit');
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      errEl.style.display = 'none';
      btn.disabled = true;
      btn.textContent = 'Signing in...';
      try {
        const res = await fetch('/v4/sso/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            user_id: document.getElementById('user_id').value,
            password: document.getElementById('password').value,
          }),
        });
        const data = await res.json();
        if (data.code === 0 && data.data && data.data.token) {
          const next = ${JSON.stringify(next)};
          const sep = next.includes('?') ? '&' : '?';
          window.location.href = next + sep + 'token=' + encodeURIComponent(data.data.token);
        } else {
          errEl.textContent = data.message || 'Login failed';
          errEl.style.display = 'block';
        }
      } catch (err) {
        errEl.textContent = 'Network error';
        errEl.style.display = 'block';
      } finally {
        btn.disabled = false;
        btn.textContent = 'Sign In';
      }
    });
  </script>
</body>
</html>`;
  setHtmlSecurityHeaders(ctx.res);
  ctx.res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  ctx.res.end(html);
  return true;
}

interface LoginBody {
  user_id: string;
  password: string;
}

async function handleLogin(
  ctx: RequestContext,
  storage: IStorage,
  accountLock: AccountLockManager,
): Promise<boolean> {
  const body = parseJsonBody<LoginBody>(ctx.body);
  if (!body?.user_id || !body?.password) {
    sendJson(ctx.res, 400, { code: -1, message: 'user_id and password are required' });
    return true;
  }

  const lockStatus = accountLock.isLocked(body.user_id);
  if (lockStatus.locked) {
    sendRateLimited(ctx.res, lockStatus.retryAfterMs);
    return true;
  }

  const valid = await storage.user.verifyPassword(body.user_id, body.password);
  if (!valid) {
    const result = accountLock.recordFailure(body.user_id);
    if (result.locked) {
      sendJson(ctx.res, 423, {
        code: -1,
        message: 'account temporarily locked due to too many failed attempts, try again later',
      });
    } else {
      sendJson(ctx.res, 401, { code: -1, message: 'invalid user_id or password' });
    }
    return true;
  }

  accountLock.recordSuccess(body.user_id);

  const token = nanoid(32);
  await storage.user.saveToken(body.user_id, token);
  const user = await storage.user.findByUserId(body.user_id);

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: {
      user_id: user!.user_id,
      nickname: user!.nickname,
      avatar: user!.avatar,
      email: user!.email,
      token,
    },
  });
  return true;
}

async function handleLogout(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const token = ctx.req.headers['x-bifrost-token'] as string | undefined;
  if (token) {
    const user = await storage.user.findByToken(token);
    if (user) {
      await storage.user.clearToken(user.user_id);
    }
  }

  const next = ctx.url.searchParams.get('next');
  if (next) {
    ctx.res.writeHead(302, { Location: next });
    ctx.res.end();
    return true;
  }

  sendJson(ctx.res, 200, { code: 0, message: 'ok', data: 1 });
  return true;
}

async function handleRegisterPage(ctx: RequestContext): Promise<boolean> {
  const html = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Bifrost Sync - Register</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f5f5f5; display: flex; align-items: center; justify-content: center; min-height: 100vh; }
    .card { background: #fff; border-radius: 12px; box-shadow: 0 2px 12px rgba(0,0,0,0.08); padding: 40px; width: 100%; max-width: 400px; }
    .card h1 { font-size: 24px; margin-bottom: 8px; color: #1a1a1a; }
    .card p { color: #666; margin-bottom: 24px; font-size: 14px; }
    .field { margin-bottom: 16px; }
    .field label { display: block; margin-bottom: 6px; font-size: 14px; font-weight: 500; color: #333; }
    .field input { width: 100%; padding: 10px 12px; border: 1px solid #ddd; border-radius: 8px; font-size: 14px; transition: border-color 0.2s; }
    .field input:focus { outline: none; border-color: #4f8ff7; }
    .hint { font-size: 12px; color: #999; margin-top: 4px; }
    .btn { width: 100%; padding: 12px; background: #4f8ff7; color: #fff; border: none; border-radius: 8px; font-size: 15px; font-weight: 500; cursor: pointer; transition: background 0.2s; }
    .btn:hover { background: #3a7de8; }
    .btn:disabled { background: #a0c4fd; cursor: not-allowed; }
    .error { color: #e53e3e; font-size: 13px; margin-top: 12px; display: none; }
    .success { color: #38a169; font-size: 13px; margin-top: 12px; display: none; }
    .footer { text-align: center; margin-top: 20px; font-size: 13px; color: #999; }
    .footer a { color: #4f8ff7; text-decoration: none; }
  </style>
</head>
<body>
  <div class="card">
    <h1>Bifrost Sync</h1>
    <p>Create a new account</p>
    <form id="form">
      <div class="field">
        <label for="user_id">Username</label>
        <input type="text" id="user_id" name="user_id" required autocomplete="username" minlength="2" maxlength="64" pattern="[a-zA-Z0-9_\\-@.]+" />
        <div class="hint">2-64 characters, letters, numbers, _ - @ .</div>
      </div>
      <div class="field">
        <label for="password">Password</label>
        <input type="password" id="password" name="password" required autocomplete="new-password" minlength="6" maxlength="128" />
        <div class="hint">At least 6 characters</div>
      </div>
      <div class="field">
        <label for="nickname">Nickname (optional)</label>
        <input type="text" id="nickname" name="nickname" autocomplete="name" maxlength="64" />
      </div>
      <div class="field">
        <label for="email">Email (optional)</label>
        <input type="email" id="email" name="email" autocomplete="email" maxlength="128" />
      </div>
      <button type="submit" class="btn" id="submit">Register</button>
      <div class="error" id="error"></div>
      <div class="success" id="success"></div>
    </form>
    <div class="footer">
      Already have an account? <a href="/v4/sso/login">Sign In</a>
    </div>
  </div>
  <script>
    const form = document.getElementById('form');
    const errEl = document.getElementById('error');
    const successEl = document.getElementById('success');
    const btn = document.getElementById('submit');
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      errEl.style.display = 'none';
      successEl.style.display = 'none';
      btn.disabled = true;
      btn.textContent = 'Registering...';
      try {
        const body = {
          user_id: document.getElementById('user_id').value,
          password: document.getElementById('password').value,
        };
        const nickname = document.getElementById('nickname').value;
        const email = document.getElementById('email').value;
        if (nickname) body.nickname = nickname;
        if (email) body.email = email;
        const res = await fetch('/v4/sso/register', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
        const data = await res.json();
        if (data.code === 0 && data.data && data.data.token) {
          successEl.textContent = 'Registration successful! Redirecting to login...';
          successEl.style.display = 'block';
          setTimeout(() => { window.location.href = '/v4/sso/login'; }, 1500);
        } else {
          errEl.textContent = data.message || 'Registration failed';
          errEl.style.display = 'block';
        }
      } catch (err) {
        errEl.textContent = 'Network error';
        errEl.style.display = 'block';
      } finally {
        btn.disabled = false;
        btn.textContent = 'Register';
      }
    });
  </script>
</body>
</html>`;
  setHtmlSecurityHeaders(ctx.res);
  ctx.res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  ctx.res.end(html);
  return true;
}

interface RegisterBody {
  user_id: string;
  password: string;
  nickname?: string;
  avatar?: string;
  email?: string;
}

async function handleRegister(ctx: RequestContext, storage: IStorage): Promise<boolean> {
  const body = parseJsonBody<RegisterBody>(ctx.body);
  if (!body?.user_id || !body?.password) {
    sendJson(ctx.res, 400, { code: -1, message: 'user_id and password are required' });
    return true;
  }

  const usernameError = validateUsername(body.user_id);
  if (usernameError) {
    sendJson(ctx.res, 400, { code: -1, message: usernameError });
    return true;
  }

  const passwordError = validatePassword(body.password);
  if (passwordError) {
    sendJson(ctx.res, 400, { code: -1, message: passwordError });
    return true;
  }

  const existing = await storage.user.findByUserId(body.user_id);
  if (existing) {
    sendJson(ctx.res, 409, { code: -1, message: 'user already exists' });
    return true;
  }

  const user = await storage.user.register(body.user_id, body.password, {
    nickname: body.nickname?.slice(0, 64),
    avatar: body.avatar?.slice(0, 256),
    email: body.email?.slice(0, 128),
  });

  const token = nanoid(32);
  await storage.user.saveToken(body.user_id, token);

  sendJson(ctx.res, 200, {
    code: 0,
    message: 'ok',
    data: {
      user_id: user.user_id,
      nickname: user.nickname,
      avatar: user.avatar,
      email: user.email,
      token,
    },
  });
  return true;
}
