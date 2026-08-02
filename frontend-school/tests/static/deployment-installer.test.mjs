import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '../../..');

test('the resolved production topology has one owner and private backend ports', async () => {
  const { stdout } = await execFileAsync(
    'docker',
    [
      'compose',
      '--env-file',
      'scripts/tests/installer/fixtures/runtime.env',
      '-f',
      'podman-compose.yml',
      'config',
      '--format',
      'json'
    ],
    { cwd: repoRoot }
  );
  const topology = JSON.parse(stdout);

  for (const standalone of [
    'backend-admin/docker-compose.yml',
    'backend-school/docker-compose.yml'
  ]) {
    await assert.rejects(access(path.join(repoRoot, standalone)));
  }
  assert.deepEqual(Object.keys(topology.services).sort(), [
    'backend-admin',
    'backend-school',
    'clamd',
    'nginx'
  ]);
  assert.equal(topology.networks['schoolorbit-net'].name, 'schoolorbit-web');
  assert.equal(
    topology.networks['file-platform-internal'].name,
    'schoolorbit-file-platform-internal'
  );
  assert.equal(topology.networks['clamav-egress'].name, 'schoolorbit-clamav-egress');
  assert.equal(topology.volumes.clamav_signatures.name, 'schoolorbit-clamav-signatures');

  for (const [service, target] of [
    ['backend-admin', 8080],
    ['backend-school', 8081]
  ]) {
    assert.deepEqual(topology.services[service].ports, [
      {
        mode: 'ingress',
        host_ip: '127.0.0.1',
        target,
        published: String(target),
        protocol: 'tcp'
      }
    ]);
  }
});

test('the proxy renderer substitutes only a validated base domain', async (t) => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-'));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const output = path.join(temporary, 'school.conf');

  await execFileAsync(
    path.join(repoRoot, 'scripts/render_nginx_config.sh'),
    [path.join(repoRoot, 'nginx-configs/school-api.conf.template'), output, 'example.test']
  );

  const rendered = await readFile(output, 'utf8');
  assert.match(rendered, /server_name school-api\.example\.test;/);
  assert.match(rendered, /\(\[\\w-\]\+\\\.\)\?example\\\.test/);
  assert.match(rendered, /ssl_certificate \/etc\/nginx\/ssl\/schoolorbit-origin\.pem;/);
  assert.match(rendered, /ssl_certificate_key \/etc\/nginx\/ssl\/schoolorbit-origin\.key;/);
  assert.doesNotMatch(rendered, /\$\{BASE_DOMAIN(?:_REGEX)?\}/);
  assert.doesNotMatch(rendered, /schoolorbit\.app/);
});

test('the proxy renderer rejects an invalid domain without replacing its output', async (t) => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-invalid-'));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const output = path.join(temporary, 'school.conf');
  await writeFile(output, 'known-good\n');

  await assert.rejects(
    execFileAsync(path.join(repoRoot, 'scripts/render_nginx_config.sh'), [
      path.join(repoRoot, 'nginx-configs/school-api.conf.template'),
      output,
      'Example Test'
    ]),
    (error) => error.code === 64 && error.stderr === 'Invalid base domain\n'
  );
  assert.equal(await readFile(output, 'utf8'), 'known-good\n');
});
