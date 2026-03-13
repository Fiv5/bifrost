import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const version = process.argv[2] || process.env.BIFROST_VERSION || process.env.VERSION;

if (!version) {
  console.error('Missing version. Pass it as argv[2] or set BIFROST_VERSION/VERSION.');
  process.exit(1);
}

const configPath = join(process.cwd(), 'desktop', 'src-tauri', 'tauri.conf.json');
const config = JSON.parse(readFileSync(configPath, 'utf8'));

if (config.version === version) {
  console.log(`Tauri version already synced: ${version}`);
  process.exit(0);
}

config.version = version;
writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
console.log(`Synced Tauri version to ${version}`);
