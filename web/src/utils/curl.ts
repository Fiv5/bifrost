import type { TrafficRecord } from '../types';

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
    const body = record.request_body;
    if (body.length > 1000) {
      parts.push(`--data '${escapeShellArg(body.substring(0, 1000))}...'`);
    } else {
      parts.push(`--data '${escapeShellArg(body)}'`);
    }
  }

  return parts.join(' \\\n  ');
}
