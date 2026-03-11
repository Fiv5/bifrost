import { cpSync, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const profile = process.argv[2] === 'release' ? 'release' : 'debug';
const binaryName = process.platform === 'win32' ? 'bifrost.exe' : 'bifrost';
const source = join(process.cwd(), 'target', profile, binaryName);
const destinationDir = join(process.cwd(), 'desktop', 'src-tauri', 'resources', 'bin');
const destination = join(destinationDir, binaryName);

if (!existsSync(source)) {
  console.error(`Missing sidecar binary: ${source}`);
  process.exit(1);
}

mkdirSync(destinationDir, { recursive: true });
cpSync(source, destination);
console.log(`Prepared Tauri sidecar: ${destination}`);
