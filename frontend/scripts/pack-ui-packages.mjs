import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(scriptDir, '..');
const packageDir = resolve(rootDir, 'dist', 'package-tarballs');
const npmCacheDir = resolve(rootDir, 'dist', '.npm-cache');

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    env: {
      ...process.env,
      npm_config_cache: npmCacheDir,
    },
    stdio: 'inherit',
    shell: false,
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

mkdirSync(packageDir, { recursive: true });
mkdirSync(npmCacheDir, { recursive: true });

run('npm', ['run', 'build:packages']);
run('npm', ['pack', '--workspaces', '--pack-destination', packageDir]);

console.log(`\nPackage tarballs written to ${packageDir}`);
