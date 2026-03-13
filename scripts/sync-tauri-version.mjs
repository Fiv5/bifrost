import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const msiSafe = process.argv.includes('--msi');
const positionalArgs = process.argv.slice(2).filter((arg) => !arg.startsWith('--'));
const version = positionalArgs[0] || process.env.BIFROST_VERSION || process.env.VERSION;

if (!version) {
  console.error('Missing version. Pass it as argv[2] or set BIFROST_VERSION/VERSION.');
  process.exit(1);
}

function toMsiVersion(input) {
  const match = input.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?$/);
  if (!match) {
    throw new Error(`Unsupported version format for MSI conversion: ${input}`);
  }

  const [, major, minor, patch, prerelease] = match;
  if (!prerelease) {
    return `${major}.${minor}.${patch}`;
  }

  const parts = prerelease.split(/[.-]+/).filter(Boolean);
  const first = (parts[0] || '').toLowerCase();

  if (/^\d+$/.test(first)) {
    const value = Number.parseInt(first, 10);
    if (value > 65535) {
      throw new Error(`Numeric prerelease identifier exceeds MSI limit: ${input}`);
    }
    return `${major}.${minor}.${patch}-${value}`;
  }

  const channelBases = new Map([
    ['alpha', 10000],
    ['beta', 20000],
    ['rc', 30000],
  ]);

  const labelMatch = first.match(/^([a-z]+)(\d+)?$/);
  const label = labelMatch?.[1] ?? first;
  const inlineSequence = labelMatch?.[2] ? Number.parseInt(labelMatch[2], 10) : null;
  const explicitSequence = parts.find((part, index) => index > 0 && /^\d+$/.test(part));
  const sequence = inlineSequence ?? (explicitSequence ? Number.parseInt(explicitSequence, 10) : null);

  const base = channelBases.get(label) ?? 40000;
  const fallbackHash = [...prerelease].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 10000;
  const suffix = channelBases.has(label) ? Math.min(sequence ?? 0, 9999) : Math.min(sequence ?? fallbackHash, 9999);

  return `${major}.${minor}.${patch}-${base + suffix}`;
}

const configPath = join(process.cwd(), 'desktop', 'src-tauri', 'tauri.conf.json');
const config = JSON.parse(readFileSync(configPath, 'utf8'));
const nextVersion = msiSafe ? toMsiVersion(version) : version;

if (config.version === nextVersion) {
  console.log(`Tauri version already synced: ${nextVersion}`);
  process.exit(0);
}

config.version = nextVersion;
writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
if (msiSafe) {
  console.log(`Synced Tauri version to MSI-safe version ${nextVersion} (from ${version})`);
} else {
  console.log(`Synced Tauri version to ${nextVersion}`);
}
