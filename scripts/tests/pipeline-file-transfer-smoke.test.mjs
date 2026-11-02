import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const smoke = readFileSync(new URL('../smoke_test.sh', import.meta.url), 'utf8');
const helpers = smoke.slice(smoke.indexOf('header_value() {'), smoke.indexOf('expect_body_contains() {'));
const probe = smoke.slice(smoke.indexOf('# Staff file-transfer smoke start'), smoke.indexOf('# Staff file-transfer smoke end'));
const id = '00000000-0000-0000-0000-000000000010';
const user = '00000000-0000-0000-0000-000000000011';
const code = 'ABCD012345';
const csrf = 'synthetic-private-csrf';
const cookie = 'synthetic-private-cookie';
const origin = 'https://sandbox.example.test';
const snapshot = () => ({ protocolVersion: 3, limits: { maxOutgoingTransfers: 6 }, device: { id, userId: user, displayName: 'บุคลากรทดสอบ', label: 'SchoolOrbit smoke' }, pairingCode: code, expiresAt: '2030-01-01T00:00:00Z', peers: [], pairs: [] });

function execute(args, env) {
 return new Promise((resolve, reject) => {
  const child = spawn('bash', args, { env, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  child.stdout.setEncoding('utf8').on('data', value => stdout += value);
  child.stderr.setEncoding('utf8').on('data', value => stderr += value);
  child.on('error', reject);
  child.on('close', status => resolve({ status, stdout, stderr }));
 });
}
async function fixture(t, mode = 'success') {
 const observations = [];
 let deletes = 0;
 const sockets = new Set();
 const server = http.createServer(async (request, response) => {
  let body = '';
  for await (const chunk of request) body += chunk;
  observations.push({ method: request.method, url: request.url, headers: request.headers, body });
  assert.equal(request.headers.origin, origin);
  assert.equal(request.headers['x-school-subdomain'], 'sandbox');
  if (request.method !== 'GET') assert.equal(request.headers['x-csrf-token'], csrf);
  const headers = { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': origin, 'Access-Control-Allow-Credentials': 'true', 'Cache-Control': 'no-store' };
  if (request.method === 'POST') {
   assert.equal(request.url, '/api/staff/file-transfer/devices');
   assert.deepEqual(JSON.parse(body), { label: 'SchoolOrbit smoke', protocolVersion: 3 });
   if (mode === 'post-transport-failure') { request.socket.destroy(); return; }
   const registered = snapshot();
   if (mode === 'bad-registration') delete registered.pairingCode;
   if (mode === 'registration-v1') registered.protocolVersion = 1;
   if (mode === 'registration-bad-limits') registered.limits.maxOutgoingTransfers = 999;
   response.writeHead(mode === 'post-http-failure' ? 503 : 200, headers);
   response.end(JSON.stringify({ success: true, data: registered }));
   return;
  }
  if (request.method === 'DELETE') {
   assert.equal(request.url, `/api/staff/file-transfer/devices/${id}`);
   deletes++;
   response.writeHead(mode === 'cleanup-404' ? 404 : mode === 'cleanup-retry' && deletes === 1 ? 503 : 200, headers);
   response.end('{"success":true,"data":{}}');
   return;
  }
  assert.equal(request.method, 'GET');
  assert.equal(request.url, `/api/staff/file-transfer/devices/${id}/events`);
  headers['Content-Type'] = mode === 'wrong-content-type' ? 'application/json' : 'text/event-stream';
  if (mode === 'missing-cors') delete headers['Access-Control-Allow-Credentials'];
  if (mode === 'missing-no-store') delete headers['Cache-Control'];
  const ready = snapshot();
  if (mode === 'wrong-id') ready.device.id = user;
  if (mode === 'stream-v1') ready.protocolVersion = 1;
  if (mode === 'stream-bad-limits') ready.limits.maxOutgoingTransfers = 4;
  let text = `event: ${mode === 'wrong-event' ? 'notification' : 'file_transfer'}\ndata: ${JSON.stringify({ type: 'ready', data: ready })}\n\n`;
  if (mode === 'missing-ready') text = ': heartbeat\n\n';
  if (mode === 'malformed-ready') text = 'event: file_transfer\ndata: {invalid}\n\n';
  if (mode === 'incomplete-frame') text = text.trimEnd();
  if (mode === 'session-invalid') text = 'event: file_transfer\ndata: {"type":"session_invalid","data":{}}\n\n' + text;
  response.writeHead(mode === 'stream-http-failure' ? 503 : 200, headers);
  if (mode === 'stream-transport-failure') { response.write('event: file_transfer\n'); request.socket.destroy(); return; }
  if (mode === 'fragmented') {
   const pretty = JSON.stringify({ type: 'ready', data: ready }, null, 2).split('\n').map(line => `data: ${line}\r\n`).join('');
   const bytes = Buffer.from(`: comment\r\n\r\nevent: file_transfer\r\n${pretty}\r\n`);
   for (let offset = 0; offset < bytes.length; offset += 7) {
    response.write(bytes.subarray(offset, offset + 7));
    await new Promise(resolve => setTimeout(resolve, 1));
   }
   response.end();
  } else if (mode === 'bounded-timeout') response.write(text);
  else response.end(text);
 });
 server.on('connection', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
 await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
 t.after(async () => { for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); });
 const directory = mkdtempSync(path.join(os.tmpdir(), 'schoolorbit-transfer-smoke-'));
 t.after(() => rmSync(directory, { recursive: true, force: true }));
 const cookiePath = path.join(directory, 'cookies');
 writeFileSync(cookiePath, `# Netscape HTTP Cookie File\n127.0.0.1\tFALSE\t/\tFALSE\t0\tfixture_session\t${cookie}\n`, { mode: 0o600 });
 const program = `failures=0
pass() { printf 'PASS %s\\n' "$1"; }
fail() { failures=$((failures + 1)); printf 'FAIL %s\\n' "$1" >&2; }
${helpers}
${probe}
cookie_jar=$1
tmp_dir=$2
csrf_token=$3
file_transfer_device_id=
file_transfer_status=
school_api_curl_options=()
trap cleanup_file_transfer_device EXIT
run_file_transfer_smoke
if ((failures > 0)); then exit 1; fi
exit 0`;
 const result = await execute(['-euc', program, 'file-transfer-fixture', cookiePath, directory, csrf], { ...process.env, SMOKE_API_URL: `http://127.0.0.1:${server.address().port}`, SMOKE_ORIGIN: origin, SMOKE_SUBDOMAIN: 'sandbox', SMOKE_TIMEOUT_SECONDS: '2', SMOKE_DIRECT_BACKEND: 'false' });
 for (const secret of [code, csrf, cookie, snapshot().device.displayName]) assert.ok(!(result.stdout + result.stderr).includes(secret), 'probe output must not expose private response or credential values');
 return { ...result, observations, deletes };
}

test('file-transfer deployment probe validates complete typed ready and cleans only its own device', async t => {
 const result = await fixture(t);
 assert.equal(result.status, 0, result.stdout + result.stderr);
 assert.match(result.stdout, /typed ready event flushed within five seconds/);
 assert.deepEqual(result.observations.map(({ method }) => method), ['POST', 'GET', 'DELETE']);
 assert.equal(result.deletes, 1);
});
test('file-transfer deployment probe accepts comments, multiline data and fragmented UTF-8', async t => {
 const result = await fixture(t, 'fragmented');
 assert.equal(result.status, 0, result.stdout + result.stderr);
 assert.equal(result.deletes, 1);
});
test('file-transfer deployment probe accepts curl timeout only after a flushed ready frame', async t => {
 const result = await fixture(t, 'bounded-timeout');
 assert.equal(result.status, 0, result.stdout + result.stderr);
 assert.equal(result.deletes, 1);
});
test('file-transfer deployment probe refuses bad stream state and still cleans the known binding', async t => {
 for (const mode of ['stream-v1', 'stream-bad-limits', 'missing-ready', 'wrong-id', 'wrong-event', 'malformed-ready', 'incomplete-frame', 'session-invalid', 'stream-http-failure', 'stream-transport-failure', 'wrong-content-type', 'missing-cors', 'missing-no-store']) {
  await t.test(mode, async child => {
   const result = await fixture(child, mode);
   assert.equal(result.status, 1, mode + result.stdout + result.stderr);
   assert.equal(result.deletes, 1);
  });
 }
});
test('file-transfer deployment probe owns cleanup immediately after a known registration identity', async t => {
 const result = await fixture(t, 'bad-registration');
 assert.equal(result.status, 1, result.stdout + result.stderr);
 assert.deepEqual(result.observations.map(({ method }) => method), ['POST', 'DELETE']);
});
test('file-transfer deployment probe preserves parent failure count for registration transport and HTTP errors', async t => {
 for (const mode of ['post-transport-failure', 'post-http-failure']) {
  await t.test(mode, async child => {
   const result = await fixture(child, mode);
   assert.equal(result.status, 1, result.stdout + result.stderr);
   assert.equal(result.deletes, 0);
   assert.equal(result.observations.length, 1);
  });
 }
});
test('file-transfer deployment probe accepts cleanup 404 and retries a failed cleanup on EXIT', async t => {
 const gone = await fixture(t, 'cleanup-404');
 assert.equal(gone.status, 0, gone.stdout + gone.stderr);
 assert.equal(gone.deletes, 1);
 const retried = await fixture(t, 'cleanup-retry');
 assert.equal(retried.status, 1, retried.stdout + retried.stderr);
 assert.equal(retried.deletes, 2);
});

test('file-transfer deployment probe refuses old protocol and invalid registration limits after binding cleanup', async t => {
 for (const mode of ['registration-v1', 'registration-bad-limits']) {
  await t.test(mode, async child => {
   const result = await fixture(child, mode);
   assert.equal(result.status, 1, result.stdout + result.stderr);
   assert.deepEqual(result.observations.map(({ method }) => method), ['POST', 'DELETE']);
  });
 }
});
