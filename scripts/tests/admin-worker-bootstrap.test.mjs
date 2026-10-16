import assert from 'node:assert/strict';
import {mkdtempSync, readFileSync, existsSync, rmSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import test from 'node:test';
import {ADMIN_WORKER, adminWorkerExists, writeAdminBootstrap, prepareAdminWorker, removeAdminWorker} from '../lib/pipeline-admin-worker.mjs';
import {requireFrontendOnly, preparedGate} from '../lib/pipeline-admin-recreate.mjs';

const credentials = {accountId: 'a'.repeat(32), token: 'synthetic-token'};
const reply = (status, body) => async () => new Response(JSON.stringify(body), {status});

test('the deletion CLI refuses non-main, non-explicit and non-manual callers before any API request', () => {
  for (const override of [{GITHUB_REF: 'refs/heads/feature'}, {GITHUB_EVENT_NAME: 'pull_request'}, {ADMIN_WORKER_RECREATE: 'false'}]) {
    const result = spawnSync(process.execPath, [path.resolve(import.meta.dirname, '../lib/pipeline-admin-worker.mjs'), 'delete'], {
      encoding: 'utf8', env: {...process.env, GITHUB_REF: 'refs/heads/main', GITHUB_EVENT_NAME: 'workflow_dispatch',
        ADMIN_WORKER_RECREATE: 'true', CLOUDFLARE_ACCOUNT_ID: 'invalid-no-network', CLOUDFLARE_API_TOKEN: credentials.token, ...override}
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /requires the explicit main Operations action/);
    assert.ok(!(result.stdout + result.stderr).includes(credentials.token));
  }
});

test('explicit recreation deletes only the Admin script without forcing dependent resources and confirms absence', async () => {
  const calls = [];
  const results = [
    [200, {success: true, result: {default_environment: {script: {tag: 'existing'}}}}],
    [200, {success: true, result: null}],
    [404, {success: false, errors: [{code: 10090}]}]
  ];
  assert.equal(await removeAdminWorker({...credentials, wait: async () => {}, fetchImpl: async (url, options) => {
    calls.push({url, method: options.method || 'GET'});
    const [status, body] = results.shift();
    return new Response(JSON.stringify(body), {status});
  }}), true);
  assert.deepEqual(calls.map(call => call.method), ['GET', 'DELETE', 'GET']);
  assert.equal(calls[1].url, `https://api.cloudflare.com/client/v4/accounts/${credentials.accountId}/workers/scripts/${ADMIN_WORKER}`);
  assert.ok(calls.every(call => !call.url.includes('force=') && !call.url.includes('schoolorbit-frontend-school')));
});

test('an already absent Worker needs no deletion; deletion refusals do not prepare bootstrap files', async () => {
  assert.equal(await removeAdminWorker({...credentials, fetchImpl: reply(404, {success: false, errors: [{code: 10090}]})}), false);
  for (const status of [403, 409, 500]) {
    await assert.rejects(removeAdminWorker({...credentials, fetchImpl: async (url, options) => {
      return options.method === 'DELETE' ? new Response(JSON.stringify({success: false}), {status}) :
        new Response(JSON.stringify({success: true, result: {default_environment: {script: {tag: 'existing'}}}}));
    }}), /deletion refused/);
  }
});

test('recreation rejects queued School/backend releases and waits for the exact successful frontend build gate', () => {
  requireFrontendOnly({deploy: ['frontend-admin']});
  for (const deploy of [[], ['backend-admin'], ['frontend-admin', 'backend-admin'], ['frontend-admin', 'frontend-school']]) {
    assert.throws(() => requireFrontendOnly({deploy}), /only frontend-admin/);
  }
  const owner = {name: 'prepare / Prepare frontend-admin', conclusion: 'success'};
  const gate = {name: 'Pipeline gate', status: 'completed', conclusion: 'success'};
  assert.equal(preparedGate([]), false);
  assert.equal(preparedGate([owner, {...gate, status: 'in_progress'}]), false);
  assert.equal(preparedGate([owner, gate]), true);
  for (const jobs of [[owner, {...gate, conclusion: 'failure'}], [gate], [owner, gate, gate],
    [owner, gate, {name: 'prepare / Prepare backend-school', conclusion: 'success'}]]) {
    assert.throws(() => preparedGate(jobs));
  }
});

test('release preparation writes a bootstrap only for a confirmed absent Admin Worker', async t => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'admin-worker-prepare-'));
  t.after(() => rmSync(directory, {recursive: true, force: true}));
  const config = {name: ADMIN_WORKER, compatibility_date: '2025-09-15'};
  const options = {...credentials, directory, config};
  assert.equal(await prepareAdminWorker({...options,
    fetchImpl: reply(200, {success: true, result: {default_environment: {script: {tag: 'existing'}}}})}), false);
  assert.ok(!existsSync(path.join(directory, '.wrangler')));
  await assert.rejects(prepareAdminWorker({...options,
    fetchImpl: reply(403, {success: false, errors: [{code: 9109}]})}));
  assert.ok(!existsSync(path.join(directory, '.wrangler')));
  await assert.rejects(prepareAdminWorker({...options, config: {...config, name: 'schoolorbit-frontend-school'},
    fetchImpl: () => assert.fail('Unexpected School Worker lookup')}));
  assert.equal(await prepareAdminWorker({...options,
    fetchImpl: reply(404, {success: false, errors: [{code: 10090}]})}), true);
  assert.ok(existsSync(path.join(directory, '.wrangler/admin-bootstrap.json')));
});

