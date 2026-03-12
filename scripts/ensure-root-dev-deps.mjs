import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const rootDir = process.cwd();
const tauriCliDir = join(rootDir, 'node_modules', '@tauri-apps', 'cli');

if (existsSync(tauriCliDir)) {
  process.exit(0);
}

if (spawnSync('pnpm', ['--version'], { stdio: 'inherit' }).status !== 0) {
  console.error('pnpm is required to install root dependencies for the desktop build.');
  process.exit(1);
}

console.warn('Root node_modules is missing. Installing root dependencies for Tauri CLI...');

const install = spawnSync('pnpm', ['install', '--frozen-lockfile'], {
  env: {
    ...process.env,
    npm_config_registry: 'https://registry.npmjs.org/',
  },
  stdio: 'inherit',
});

if (install.status !== 0) {
  console.error('Failed to install root dependencies. Run `pnpm install --frozen-lockfile` in the repository root and retry.');
  process.exit(install.status ?? 1);
}
