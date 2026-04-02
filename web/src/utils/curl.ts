import type { BodyType, RawType, ReplayKeyValueItem, TrafficRecord } from '../types';

function escapeShellArg(str: string): string {
  return str.replace(/'/g, "'\\''");
}

export function generateCurl(record: TrafficRecord): string {
  const parts: string[] = ['curl'];

  if (record.method !== 'GET') {
    parts.push(`-X ${record.method}`);
  }

  parts.push(`'${escapeShellArg(record.url)}'`);

  if (record.request_headers) {
    for (const [key, value] of record.request_headers) {
      const lowerKey = key.toLowerCase();
      if (lowerKey === 'host' || lowerKey === 'content-length') {
        continue;
      }
      parts.push(`-H '${escapeShellArg(key)}: ${escapeShellArg(value)}'`);
    }
  }

  if (record.request_body) {
    parts.push(`--data '${escapeShellArg(record.request_body)}'`);
  }

  return parts.join(' \\\n  ');
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function decodeAnsiCStringEscape(
  input: string,
  startIndex: number,
): { char: string; nextIndex: number } {
  const ch = input[startIndex];
  if (ch === undefined) {
    return { char: '', nextIndex: startIndex };
  }

  const simple: Record<string, string> = {
    a: '\u0007',
    b: '\b',
    e: '\u001b',
    f: '\f',
    n: '\n',
    r: '\r',
    t: '\t',
    v: '\u000b',
    '\\': '\\',
    "'": "'",
    '"': '"',
  };

  if (simple[ch] !== undefined) {
    return { char: simple[ch], nextIndex: startIndex + 1 };
  }

  if (ch === 'x') {
    const hex = input.slice(startIndex + 1, startIndex + 3);
    if (/^[0-9a-fA-F]{1,2}$/.test(hex)) {
      return {
        char: String.fromCharCode(parseInt(hex, 16)),
        nextIndex: startIndex + 1 + hex.length,
      };
    }
    return { char: 'x', nextIndex: startIndex + 1 };
  }

  if (ch === 'u') {
    const hex = input.slice(startIndex + 1, startIndex + 5);
    if (/^[0-9a-fA-F]{4}$/.test(hex)) {
      return {
        char: String.fromCharCode(parseInt(hex, 16)),
        nextIndex: startIndex + 5,
      };
    }
    return { char: 'u', nextIndex: startIndex + 1 };
  }

  if (ch === 'U') {
    const hex = input.slice(startIndex + 1, startIndex + 9);
    if (/^[0-9a-fA-F]{8}$/.test(hex)) {
      const codePoint = parseInt(hex, 16);
      return {
        char: String.fromCodePoint(codePoint),
        nextIndex: startIndex + 9,
      };
    }
    return { char: 'U', nextIndex: startIndex + 1 };
  }

  if (/[0-7]/.test(ch)) {
    let j = startIndex;
    let octal = '';
    while (j < input.length && octal.length < 3 && /[0-7]/.test(input[j])) {
      octal += input[j];
      j += 1;
    }
    return {
      char: String.fromCharCode(parseInt(octal, 8)),
      nextIndex: j,
    };
  }

  return { char: ch, nextIndex: startIndex + 1 };
}

function tokenizeShellLikeCommand(input: string): string[] {
  const normalized = input
    .trim()
    .replace(/\\\r?\n/g, ' ')
    .replace(/\^\r?\n/g, ' ')
    .replace(/`\r?\n/g, ' ');
  const tokens: string[] = [];
  let i = 0;
  let current = '';
  let hasCurrent = false;

  const pushCurrent = () => {
    if (!hasCurrent) return;
    tokens.push(current);
    current = '';
    hasCurrent = false;
  };

  while (i < normalized.length) {
    const ch = normalized[i];

    if (/\s/.test(ch)) {
      pushCurrent();
      i += 1;
      continue;
    }

    if (ch === '\\') {
      if (i + 1 < normalized.length) {
        current += normalized[i + 1];
        hasCurrent = true;
        i += 2;
        continue;
      }
      hasCurrent = true;
      i += 1;
      continue;
    }

    if (ch === "'") {
      hasCurrent = true;
      i += 1;
      while (i < normalized.length && normalized[i] !== "'") {
        current += normalized[i];
        i += 1;
      }
      if (i < normalized.length && normalized[i] === "'") {
        i += 1;
      }
      continue;
    }

    if (ch === '"') {
      hasCurrent = true;
      i += 1;
      while (i < normalized.length) {
        const c = normalized[i];
        if (c === '"') {
          i += 1;
          break;
        }
        if (c === '\\') {
          const next = normalized[i + 1];
          if (next === undefined) {
            current += '\\';
            hasCurrent = true;
            i += 1;
            continue;
          }
          if (next === '\\' || next === '"' || next === '$' || next === '`') {
            current += next;
            hasCurrent = true;
            i += 2;
            continue;
          }
          current += `\\${next}`;
          hasCurrent = true;
          i += 2;
          continue;
        }
        current += c;
        hasCurrent = true;
        i += 1;
      }
      continue;
    }

    if (ch === '$' && i + 1 < normalized.length && normalized[i + 1] === "'") {
      hasCurrent = true;
      i += 2;
      while (i < normalized.length) {
        const c = normalized[i];
        if (c === "'") {
          i += 1;
          break;
        }
        if (c === '\\') {
          const decoded = decodeAnsiCStringEscape(normalized, i + 1);
          current += decoded.char;
          hasCurrent = true;
          i = decoded.nextIndex;
          continue;
        }
        current += c;
        hasCurrent = true;
        i += 1;
      }
      continue;
    }

    current += ch;
    hasCurrent = true;
    i += 1;
  }

  pushCurrent();
  return tokens;
}