test('existing Admin Worker is detected with a read-only request to its exact service', async () => {
  const exists = await adminWorkerExists({...credentials, fetchImpl: async (url, options) => {
    assert.equal(url, `https://api.cloudflare.com/client/v4/accounts/${credentials.accountId}/workers/services/${ADMIN_WORKER}`);
    assert.ok(options.method === undefined || options.method === 'GET');
    assert.equal(options.headers.Authorization, 'Bearer synthetic-token');
    return new Response(JSON.stringify({success: true, result: {default_environment: {script: {tag: 'fixture'}}}}));
  }});
  assert.equal(exists, true);
});

for (const code of [10007, 10090]) {
  test(`recognized missing-service code ${code} permits first-time creation`, async () => {
    assert.equal(await adminWorkerExists({...credentials,
      fetchImpl: reply(404, {success: false, errors: [{code}]})}), false);
  });
}

test('authorization failures, generic 404s and inconsistent metadata never permit creation', async () => {
  for (const [status, body] of [
    [401, {success: false, errors: [{code: 10007}]}],
    [403, {success: false, errors: [{code: 10090}]}],
    [404, {success: false, errors: [{code: 99999}]}],
    [404, {success: false, errors: [{code: 10007}, {code: 9109}]}],
    [500, {success: false, errors: [{code: 10007}]}],
    [200, {success: false, errors: [{code: 10007}]}],
    [200, {success: true, result: {}}]
  ]) {
    await assert.rejects(adminWorkerExists({...credentials, fetchImpl: reply(status, body)}), /lookup refused/);
  }
});

test('transport and parse failures fail closed without exposing credentials', async () => {
  for (const fetchImpl of [
    async () => { throw new Error('synthetic-token'); },
    async () => new Response('not JSON', {status: 404})
  ]) {
    await assert.rejects(adminWorkerExists({...credentials, fetchImpl}), error => {
      assert.equal(error.message, 'Cloudflare Admin Worker lookup unavailable');
      assert.ok(!error.message.includes(credentials.token));
      return true;
    });
  }
});

test('missing credentials or an invalid account stop before contacting Cloudflare', async () => {
  for (const config of [{...credentials, token: ''}, {...credentials, accountId: '../other'}]) {
    await assert.rejects(adminWorkerExists({...config, fetchImpl: () => assert.fail('Unexpected API call')}), /not configured/);
  }
});

test('bootstrap excludes application assets, routes, previews and private bindings and serves only 503', async t => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'admin-worker-bootstrap-'));
  t.after(() => rmSync(directory, {recursive: true, force: true}));
  const file = writeAdminBootstrap(directory, {name: ADMIN_WORKER, compatibility_date: '2025-09-15',
    build: {command: 'must never run'}, main: '/private/application.js', assets: {directory: '/private/assets'},
    vars: {PRIVATE: 'fixture-secret'}, routes: ['*.example.test/*'], workers_dev: true, preview_urls: true});
  const config = JSON.parse(readFileSync(file, 'utf8'));
  assert.deepEqual(config, {name: ADMIN_WORKER, main: 'admin-bootstrap.mjs', compatibility_date: '2025-09-15',
    workers_dev: false, preview_urls: false, routes: []});
  const worker = (await import(pathToFileURL(path.join(path.dirname(file), config.main)))).default;
  for (const pathname of ['/', '/login', '/api/v1/schools']) {
    const response = await worker.fetch(new Request('https://example.test' + pathname));
    assert.equal(response.status, 503);
    assert.equal(response.headers.get('Cache-Control'), 'no-store');
    assert.deepEqual(await response.json(), {error: 'maintenance'});
  }
});

test('a School Worker or an invalid compatibility date cannot be bootstrapped', t => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'admin-worker-owner-'));
  t.after(() => rmSync(directory, {recursive: true, force: true}));
  for (const config of [{name: 'schoolorbit-frontend-school', compatibility_date: '2025-09-15'},
    {name: ADMIN_WORKER, compatibility_date: ''}]) {
    assert.throws(() => writeAdminBootstrap(directory, config), /Invalid Admin Worker/);
    assert.ok(!existsSync(path.join(directory, '.wrangler')));
  }
});
