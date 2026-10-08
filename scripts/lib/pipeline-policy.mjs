import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';

export const components = ['backend-admin', 'frontend-admin', 'backend-school', 'frontend-school'];
export const suites = ['documentation', 'deployment', 'contracts', ...components];
const control = /^(?:\.github\/|scripts\/(?:pipeline|lib\/pipeline|lib\/schoolorbit-installer|schoolorbit-installer|render_nginx|discover_school|find_worker|smoke_test|prune_runtime|clamd_runtime|reconcile_r2)|podman-compose\.yml$|nginx-configs\/)/;
const contracts = /^(?:contracts\/|scripts\/(?:generate-(?:api-contracts|permissions)|tests\/generate-))/;
const testOnly = (file) => /^(?:backend|frontend)-(?:school|admin)\/tests\/|^scripts\/tests\/|\/README\.md$/.test(file);

export function classify(files) {
  const verify = new Set(['documentation']);
  const deploy = new Set();
  const unknown = [];
  for (const file of files) {
    if (!file || file.startsWith('/') || file.split('/').includes('..')) throw new Error('Invalid change path');
    if (file.endsWith('.md')) continue;
    if (file === 'scripts/prepare_frontend_worker.mjs') {
      ['deployment', 'frontend-admin', 'frontend-school'].forEach(suite => verify.add(suite));
      ['frontend-admin', 'frontend-school'].forEach(part => deploy.add(part));
      continue;
    }
    if (file === '.rules' || control.test(file)) {
      suites.forEach((suite) => verify.add(suite));
      if (!testOnly(file) && /^(?:podman-compose\.yml$|nginx-configs\/|scripts\/(?:render_nginx|smoke_test|lib\/pipeline-remote)|\.github\/workflows\/(?:release|deploy-))/.test(file)) components.forEach((part) => deploy.add(part));
      continue;
    }
    if (contracts.test(file)) {
      ['contracts', 'backend-school', 'frontend-school'].forEach((suite) => verify.add(suite));
      if (!testOnly(file)) ['backend-school', 'frontend-school'].forEach((part) => deploy.add(part));
      continue;
    }
    const part = components.find((name) => file.startsWith(`${name}/`));
    if (part) {
      verify.add(part);
      if (!testOnly(file)) deploy.add(part);
      if (part === 'backend-school') verify.add('contracts');
      if (part === 'backend-school' && /\/(?:school-(?:auth|permissions|tenancy)|auth|permissions|websockets|realtime)(?:[/.~-]|$)/.test(file)) verify.add('frontend-school');
      // Generated contracts and consumers form one release unit. Body-only service edits
      // still export/check OpenAPI but do not build the frontend unless its contract changes.
      if (file.startsWith('backend-school/src/api_') || /\/registry(?:_generated)?\./.test(file)) {
        verify.add('frontend-school');
        deploy.add('frontend-school');
      }
      if (file.startsWith('frontend-school/tests/static/') && /deploy|installer|release|menu-route/.test(file)) verify.add('deployment');
      continue;
    }
    if (file === 'compose.local.yml' || file.startsWith('scripts/tests/') || file.startsWith('scripts/test_')) {
      verify.add('deployment');
      if (file.includes('backend_school') || file.includes('school_database_suite')) verify.add('backend-school');
      if (file.includes('backend_admin')) verify.add('backend-admin');
      continue;
    }
    if (/^(?:\.gitignore|\.dockerignore|\.editorconfig|\.gitattributes|CLAUDE|GEMINI|AGENTS)$/.test(file)) continue;
    unknown.push(file);
    suites.forEach((suite) => verify.add(suite));
  }
  return { verify: suites.filter((suite) => verify.has(suite)), deploy: components.filter((part) => deploy.has(part)), unknown };
}

export function git(root, ...args) {
  return execFileSync('git', ['-C', root, ...args], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }).trim();
}

export function changes(root, base, head) {
  if (!/^[0-9a-f]{40}$/.test(base) || !/^[0-9a-f]{40}$/.test(head)) throw new Error('Change range requires exact commit IDs');
  // Both sides of renames are included; deleting a runtime input must deploy its owner.
  return git(root, 'diff', '--no-renames', '--name-only', '-z', base, head).split('\0').filter(Boolean);
}

export function inputHash(root, head, component, publicConfig = {}) {
  if (!components.includes(component)) throw new Error('Unknown component');
  const entries = git(root, 'ls-tree', '-r', head, '--', component, '.github/workflows/prepare.yml', ...(component.startsWith('frontend-') ? ['scripts/prepare_frontend_worker.mjs'] : []))
    .split('\n').filter(Boolean).filter((entry) => !testOnly(entry.split('\t')[1] || ''));
  return createHash('sha256').update(JSON.stringify({ version: 1, entries, publicConfig, platform: 'linux/amd64', node: component.startsWith('frontend-') ? process.versions.node : null, rust: '1.98.1', profile: 'release' })).digest('hex');
}

export function makePlan({ root, head, base, state = null, scope = 'auto', publicConfig = {} }) {
  const commit = classify(changes(root, base, head));
  const verify = new Set(commit.verify);
  const deploy = new Set();
  for (const part of components) {
    const accepted = state?.components?.[part]?.sha;
    if (!accepted) {
      deploy.add(part);
      continue;
    }
    try {
      if (git(root, 'merge-base', accepted, head) !== accepted) throw new Error('Divergent baseline');
      const queuedFiles = changes(root, accepted, head);
      const queued = classify(queuedFiles);
      if (queued.deploy.includes(part) || state.components[part].inputHash !== inputHash(root, head, part, publicConfig[part] || {})) {
        deploy.add(part);
        // Another component can have a newer accepted SHA. Its already deployed
        // history must not make this owner reverify unrelated code on every run.
        classify(queuedFiles.filter(file => classify([file]).deploy.includes(part))).verify.forEach(suite => verify.add(suite));
      }
    } catch {
      deploy.add(part);
    }
  }
  if (scope !== 'auto') {
    if (scope === 'full') components.forEach((part) => deploy.add(part));
    else if (components.includes(scope)) deploy.add(scope);
    else throw new Error('Invalid requested scope');
  }
  // An explicit scope may add work; it cannot omit queued incompatible changes.
  for (const part of deploy) verify.add(part);
  if (verify.has('backend-school')) verify.add('contracts');
  const build = components.filter((part) => deploy.has(part));
  const hashes = Object.fromEntries(components.map((part) => [part, inputHash(root, head, part, publicConfig[part] || {})]));
  return { schemaVersion: 1, sha: head, tree: git(root, 'rev-parse', `${head}^{tree}`), base, verify: suites.filter((suite) => verify.has(suite)), build, deploy: build, hashes, unknown: commit.unknown };
}
