import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import test from 'node:test';

test('network validation preserves an established TCP connection on success and configuration failure', { timeout: 60000 }, async (t) => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'pipeline-network-'));
  const suffix = `${process.pid}-${Math.random().toString(16).slice(2, 10)}`;
  const network = `pipeline-network-${suffix}`;
  const backend = `pipeline-backend-${suffix}`;
  const probe = `pipeline-dns-${suffix}`;
  const agent = new http.Agent({ keepAlive: true, maxSockets: 1 });
  const docker = (...args) => execFileSync('docker', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  t.after(() => {
    agent.destroy();
    for (const name of [probe, backend]) { try { docker('rm', '-f', '-v', name); } catch {} }
    try { docker('network', 'rm', network); } catch {}
    rmSync(temp, { recursive: true, force: true });
  });
  const image = 'public.ecr.aws/docker/library/nginx:stable-alpine@sha256:0985e772fb9f729e6fa0980da05fca5d9c468e870eed43071545afa9d2e27d94';
  docker('network', 'create', network);
  docker('run', '-d', '--name', backend, '--network', network,
    '--network-alias', 'backend-school', '--network-alias', 'schoolorbit-backend-school',
    '-p', '127.0.0.1::80', image);
  docker('run', '-d', '--name', probe, '--network', network, image);
  const binding = docker('port', backend, '80/tcp');
  assert.match(binding, /^127\.0\.0\.1:[0-9]+$/);
  const port = Number(binding.split(':')[1]);
  const request = () => new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path: '/', method: 'HEAD', agent }, (res) => {
      const result = { status: res.statusCode, socket: res.socket, reused: req.reusedSocket };
      res.resume();
      res.once('end', () => resolve(result));
    });
    req.once('error', reject);
    req.setTimeout(5000, () => req.destroy(new Error('fixture request timed out')));
    req.end();
  });
  let before;
  for (let attempt = 0; attempt < 30; attempt++) {
    try { before = await request(); break; } catch (error) {
      if (attempt === 29) throw error;
      await delay(100);
    }
  }
  assert.equal(before.status, 200);
  const address = () => JSON.parse(docker('inspect', '--format', '{{json .NetworkSettings.Networks}}', backend))[network].IPAddress;
  const originalAddress = address();
  const bin = path.join(temp, 'bin');
  mkdirSync(bin);
  // Exercise the same gate with Docker's disposable network; translate only Podman's
  // read-only `container exists` command. Network mutation would still execute and fail this test.
  writeFileSync(path.join(bin, 'podman'), '#!/usr/bin/env bash\nif [[ $1 == container && $2 == exists ]]; then exec docker container inspect "$3" >/dev/null; fi\nexec docker "$@"\n', { mode: 0o755 });
  const validate = (alias) => spawnSync('bash', ['-c',
    'source "$1"; schoolorbit_validate_container_network_aliases "$2" "$3" backend-school "$4" "$5"',
    '_', 'scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh', network, backend, alias, probe],
    { encoding: 'utf8', env: { ...process.env, PATH: `${bin}:${process.env.PATH}` } });
  assert.equal(validate('schoolorbit-backend-school').status, 0);
  assert.notEqual(validate('missing-alias').status, 0);
  assert.equal(address(), originalAddress);
  const after = await request();
  assert.equal(after.status, 200);
  assert.equal(after.reused, true);
  assert.equal(after.socket, before.socket, 'validation must preserve the actual connection, not merely restore reachability');
});
