import { readFile, readdir, stat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
const root = resolve(import.meta.dirname, '..');
const html = await readFile(resolve(root, 'dist/index.html'), 'utf8');
if (!html.includes('PoCol') || !html.includes('<title>')) throw new Error('Missing product title');
for (const match of html.matchAll(/(?:src|href)=["']([^"']+)["']/g)) {
  const value = match[1].split(/[?#]/)[0];
  if (!value || /^(?:https?:|data:|blob:|mailto:|chrome:)/.test(value)) continue;
  const path = resolve(root, 'dist', value.replace(/^\//, ''));
  if (!(await stat(path)).isFile()) throw new Error('Missing linked asset: ' + value);
}
for (const file of await readdir(resolve(root, 'dist'))) {
  if (!file.endsWith('.js')) continue;
  const result = spawnSync(process.execPath, ['--check', resolve(root, 'dist', file)], {encoding:'utf8'});
  if (result.status) throw new Error(result.stderr);
}
console.log('Static package links and JavaScript syntax verified.');
