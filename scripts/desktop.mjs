import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve, delimiter } from 'node:path';
const local = resolve('.tools');
const env = { ...process.env };
const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') ?? 'PATH';
if (existsSync(resolve(local, 'cargo/bin'))) {
  env.CARGO_HOME = resolve(local, 'cargo');
  env.RUSTUP_HOME = resolve(local, 'rustup');
  env[pathKey] = `${resolve(local, 'cargo/bin')}${delimiter}${env[pathKey] ?? ''}`;
}
const result = spawnSync(process.execPath, ['node_modules/@tauri-apps/cli/tauri.js', ...process.argv.slice(2)], { env, stdio: 'inherit' });
process.exit(result.status ?? 1);
