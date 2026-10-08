#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { classify, changes, git, makePlan, suites } from './lib/pipeline-policy.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const command = args.shift();
if (command === 'verify' || command === 'prime') {
  process.env.CARGO_INCREMENTAL ||= '0';
  process.env.CARGO_PROFILE_DEV_DEBUG ||= '0';
  process.env.CARGO_PROFILE_TEST_DEBUG ||= '0';
}
const option = (name, fallback) => { const i = args.indexOf(name); return i === -1 ? fallback : args[i + 1]; };
const run = (cmd, argv, cwd = root, env = {}) => {
  const start = Date.now();
  const result = spawnSync(cmd, argv, { cwd, env: { ...process.env, ...env }, stdio: 'inherit' });
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `- ${cmd} ${argv.join(' ')}: ${((Date.now() - start) / 1000).toFixed(1)}s\n`);
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${cmd} failed (${result.status})`);
};

try {
  if (command === 'plan') {
    const head = git(root, 'rev-parse', 'HEAD');
    const base = option('--base', git(root, 'merge-base', 'origin/main', head));
    const stateFile = option('--state', '');
    const config = {
      'frontend-school': { backend: process.env.PUBLIC_BACKEND_URL || '', vapid: process.env.PUBLIC_VAPID_KEY || '' },
      'frontend-admin': { backend: process.env.PUBLIC_API_URL || '', schoolBackend: process.env.BACKEND_SCHOOL_URL || '' }
    };
    const plan = option('--release', '')
      ? makePlan({ root, head, base, state: stateFile ? JSON.parse(readFileSync(stateFile, 'utf8')) : null, scope: option('--scope', 'auto'), publicConfig: config })
      : { schemaVersion: 1, sha: head, tree: git(root, 'rev-parse', 'HEAD^{tree}'), base, ...classify(changes(root, base, head)), build: [] };
    if (process.env.GITHUB_RUN_ID) { plan.runId = process.env.GITHUB_RUN_ID; plan.attempt = Number(process.env.GITHUB_RUN_ATTEMPT); }
    const output = option('--output', '');
    if (output) writeFileSync(output, JSON.stringify(plan));
    else process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
    if (process.env.GITHUB_OUTPUT) {
      appendFileSync(process.env.GITHUB_OUTPUT, `plan=${JSON.stringify(plan)}\nverify=${JSON.stringify(plan.verify)}\nbuild=${JSON.stringify(plan.build)}\ndeploy=${JSON.stringify(plan.deploy)}\n`);
    }
  } else if (command === 'verify' || command === 'prime') {
    const prime = command === 'prime';
    let selected = option('--scope', 'auto');
    if (prime) {
      const proof = JSON.parse(process.env.PR_VERIFICATION_PROOF || 'null');
      if (process.env.GITHUB_REF !== 'refs/heads/main' || !['push','workflow_dispatch'].includes(process.env.GITHUB_EVENT_NAME) ||
          !['backend-school','backend-admin'].includes(selected) || !proof || !/^\d+$/.test(proof.runId) || !Number.isInteger(proof.attempt) || proof.attempt < 1 ||
          proof.tree !== git(root, 'rev-parse', 'HEAD^{tree}') || !proof.suites?.includes(selected)) throw new Error('Compiler priming requires exact-tree verified PR evidence on main');
      console.log(`Reuse successful PR run ${proof.runId} attempt ${proof.attempt}; compile snapshot only, no duplicate database fixture execution.`);
    }
    const testFlags = prime ? ['--no-run'] : [];
    if (selected === 'auto') {
      const base = git(root, 'merge-base', 'origin/main', 'HEAD');
      const tracked = git(root, 'diff', '--no-renames', '--name-only', '-z', base).split('\0').filter(Boolean);
      const untracked = git(root, 'ls-files', '--others', '--exclude-standard', '-z').split('\0').filter(Boolean);
      selected = classify([...tracked, ...untracked]).verify;
    }
    else selected = selected === 'full' ? suites : selected.split(',');
    for (const suite of selected) {
      if (!suites.includes(suite)) throw new Error(`Unknown verification suite: ${suite}`);
      if (suite === 'documentation') run('node', ['--test', 'frontend-school/tests/static/documentation-policy.test.mjs']);
      if (suite === 'deployment') run('bash', ['scripts/verify_deployment.sh']);
      if (suite === 'contracts') {
        run('node', ['scripts/generate-permissions.mjs', '--check']);
        run('node', ['--test', 'scripts/tests/generate-permissions.test.mjs', 'scripts/tests/generate-api-contracts.test.mjs']);
      }
      if (suite === 'backend-school') {
        const cwd = path.join(root, suite);
        run('cargo', ['fmt', '--all', '--', '--check'], cwd);
        run('cargo', ['check', '--workspace', '--all-targets', '--locked'], cwd);
        run('cargo', ['test', '--test', 'static_architecture', '--locked', ...testFlags], cwd);
        run('cargo', ['test', '--bin', 'backend-school', 'api_contract::tests', '--locked', ...testFlags], cwd);
        run('cargo', ['test', '-p', 'school-academic-assessment', '-p', 'school-auth-http', '-p', 'school-academic-http', '-p', 'school-certificates-http', '-p', 'school-notifications', '--lib', '--locked', ...testFlags], cwd);
        run('node', ['scripts/generate-api-contracts.mjs', '--check']);
        const executable = JSON.parse(execFileSync('cargo', ['metadata', '--locked', '--format-version', '1', '--no-deps'], { cwd, encoding: 'utf8' })).target_directory + '/debug/backend-school';
        JSON.parse(execFileSync('env', ['-i', `PATH=${process.env.PATH}`, `HOME=${process.env.HOME}`, executable, 'export-openapi'], { cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }));
        run('node', ['--test', 'scripts/tests/neon-compatibility.test.mjs']);
        if (prime) {
          run('cargo', ['test', '--bin', 'seed_sandbox', '--locked', '--no-run'], cwd);
          run('cargo', ['test', '--test', 'delivery_versions', '--locked', '--no-run'], cwd);
          // Match the fixture owner's individual feature graphs, rather than
          // unifying packages and starving its next PR of those cached variants.
          run('cargo', ['test', '-p', 'school-navigation', '--locked', '--no-run'], cwd);
          run('cargo', ['test', '-p', 'school-auth', '--lib', '--locked', '--no-run'], cwd);
          run('cargo', ['test', '-p', 'school-certificates', '--locked', '--no-run'], cwd);
        } else run('bash', ['scripts/test_school_database_suite.sh']);
      }
      if (suite === 'backend-admin') {
        run('cargo', ['fmt', '--all', '--', '--check'], path.join(root, suite));
        run('bash', ['scripts/test_backend_admin.sh'], root, prime ? {SCHOOLORBIT_COMPILE_ONLY:'true'} : {});
      }
      if (suite.startsWith('frontend-')) {
        run('npm', ['run', 'lint'], path.join(root, suite));
        run('npm', ['run', 'check'], path.join(root, suite), { PUBLIC_BACKEND_URL: process.env.PUBLIC_BACKEND_URL || 'http://localhost:3000', PUBLIC_VAPID_KEY: process.env.PUBLIC_VAPID_KEY || 'test', PUBLIC_API_URL: process.env.PUBLIC_API_URL || 'http://localhost:8080' });
        run('npm', ['run', suite === 'frontend-school' ? 'test:static' : 'test:unit'], path.join(root, suite));
        if (suite === 'frontend-school') run('npx', ['playwright', 'test', '--list', 'tests/e2e/login.spec.ts', 'tests/e2e/session-security.spec.ts'], path.join(root, suite), { E2E_SESSION_USERNAME: 'discovery-only', E2E_SESSION_PASSWORD: 'discovery-only', E2E_BASE_URL: 'http://127.0.0.1:4173', E2E_API_URL: 'http://127.0.0.1:3000' });
      }
    }
  } else throw new Error('Usage: scripts/pipeline plan [--release true --state FILE --scope COMPONENT] | verify --scope auto|full|SUITE | prime --scope BACKEND (internal, verified main only)');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