function expandCurlTokens(tokens: string[]): string[] {
  const expanded: string[] = [];
  for (const t of tokens) {
    if (t.startsWith('--') && t.includes('=')) {
      const idx = t.indexOf('=');
      const key = t.slice(0, idx);
      const value = t.slice(idx + 1);
      expanded.push(key);
      if (value !== '') {
        expanded.push(value);
      }
      continue;
    }

    const shortWithArg = ['-X', '-H', '-d', '-F', '-b', '-u', '-e', '-x', '-A'];
    const matched = shortWithArg.find((k) => t.startsWith(k) && t.length > k.length);
    if (matched) {
      expanded.push(matched);
      expanded.push(t.slice(matched.length));
      continue;
    }

    expanded.push(t);
  }
  return expanded;
}

export interface ParsedCurl {
  method: string;
  url: string;
  headers: ReplayKeyValueItem[];
  body?: {
    type: BodyType;
    raw_type?: RawType;
    content?: string;
    form_data?: ReplayKeyValueItem[];
  };
}

function inferRawTypeFromContentType(contentType: string): RawType {
  const ct = contentType.toLowerCase();
  if (ct.includes('json')) return 'json';
  if (ct.includes('xml')) return 'xml';
  if (ct.includes('html')) return 'html';
  if (ct.includes('javascript') || ct.includes('ecmascript')) return 'javascript';
  return 'text';
}

function isValidHeaderName(name: string): boolean {
  return /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(name);
}

function sanitizeHeaderValue(value: string): string {
  return value.replace(/[\r\n]+/g, ' ').trim();
}

function parseHeaderLine(headerValue: string): { key: string; value: string } | null {
  const normalized = headerValue.replace(/[\r\n]+/g, ' ');
  const colonIndex = normalized.indexOf(':');
  if (colonIndex === -1) return null;
  const key = normalized.substring(0, colonIndex).trim();
  const value = sanitizeHeaderValue(normalized.substring(colonIndex + 1));
  if (!key || !isValidHeaderName(key)) return null;
  return { key, value };
}

function upsertHeader(
  headers: ReplayKeyValueItem[],
  key: string,
  value: string,
  mode: 'replace' | 'append_cookie' = 'replace',
) {
  const lower = key.toLowerCase();
  const idx = headers.findIndex((h) => h.key.toLowerCase() === lower);
  const sanitizedValue = sanitizeHeaderValue(value);
  if (idx === -1) {
    if (!isValidHeaderName(key)) return;
    headers.push({ id: generateId(), key, value: sanitizedValue, enabled: true });
    return;
  }

  if (mode === 'append_cookie') {
    const existing = headers[idx].value || '';
    const next = existing.trim()
      ? `${existing.replace(/;\s*$/, '')}; ${sanitizedValue.replace(/^\s*;\s*/, '')}`
      : sanitizedValue;
    headers[idx] = { ...headers[idx], value: next };
    return;
  }

  headers[idx] = { ...headers[idx], value: sanitizedValue };
}

function isCurlCommandToken(token: string): boolean {
  const lower = token.toLowerCase();
  const base = lower.split(/[\\/]/).pop() || lower;
  return base === 'curl' || base === 'curl.exe';
}

function base64EncodeUtf8(input: string): string {
  const globalWithBase64 = globalThis as typeof globalThis & {
    Buffer?: {
      from: (value: string, encoding: string) => { toString: (encoding: string) => string };
    };
    btoa?: (value: string) => string;
  };
  if (typeof globalWithBase64.Buffer !== 'undefined') {
    return globalWithBase64.Buffer.from(input, 'utf8').toString('base64');
  }
  if (typeof globalWithBase64.btoa !== 'undefined') {
    const bytes = encodeURIComponent(input).replace(/%([0-9A-F]{2})/g, (_, p1) =>
      String.fromCharCode(parseInt(p1, 16)),
    );
    return globalWithBase64.btoa(bytes);
  }
  return input;
}

