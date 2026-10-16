import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
const script = path.join(repo, 'scripts/lib/pipeline-remote/admin-accounts.sh');
const imageDigest = 'sha256:' + 'a'.repeat(64);
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'admin-accounts-'));
  t.after(() => rmSync(root, {recursive: true, force: true}));
  mkdirSync(path.join(root, 'postgresql'));
  mkdirSync(path.join(root, 'bin'));
  writeFileSync(path.join(root, 'postgresql/ca.crt'), 'fixture CA');
  const values = {
    ADMIN_MIGRATION_DATABASE_URL: 'postgresql://fixture:synthetic@db/admin?sslmode=verify-full',
    RESEND_API_KEY: 're_synthetic-mail-key',
    RESEND_FROM: "SchoolOrbit's mail <noreply@mail.example.test>"
  };
  const encode = value => "'" + value.replaceAll('\\', '\\\\').replaceAll("'", "\\'") + "'";
  const envFile = path.join(root, '.env');
  writeFileSync(envFile, Object.entries(values).map(([key, value]) => key + '=' + encode(value)).join('\n') + '\n', {mode: 0o600});
  const log = path.join(root, 'commands');
  writeFileSync(path.join(root, 'bin/podman'), `#!/usr/bin/env bash
set -eu
printf '%s\\n' "$*" >> "$COMMAND_LOG"
case "$1" in
  login) test "$(cat)" = "$EXPECTED_REGISTRY_TOKEN" ;;
  pull) ;;
  run)
    test "$ADMIN_MIGRATION_DATABASE_URL" = "$EXPECTED_DATABASE_URL"
    test "$RESEND_API_KEY" = "$EXPECTED_MAIL_KEY"
    test "$RESEND_FROM" = "$EXPECTED_SENDER"
    test "$ADMIN_BOOTSTRAP_PASSWORD" = "$EXPECTED_BOOTSTRAP_PASSWORD"
    exit "$ACCOUNT_EXIT"
    ;;
  *) exit 99 ;;
esac
`, {mode: 0o700});
  const bootstrapPassword = "fixture-password'$(touch NEVER_CREATE)";
  const env = {...process.env, PATH: path.join(root, 'bin') + ':' + process.env.PATH,
    SCHOOLORBIT_STACK_ROOT: root, BASE_DOMAIN: 'example.test', BACKEND_IMAGE_DIGEST: imageDigest,
    REGISTRY_USER: 'fixture-actor', REGISTRY_TOKEN: 'synthetic-registry-token',
    ADMIN_BOOTSTRAP_EMAIL: 'operator@example.test', ADMIN_BOOTSTRAP_PASSWORD: bootstrapPassword,
    EXPECTED_DATABASE_URL: values.ADMIN_MIGRATION_DATABASE_URL, EXPECTED_MAIL_KEY: values.RESEND_API_KEY,
    EXPECTED_SENDER: values.RESEND_FROM, EXPECTED_BOOTSTRAP_PASSWORD: bootstrapPassword,
    EXPECTED_REGISTRY_TOKEN: 'synthetic-registry-token', COMMAND_LOG: log, ACCOUNT_EXIT: '0'};
  return {root, env, envFile, log, values,
    run: (mode, overrides = {}) => spawnSync('bash', [script, mode], {cwd: root, env: {...env, ...overrides}, encoding: 'utf8'})};
}

test('first-operator preflight pulls an immutable image and passes private values only by environment name', t => {
  const f = fixture(t);
  const result = f.run('--check-deploy-inputs');
  assert.equal(result.status, 0, result.stderr);
  const commands = readFileSync(f.log, 'utf8');
  assert.match(commands, /login ghcr.io -u fixture-actor --password-stdin/);
  assert.ok(commands.includes('pull ghcr.io/akephisit/schoolorbit-backend-admin@' + imageDigest));
  assert.match(commands, /--env ADMIN_BOOTSTRAP_PASSWORD /);
  assert.match(commands, /--network schoolorbit-web/);
  assert.ok(commands.endsWith('/app/create_admin --check-deploy-inputs\n'));
  assert.doesNotMatch(commands, /--env-file|--env [A-Z_]+=/);
  for (const secret of [...Object.values(f.values), f.env.REGISTRY_TOKEN, f.env.ADMIN_BOOTSTRAP_PASSWORD]) {
    assert.ok(!commands.includes(secret));
    assert.ok(!(result.stdout + result.stderr).includes(secret));
  }
  assert.ok(!existsSync(path.join(f.root, 'NEVER_CREATE')));
});

for (const mode of ['--prepare-release', '--check-release-ready']) {
  test(`${mode} uses the selected image without changing running containers`, t => {
    const f = fixture(t);
    assert.equal(f.run(mode).status, 0);
    const commands = readFileSync(f.log, 'utf8').trim().split('\n');
    assert.equal(commands.length, 1);
    assert.ok(commands[0].startsWith('run --rm '));
    assert.ok(commands[0].endsWith('/app/create_admin ' + mode));
  });
}

test('unavailable accounts stop release and propagate the CLI failure', t => {
  const f = fixture(t);
  assert.equal(f.run('--check-release-ready', {ACCOUNT_EXIT: '73'}).status, 73);
});

test('invalid modes, image identities and missing private settings fail before contacting Podman', t => {
  const f = fixture(t);
  assert.notEqual(f.run('--bootstrap').status, 0);
  assert.notEqual(f.run('--check-deploy-inputs', {BACKEND_IMAGE_DIGEST: 'latest'}).status, 0);
  writeFileSync(f.envFile, "ADMIN_MIGRATION_DATABASE_URL='private-fixture'\n");
  assert.notEqual(f.run('--check-deploy-inputs').status, 0);
  assert.ok(!existsSync(f.log));
});
