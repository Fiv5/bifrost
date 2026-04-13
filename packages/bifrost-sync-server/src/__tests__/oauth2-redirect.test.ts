import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import http from 'http';
import fs from 'fs';
import path from 'path';
import { createSyncServer, type SyncServerInstance, type SyncServerConfig } from '../index';

const TEST_DATA_DIR = path.join(__dirname, '.test-data-oauth2');

function rawGet(
  baseUrl: string,
  urlPath: string,
  headers: Record<string, string> = {},
): Promise<{ status: number; headers: http.IncomingHttpHeaders; body: string }> {
  return new Promise((resolve, reject) => {
    const url = new URL(urlPath, baseUrl);
    const options: http.RequestOptions = {
      method: 'GET',
      hostname: url.hostname,
      port: url.port,
      path: url.pathname + url.search,
      headers,
    };
    const r = http.request(options, (res) => {
      let chunks = '';
      res.on('data', (c: Buffer) => (chunks += c.toString()));
      res.on('end', () => {
        resolve({ status: res.statusCode!, headers: res.headers, body: chunks });
      });
    });
    r.on('error', reject);
    r.end();
  });
}

async function listenServer(instance: SyncServerInstance): Promise<number> {
  return new Promise<number>((resolve) => {
    instance.server.listen(0, '127.0.0.1', () => {
      const addr = instance.server.address();
      if (addr && typeof addr === 'object') {
        instance.port = addr.port;
        resolve(addr.port);
      }
    });
  });
}

const OAUTH2_CONFIG = {
  client_id: 'test-client',
  client_secret: 'test-secret',
  authorize_url: 'https://auth.example.com/authorize',
  token_url: 'https://auth.example.com/token',
  userinfo_url: 'https://auth.example.com/userinfo',
  scopes: ['openid', 'profile'],
};

describe('OAuth2 getRedirectUri with trust_forwarded_for=false (default)', () => {
  let server: SyncServerInstance;
  let baseUrl: string;

  beforeAll(async () => {
    if (fs.existsSync(TEST_DATA_DIR)) fs.rmSync(TEST_DATA_DIR, { recursive: true });
    fs.mkdirSync(TEST_DATA_DIR, { recursive: true });

    const config: SyncServerConfig = {
      server: { port: 0, host: '127.0.0.1' },
      storage: { type: 'sqlite', sqlite: { data_dir: TEST_DATA_DIR } },
      auth: { mode: 'oauth2', oauth2: OAUTH2_CONFIG },
    };
    server = createSyncServer(config);
    const port = await listenServer(server);
    baseUrl = `http://127.0.0.1:${port}`;
  });

  afterAll(async () => {
    await server?.close();
    if (fs.existsSync(TEST_DATA_DIR)) fs.rmSync(TEST_DATA_DIR, { recursive: true });
  });

  it('should generate http redirect_uri by default (ignoring x-forwarded-proto)', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      'x-forwarded-proto': 'https',
      host: 'sync.example.com',
    });
    expect(res.status).toBe(302);
    const location = res.headers['location'] as string;
    expect(location).toBeDefined();
    const url = new URL(location);
    const redirectUri = url.searchParams.get('redirect_uri');
    expect(redirectUri).toBe('http://sync.example.com/v4/sso/callback');
  });

  it('should use host header for redirect_uri', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      host: 'my-sync.local:9090',
    });
    expect(res.status).toBe(302);
    const location = res.headers['location'] as string;
    const url = new URL(location);
    const redirectUri = url.searchParams.get('redirect_uri');
    expect(redirectUri).toBe('http://my-sync.local:9090/v4/sso/callback');
  });
});

describe('OAuth2 getRedirectUri with trust_forwarded_for=true', () => {
  let server: SyncServerInstance;
  let baseUrl: string;

  beforeAll(async () => {
    const dataDir = TEST_DATA_DIR + '-trust';
    if (fs.existsSync(dataDir)) fs.rmSync(dataDir, { recursive: true });
    fs.mkdirSync(dataDir, { recursive: true });

    const config: SyncServerConfig = {
      server: { port: 0, host: '127.0.0.1', trust_forwarded_for: true },
      storage: { type: 'sqlite', sqlite: { data_dir: dataDir } },
      auth: { mode: 'oauth2', oauth2: OAUTH2_CONFIG },
    };
    server = createSyncServer(config);
    const port = await listenServer(server);
    baseUrl = `http://127.0.0.1:${port}`;
  });

  afterAll(async () => {
    await server?.close();
    const dataDir = TEST_DATA_DIR + '-trust';
    if (fs.existsSync(dataDir)) fs.rmSync(dataDir, { recursive: true });
  });

  it('should use x-forwarded-proto when trust_forwarded_for is enabled', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      'x-forwarded-proto': 'https',
      host: 'sync.example.com',
    });
    expect(res.status).toBe(302);
    const location = res.headers['location'] as string;
    const url = new URL(location);
    const redirectUri = url.searchParams.get('redirect_uri');
    expect(redirectUri).toBe('https://sync.example.com/v4/sso/callback');
  });

  it('should fallback to http when x-forwarded-proto is absent', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      host: 'sync.example.com',
    });
    expect(res.status).toBe(302);
    const location = res.headers['location'] as string;
    const url = new URL(location);
    const redirectUri = url.searchParams.get('redirect_uri');
    expect(redirectUri).toBe('http://sync.example.com/v4/sso/callback');
  });

  it('should use x-forwarded-for for rate limiting', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      'x-forwarded-for': '203.0.113.50',
      host: 'sync.example.com',
    });
    expect(res.status).toBe(302);
  });
});

describe('OAuth2 with explicit redirect_uri config', () => {
  let server: SyncServerInstance;
  let baseUrl: string;

  beforeAll(async () => {
    const dataDir = TEST_DATA_DIR + '-explicit';
    if (fs.existsSync(dataDir)) fs.rmSync(dataDir, { recursive: true });
    fs.mkdirSync(dataDir, { recursive: true });

    const config: SyncServerConfig = {
      server: { port: 0, host: '127.0.0.1' },
      storage: { type: 'sqlite', sqlite: { data_dir: dataDir } },
      auth: {
        mode: 'oauth2',
        oauth2: {
          ...OAUTH2_CONFIG,
          redirect_uri: 'https://my-domain.com/v4/sso/callback',
        },
      },
    };
    server = createSyncServer(config);
    const port = await listenServer(server);
    baseUrl = `http://127.0.0.1:${port}`;
  });

  afterAll(async () => {
    await server?.close();
    const dataDir = TEST_DATA_DIR + '-explicit';
    if (fs.existsSync(dataDir)) fs.rmSync(dataDir, { recursive: true });
  });

  it('should always use configured redirect_uri regardless of headers', async () => {
    const res = await rawGet(baseUrl, '/v4/sso/login?next=/done', {
      'x-forwarded-proto': 'http',
      host: 'other-host.com',
    });
    expect(res.status).toBe(302);
    const location = res.headers['location'] as string;
    const url = new URL(location);
    const redirectUri = url.searchParams.get('redirect_uri');
    expect(redirectUri).toBe('https://my-domain.com/v4/sso/callback');
  });
});