export function parseCurl(curlCommand: string): ParsedCurl | null {
  const rawTokens = tokenizeShellLikeCommand(curlCommand);
  const tokens = expandCurlTokens(rawTokens);
  if (tokens.length === 0 || !isCurlCommandToken(tokens[0])) {
    return null;
  }

  let method = 'GET';
  let explicitMethod = false;
  let url = '';
  const headers: ReplayKeyValueItem[] = [];
  const bodyParts: string[] = [];
  let contentType = '';
  let sawData = false;
  let getMode = false;

  const positional: string[] = [];
  const optionsWithValue = new Set([
    '-X',
    '--request',
    '--url',
    '-H',
    '--header',
    '-d',
    '--data',
    '--data-raw',
    '--data-binary',
    '--data-urlencode',
    '--data-ascii',
    '--json',
    '-e',
    '--referer',
    '-u',
    '--user',
    '-b',
    '--cookie',
    '--cookie-jar',
    '-x',
    '--proxy',
    '--proxy-user',
    '--proxy-header',
    '-A',
    '--user-agent',
    '--request-target',
    '-o',
    '--output',
    '--connect-to',
    '--resolve',
    '--cacert',
    '--capath',
    '--cert',
    '--key',
    '--pass',
    '--max-time',
    '--connect-timeout',
    '--retry',
    '--retry-delay',
    '--retry-max-time',
  ]);

  for (let i = 1; i < tokens.length; i += 1) {
    const token = tokens[i];

    if (token === '--') {
      positional.push(...tokens.slice(i + 1));
      break;
    }

    if (token === '-X' || token === '--request') {
      const value = tokens[i + 1];
      if (value) {
        method = value.toUpperCase();
        explicitMethod = true;
        i += 1;
      }
      continue;
    }

    if (token === '-I' || token === '--head') {
      if (!explicitMethod) {
        method = 'HEAD';
        explicitMethod = true;
      }
      continue;
    }

    if (token === '-G' || token === '--get') {
      getMode = true;
      continue;
    }

    if (token === '--url') {
      const value = tokens[i + 1];
      if (value) {
        url = value;
        i += 1;
      }
      continue;
    }

    if (token === '-e' || token === '--referer') {
      if (tokens[i + 1]) {
        i += 1;
      }
      continue;
    }

    if (token === '-u' || token === '--user') {
      const value = tokens[i + 1];
      if (value !== undefined) {
        const hasAuth = headers.some((h) => h.key.toLowerCase() === 'authorization');
        if (!hasAuth) {
          upsertHeader(headers, 'Authorization', `Basic ${base64EncodeUtf8(value)}`);
        }
        i += 1;
      }
      continue;
    }

    if (token === '-A' || token === '--user-agent') {
      const value = tokens[i + 1];
      if (value) {
        upsertHeader(headers, 'User-Agent', value);
        i += 1;
      }
      continue;
    }

    if (token === '-H' || token === '--header') {
      const headerValue = tokens[i + 1];
      if (headerValue) {
        const parsed = parseHeaderLine(headerValue);
        if (parsed) {
          headers.push({
            id: generateId(),
            key: parsed.key,
            value: parsed.value,
            enabled: true,
          });
          if (parsed.key.toLowerCase() === 'content-type') {
            contentType = parsed.value;
          }
        }
        i += 1;
      }
      continue;
    }

    if (token === '-b' || token === '--cookie') {
      const value = tokens[i + 1];
      if (value !== undefined) {
        if (!value.startsWith('@')) {
          upsertHeader(headers, 'Cookie', value, 'append_cookie');
        }
        i += 1;
      }
      continue;
    }

    if (
      token === '-d' ||
      token === '--data' ||
      token === '--data-raw' ||
      token === '--data-binary' ||
      token === '--data-urlencode' ||
      token === '--data-ascii' ||
      token === '--json'
    ) {
      const value = tokens[i + 1];
      if (value !== undefined) {
        bodyParts.push(value);
        sawData = true;
        i += 1;
      }
      if (token === '--json') {
        if (!contentType) {
          contentType = 'application/json';
          upsertHeader(headers, 'Content-Type', 'application/json');
        }
      }
      if (!getMode && !explicitMethod && method === 'GET') {
        method = 'POST';
      }
      continue;
    }

    if (token.startsWith('-')) {
      if (optionsWithValue.has(token) && tokens[i + 1]) {
        i += 1;
      }
      continue;
    }

    positional.push(token);
  }

  if (!url) {
    const urlCandidates = positional.filter((t) => /^(https?|wss?):\/\//i.test(t));
    if (urlCandidates.length > 0) {
      url = urlCandidates[urlCandidates.length - 1];
    } else if (positional.length > 0) {
      url = positional[positional.length - 1];
    }
  }

  if (!url) {
    return null;
  }

  const result: ParsedCurl = {
    method,
    url,
    headers,
  };

  if (getMode && sawData && bodyParts.length > 0) {
    const joined = bodyParts.join('&');
    const hashIndex = result.url.indexOf('#');
    const base = hashIndex === -1 ? result.url : result.url.slice(0, hashIndex);
    const hash = hashIndex === -1 ? '' : result.url.slice(hashIndex);
    const sep = base.includes('?') ? '&' : '?';
    result.url = `${base}${joined ? sep + joined : ''}${hash}`;
    return result;
  }

  if (sawData && bodyParts.length > 0) {
    result.body = {
      type: 'raw',
      raw_type: inferRawTypeFromContentType(contentType),
      content: bodyParts.join('&'),
    };
  }

  return result;
}
