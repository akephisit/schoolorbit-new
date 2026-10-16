import assert from 'node:assert/strict';
import {mkdtempSync, readFileSync, existsSync, rmSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import test from 'node:test';
import {ADMIN_WORKER, adminWorkerExists, writeAdminBootstrap, prepareAdminWorker, removeAdminWorker, retireAdminAlias} from '../lib/pipeline-admin-worker.mjs';
import {requireFrontendOnly, preparedGate} from '../lib/pipeline-admin-recreate.mjs';

const credentials = {accountId: 'a'.repeat(32), token: 'synthetic-token'};
const reply = (status, body) => async () => new Response(JSON.stringify(body), {status});

// Workers Domains returns an opaque ID; actual records can be longer than account IDs.
const alias = {id: 'b'.repeat(40), hostname: 'admin.example.test', service: ADMIN_WORKER, environment: 'production'};

test('alias retirement accepts bounded opaque hexadecimal IDs without changing its exact owner scope', async () => {
  for (const length of [32, 40, 64]) {
    let reads = 0;
    const domain = {...alias, id: 'b'.repeat(length)};
    const calls = [];
    assert.equal(await retireAdminAlias({...credentials, baseDomain: 'example.test', fetchImpl: async (url, request) => {
      calls.push({url, method: request.method});
      if (request.method === 'DELETE') return Response.json({success: true});
      return Response.json({success: true, result: reads++ ? [] : [domain]});
    }}), true);
    assert.ok(calls[1].url.endsWith('/workers/domains/' + domain.id));
  }
});

test('alias preflight verifies the same ownership without mutating Cloudflare', async () => {
  let reads = 0;
  assert.equal(await retireAdminAlias({...credentials, baseDomain: 'example.test', checkOnly: true,
    fetchImpl: async (url, request) => {
      assert.equal(request.method, 'GET');
      reads++;
      return Response.json({success: true, result: [alias]});
    }}), true);
  assert.equal(reads, 1);
});

test('alias retirement detaches only its confirmed Admin production owner and verifies absence', async () => {
  const calls = [];
  const responses = [{success: true, result: [alias]}, {success: true}, {success: true, result: []}];
  assert.equal(await retireAdminAlias({...credentials, baseDomain: 'example.test', fetchImpl: async (url, options) => {
    calls.push({url, method: options.method});
    return Response.json(responses.shift());
  }}), true);
  const root = `https://api.cloudflare.com/client/v4/accounts/${credentials.accountId}/workers/domains`;
  assert.deepEqual(calls, [
    {url: root + '?hostname=admin.example.test', method: 'GET'},
    {url: root + '/' + alias.id, method: 'DELETE'},
    {url: root + '?hostname=admin.example.test', method: 'GET'}
  ]);
  assert.ok(calls.every(call => !call.url.includes('/zones/') && !call.url.includes('/workers/scripts/')));
});

test('an absent alias is idempotent and a no-content deletion is accepted only after confirmed absence', async () => {
  const options = {...credentials, baseDomain: 'example.test'};
  let calls = 0;
  assert.equal(await retireAdminAlias({...options, fetchImpl: async () => {
    calls++;
    return Response.json({success: true, result: []});
  }}), false);
  assert.equal(calls, 1);
  let reads = 0;
  assert.equal(await retireAdminAlias({...options, fetchImpl: async (url, request) => {
    if (request.method === 'DELETE') return new Response(null, {status: 204});
    return Response.json({success: true, result: reads++ ? [] : [alias]});
  }}), true);
});

test('unexpected aliases, environments, owners and incomplete lookup results never authorize deletion', async () => {
  for (const body of [
    {success: true, result: [{...alias, hostname: 'school.example.test'}]},
    {success: true, result: [{...alias, service: 'schoolorbit-frontend-school'}]},
    {success: true, result: [{...alias, environment: 'staging'}]},
    {success: true, result: [{...alias, id: '../other'}]},
    {success: true, result: [{...alias, id: 'b'.repeat(31)}]},
    {success: true, result: [{...alias, id: 'b'.repeat(65)}]},
    {success: true, result: [null]},
    {success: true, result: [alias, alias]},
    {success: true, result: [] , result_info: {total_count: 1}},
    {success: true, result: [alias], result_info: {total_count: 'unknown'}},
    {success: true, result: {}},
    {success: false, result: []}
  ]) {
    await assert.rejects(retireAdminAlias({...credentials, baseDomain: 'example.test', fetchImpl: async (url, request) => {
      assert.equal(request.method, 'GET', 'unsafe ownership must never reach deletion');
      return Response.json(body);
    }}));
  }
});

test('alias retirement fails closed on unavailable lookup or deletion without leaking credentials', async () => {
  for (const fetchImpl of [
    async () => { throw new Error(credentials.token); },
    async () => new Response(credentials.token),
    reply(403, {success: false}),
    async (url, request) => request.method === 'DELETE' ? Response.json({success: false}) :
      Response.json({success: true, result: [alias]})
  ]) {
    await assert.rejects(retireAdminAlias({...credentials, baseDomain: 'example.test', fetchImpl}), error => {
      assert.ok(!error.message.includes(credentials.token));
      return true;
    });
  }
});

test('alias deletion waits for bounded absence confirmation without deleting again', async () => {
  let deletes = 0;
  let waits = 0;
  await assert.rejects(retireAdminAlias({...credentials, baseDomain: 'example.test', wait: async () => { waits++; },
    fetchImpl: async (url, request) => {
      if (request.method === 'DELETE') { deletes++; return Response.json({success: true}); }
      return Response.json({success: true, result: [alias]});
    }}), /not yet confirmed/);
  assert.equal(deletes, 1);
  assert.equal(waits, 2);
});

test('alias retirement refuses invalid configuration and non-production callers before contacting Cloudflare', async () => {
  for (const override of [{accountId: '../other'}, {token: ''}, {baseDomain: '*.example.test'}, {baseDomain: 'https://example.test'}]) {
    await assert.rejects(retireAdminAlias({...credentials, baseDomain: 'example.test', ...override,
      fetchImpl: () => assert.fail('Unexpected network request')}));
  }
  for (const override of [{GITHUB_REF: 'refs/heads/feature'}, {GITHUB_EVENT_NAME: 'pull_request'}]) {
    const result = spawnSync(process.execPath, [path.resolve(import.meta.dirname, '../lib/pipeline-admin-worker.mjs'), 'retire-alias'], {
      encoding: 'utf8', env: {...process.env, GITHUB_REF: 'refs/heads/main', GITHUB_EVENT_NAME: 'workflow_dispatch', ...override}
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /requires the main production Pipeline/);
  }
});

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
