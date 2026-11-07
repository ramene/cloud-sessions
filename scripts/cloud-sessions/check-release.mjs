// Release consistency for the cloud-sessions mod: every manifest names one version and the changelog has its entry.
// usage: node scripts/cloud-sessions/check-release.mjs [expected version]
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../', import.meta.url);
const read = path => readFileSync(new URL(path, root), 'utf8');
const toml = path => /^version = "([^"]+)"/m.exec(read(path))?.[1];

export function versions() {
  const marketplace = JSON.parse(read('.claude-plugin/marketplace.json'));
  const inner = JSON.parse(read('claude-mod/.claude-plugin/marketplace.json'));
  return {
    core: toml('crates/cloud-sessions-core/Cargo.toml'),
    cli: toml('crates/cloud-sessions-cli/Cargo.toml'),
    pluginVersionFile: read('claude-mod/cloud-sessions/VERSION').trim(),
    plugin: JSON.parse(read('claude-mod/cloud-sessions/.claude-plugin/plugin.json')).version,
    marketplace: marketplace.plugins.find(p => p.name === 'cloud-sessions')?.version,
    marketplaceInner: inner.plugins.find(p => p.name === 'cloud-sessions')?.version,
    formula: /^\s*version "([^"]+)"/m.exec(read('packaging/homebrew/cloud-sessions.rb'))?.[1],
    changelog: /^## \[?(\d+\.\d+\.\d+)\]?/m.exec(read('CHANGELOG.md'))?.[1],
  };
}
export function problems(expected) {
  const found = versions();
  const want = expected ?? found.core;
  return Object.entries(found).filter(([, v]) => v !== want).map(([name, v]) => `${name} is ${v ?? 'missing'}, expected ${want}`);
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const issues = problems(process.argv[2]);
  if (issues.length) { console.error(issues.join('\n')); process.exit(1); }
  console.log(`cloud-sessions ${versions().core}: all manifests agree`);
}
