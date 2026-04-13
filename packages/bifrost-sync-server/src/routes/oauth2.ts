import type { IStorage } from '../dao/types';
import type { RequestContext } from '../http';
import type { OAuth2Config } from '../types';
import { sendJson, sendError } from '../http';
import { nanoid } from 'nanoid';
import crypto from 'crypto';

const pendingStates = new Map<string, { next: string; created: number }>();

setInterval(() => {
  const now = Date.now();
  for (const [key, val] of pendingStates) {
    if (now - val.created > 600_000) pendingStates.delete(key);
  }
}, 60_000);

export async function handleOAuth2(
  ctx: RequestContext,
  storage: IStorage,
  oauth2: OAuth2Config,
): Promise<boolean> {
  const { url, req } = ctx;
  const method = req.method ?? 'GET';

  if (url.pathname === '/v4/sso/login' && method === 'GET') {
    return handleOAuth2Login(ctx, oauth2);
  }
  if (url.pathname === '/v4/sso/callback' && method === 'GET') {
    return handleOAuth2Callback(ctx, storage, oauth2);
  }

  return false;
}

function getRedirectUri(ctx: RequestContext, oauth2: OAuth2Config): string {
  if (oauth2.redirect_uri) return oauth2.redirect_uri;
  const proto = ctx.trustForwardedFor ? (ctx.req.headers['x-forwarded-proto'] ?? 'http') : 'http';
  const host = ctx.req.headers['host'] ?? 'localhost';
  return `${proto}://${host}/v4/sso/callback`;
}

async function handleOAuth2Login(ctx: RequestContext, oauth2: OAuth2Config): Promise<boolean> {
  const next = ctx.url.searchParams.get('next') ?? '/v4/sso/check';
  const state = crypto.randomBytes(16).toString('hex');
  pendingStates.set(state, { next, created: Date.now() });

  const redirectUri = getRedirectUri(ctx, oauth2);
  const params = new URLSearchParams({
    client_id: oauth2.client_id,
    response_type: 'code',
    redirect_uri: redirectUri,
    scope: oauth2.scopes.join(' '),
    state,
  });

  const authorizeUrl = `${oauth2.authorize_url}?${params.toString()}`;
  ctx.res.writeHead(302, { Location: authorizeUrl });
  ctx.res.end();
  return true;
}

async function handleOAuth2Callback(
  ctx: RequestContext,
  storage: IStorage,
  oauth2: OAuth2Config,
): Promise<boolean> {
  const code = ctx.url.searchParams.get('code');
  const state = ctx.url.searchParams.get('state');
  const error = ctx.url.searchParams.get('error');

  if (error) {
    const desc = ctx.url.searchParams.get('error_description') ?? error;
    sendJson(ctx.res, 400, { code: -1, message: `OAuth2 error: ${desc}` });
    return true;
  }

  if (!code || !state) {
    sendError(ctx.res, 400, 'missing code or state');
    return true;
  }

  const pending = pendingStates.get(state);
  if (!pending) {
    sendError(ctx.res, 400, 'invalid or expired state');
    return true;
  }
  pendingStates.delete(state);

  try {
    const redirectUri = getRedirectUri(ctx, oauth2);

    const tokenRes = await fetch(oauth2.token_url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        Accept: 'application/json',
      },
      body: new URLSearchParams({
        grant_type: 'authorization_code',
        client_id: oauth2.client_id,
        client_secret: oauth2.client_secret,
        code,
        redirect_uri: redirectUri,
      }).toString(),
    });

    if (!tokenRes.ok) {
      const text = await tokenRes.text();
      console.error('[bifrost-sync-server] OAuth2 token exchange failed:', text);
      sendError(ctx.res, 502, 'OAuth2 token exchange failed');
      return true;
    }

    const tokenData = (await tokenRes.json()) as Record<string, unknown>;
    const accessToken = tokenData.access_token as string;
    if (!accessToken) {
      sendError(ctx.res, 502, 'OAuth2 token response missing access_token');
      return true;
    }

    const userRes = await fetch(oauth2.userinfo_url, {
      headers: {
        Authorization: `Bearer ${accessToken}`,
        Accept: 'application/json',
      },
    });

    if (!userRes.ok) {
      const text = await userRes.text();
      console.error('[bifrost-sync-server] OAuth2 userinfo failed:', text);
      sendError(ctx.res, 502, 'OAuth2 userinfo request failed');
      return true;
    }

    const userInfo = (await userRes.json()) as Record<string, unknown>;

    const userIdField = oauth2.user_id_field ?? 'sub';
    const nicknameField = oauth2.nickname_field ?? 'name';
    const emailField = oauth2.email_field ?? 'email';
    const avatarField = oauth2.avatar_field ?? 'picture';

    const userId = getNestedField(userInfo, userIdField);
    if (!userId) {
      console.error('[bifrost-sync-server] OAuth2 userinfo missing user_id field:', userIdField, userInfo);
      sendError(ctx.res, 502, `OAuth2 userinfo missing field: ${userIdField}`);
      return true;
    }

    const nickname = getNestedField(userInfo, nicknameField) ?? '';
    const email = getNestedField(userInfo, emailField) ?? '';
    const avatar = getNestedField(userInfo, avatarField) ?? '';

    let user = await storage.user.findByUserId(userId);
    if (!user) {
      const randomPassword = crypto.randomBytes(32).toString('hex');
      user = await storage.user.register(userId, randomPassword, { nickname, email, avatar });
    }

    const bifrostToken = nanoid(32);
    await storage.user.saveToken(userId, bifrostToken);

    const next = pending.next;
    const separator = next.includes('?') ? '&' : '?';
    ctx.res.writeHead(302, {
      Location: `${next}${separator}token=${encodeURIComponent(bifrostToken)}`,
    });
    ctx.res.end();
    return true;
  } catch (e: unknown) {
    console.error('[bifrost-sync-server] OAuth2 callback error:', e);
    sendError(ctx.res, 500, 'OAuth2 callback failed');
    return true;
  }
}

function getNestedField(obj: Record<string, unknown>, field: string): string | undefined {
  const parts = field.split('.');
  let current: unknown = obj;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current != null ? String(current) : undefined;
}
