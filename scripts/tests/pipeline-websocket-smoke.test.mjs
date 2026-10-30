import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const smoke = readFileSync(new URL('../smoke_test.sh', import.meta.url), 'utf8');
const probeSource = smoke.slice(smoke.indexOf('timetable_websocket_smoke() {'), smoke.indexOf('# Timetable WebSocket read-only smoke end'));
const year = '00000000-0000-0000-0000-000000000001';
const term = '00000000-0000-0000-0000-000000000002';
const draft = '00000000-0000-0000-0000-000000000003';
const connectionId = '00000000-0000-0000-0000-000000000004';
const epoch = '00000000-0000-0000-0000-000000000005';
const session = 'synthetic-private-session';
const probeToken = 'e'.repeat(64);
const origin = 'https://sandbox.example.test';
const maxMessageBytes = 64 * 1024 * 1024;
const nginxImage = 'public.ecr.aws/docker/library/nginx:stable-alpine@sha256:0985e772fb9f729e6fa0980da05fca5d9c468e870eed43071545afa9d2e27d94';
const workspace = () => ({
 version: { id: draft, academicYearId: year, academicTermId: term, status: 'draft' },
 latestDeliveryVersionId: epoch, totalDraftBlockCount: 0, summary: {},
 ...Object.fromEntries(['sourceIssues', 'bellPeriods', 'blocks', 'ordinaryDemands', 'synchronizedDemands', 'learningGroups', 'homerooms', 'rooms', 'staff', 'offeringSubjectGroups'].map(key => [key, []]))
});
const sync = () => ({ epoch, seq: 0, type: 'DraftStateSync', payload: {
 connectionId, workspace: workspace(), users: [{ userId: epoch, name: 'Synthetic staff', color: '#123456', connections: [{ connectionId, context: null, anchor: null }] }], reservations: []
} });
const frameHeader = (opcode, length, final = true) => {
 const first = (final ? 128 : 0) | opcode;
 if (length < 126) return Buffer.from([first, length]);
 if (length <= 65535) return Buffer.from([first, 126, length >> 8, length & 255]);
 const header = Buffer.alloc(10);
 header[0] = first;
 header[1] = 127;
 header.writeBigUInt64BE(BigInt(length), 2);
 return header;
};
const frame = (opcode, payload, final = true) => {
 const body = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
 return Buffer.concat([frameHeader(opcode, body.length, final), body]);
};
function temporary(t) {
 const directory = mkdtempSync(path.join(os.tmpdir(), 'schoolorbit-ws-smoke-'));
 t.after(() => rmSync(directory, { recursive: true, force: true }));
 return directory;
}
function execute(command, args, options) {
 return new Promise((resolve, reject) => {
  const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  child.stdout.setEncoding('utf8').on('data', chunk => stdout += chunk);
  child.stderr.setEncoding('utf8').on('data', chunk => stderr += chunk);
  child.on('error', reject);
  child.on('close', status => resolve({ status, stdout, stderr }));
 });
}
async function runProbe(directory, api, { version = draft, ca = '', resolve = '', token = '', cookie = session, timeout = '3', httpWorkspace = workspace() } = {}) {
 const hostname = new URL(api).hostname;
 writeFileSync(path.join(directory, 'cookies'), `# Netscape HTTP Cookie File\n#HttpOnly_${hostname}\tFALSE\t/\tTRUE\t0\t__Host-schoolorbit_session\t${cookie}\n`, { mode: 0o600 });
 writeFileSync(path.join(directory, 'workspace'), JSON.stringify({ success: true, data: httpWorkspace }), { mode: 0o600 });
 return execute('bash', ['-euc', `pass() { printf 'PASS %s\\n' "$1"; }; fail() { printf 'FAIL %s\\n' "$1"; exit 1; }
${probeSource}
cookie_jar=$1
timetable_websocket_smoke fixture "$2" "$3" "$4" "$5"`, 'ws-fixture', path.join(directory, 'cookies'), year, term, version, path.join(directory, 'workspace')], {
  env: { ...process.env, SMOKE_API_URL: api, SMOKE_ORIGIN: origin, SMOKE_SUBDOMAIN: 'sandbox', SMOKE_DIRECT_BACKEND: api.startsWith('http:') ? 'true' : 'false', SMOKE_RESOLVE_IP: resolve, SMOKE_CA_CERT: ca, SMOKE_RELEASE_PROBE_TOKEN: token, SMOKE_TIMEOUT_SECONDS: timeout }
 });
}
async function fixtureServer(t, { mode = 'draft', tls, requiredProbe = '', fragmented = false } = {}) {
 const observations = { requests: [], commands: [], pings: 0, pongs: 0, closes: 0, errors: [], httpConnections: [], waitedForOversizedPayload: false };
 const server = tls ? https.createServer(tls) : http.createServer();
 server.on('request', (request, response) => {
  observations.httpConnections.push(request.headers.connection ?? '');
  response.writeHead(request.headers.cookie === `__Host-schoolorbit_session=${session}` ? 200 : 401);
  response.end('{}');
 });
 const sockets = new Set();
 server.on('connection', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
 server.on('upgrade', (request, socket, head) => {
  socket.on('error', () => {});
  observations.requests.push({ url: request.url, headers: request.headers });
  const query = new URL(request.url, 'http://fixture').searchParams;
  if (request.headers.cookie !== `__Host-schoolorbit_session=${session}` || request.headers.origin !== origin || request.headers['x-school-subdomain'] !== 'sandbox' || requiredProbe && request.headers['x-schoolorbit-release-probe'] !== requiredProbe) {
   socket.end('HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n');
   return;
  }
  if (mode === 'reject') { socket.end('HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n'); return; }
  if (mode === 'bad-handshake') { socket.end('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: invalid\r\n\r\n'); return; }
  const accept = createHash('sha1').update(request.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  let state = query.has('timetableVersionId') ? sync() : { type: 'StateSync', payload: { users: [], current_seq: 0 } };
  if (mode === 'wrong-version') state.payload.workspace.version.id = epoch;
  if (mode === 'invalid-state') state.payload.users = null;
  if (mode === 'private-lease') state.payload.reservations = [{ leaseToken: session }];
  if (mode === 'oversized-frame' || mode === 'oversized-fragments') {
   if (mode === 'oversized-fragments') socket.write(frame(1, '{', false));
   socket.write(frameHeader(mode === 'oversized-frame' ? 1 : 0, maxMessageBytes + (mode === 'oversized-frame' ? 1 : 0)));
   // The reader must reject from the header, without waiting for this absent payload.
   const guard = setTimeout(() => { observations.waitedForOversizedPayload = true; socket.destroy(); }, 2000);
   socket.once('close', () => clearTimeout(guard));
   return;
  }
  if (mode === 'masked-server') { socket.write(Buffer.from([0x81, 0x80])); return; }
  if (mode === 'large-thai') {
   state.payload.workspace.blocks = [{ id: epoch, note: 'ก'.repeat(6 * 1024 * 1024) }];
   state.payload.workspace.totalDraftBlockCount = 1;
  }
  const body = Buffer.from(JSON.stringify(state));
  if (mode === 'large-thai') assert.ok(body.length > 16 * 1024 * 1024 && body.length < maxMessageBytes);
  if (fragmented) {
   const middle = Math.floor(body.length / 2);
   socket.write(frame(1, body.slice(0, middle), false));
   socket.write(frame(9, 'server-heartbeat'));
   socket.write(frame(0, body.slice(middle)));
  } else socket.write(frame(1, body));
  let buffered = head;
  const consume = chunk => {
   buffered = Buffer.concat([buffered, chunk]);
   try {
    while (buffered.length >= 2) {
     const first = buffered[0], second = buffered[1];
     assert.equal(first & 0x80, 0x80);
     assert.equal(second & 0x80, 0x80, 'client frames must be masked');
     let length = second & 127, offset = 2;
     if (length === 126) { if (buffered.length < 4) return; length = buffered.readUInt16BE(2); offset = 4; }
     assert.notEqual(length, 127);
     if (buffered.length < offset + 4 + length) return;
     const mask = buffered.subarray(offset, offset + 4);
     const payload = Buffer.from(buffered.subarray(offset + 4, offset + 4 + length));
     for (let index = 0; index < payload.length; index++) payload[index] ^= mask[index % 4];
     buffered = buffered.subarray(offset + 4 + length);
     const opcode = first & 15;
     if (opcode === 1) {
      const command = JSON.parse(payload.toString());
      assert.ok(['ClientPresenceUpdate', 'ClientReplay'].includes(command.type), 'probe must never mutate timetable or claim a lease');
      observations.commands.push(command);
      // Current-sequence replay deliberately has no acknowledgement.
     } else if (opcode === 9) {
      observations.pings++;
      if (mode === 'no-pong') continue;
      socket.write(frame(10, payload));
     } else if (opcode === 10) observations.pongs++;
     else if (opcode === 8) {
      observations.closes++;
      socket.end(mode === 'close-without-reply' ? undefined : frame(8, payload));
     }
     else assert.fail('Unexpected client frame');
    }
   } catch (error) { observations.errors.push(error.message); socket.destroy(); }
  };
  socket.on('data', consume);
  if (head.length) consume(Buffer.alloc(0));
 });
 await new Promise(resolve => server.listen(0, '0.0.0.0', resolve));
 t.after(async () => { for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); });
 return { server, port: server.address().port, observations };
}

test('standalone smoke performs read-only draft and term exchanges with masked frames and fragmented server heartbeat', async t => {
 const directory = temporary(t);
 const { port, observations } = await fixtureServer(t, { fragmented: true });
 for (const version of [draft, '']) {
  const result = await runProbe(directory, `http://localhost:${port}`, { version });
  assert.equal(result.status, 0, result.stdout + result.stderr);
 }
 assert.deepEqual(observations.errors, []);
 assert.equal(observations.requests.length, 2);
 assert.deepEqual([...new URL(observations.requests[0].url, 'http://fixture').searchParams.keys()], ['academicTermId', 'timetableVersionId']);
 assert.deepEqual([...new URL(observations.requests[1].url, 'http://fixture').searchParams.keys()], ['academicTermId']);
 assert.deepEqual(observations.commands, [
  { type: 'ClientPresenceUpdate', payload: { context: { viewMode: 'wholeSchool', viewId: null, dayOfWeek: null }, anchor: null } },
  { type: 'ClientReplay', payload: { epoch, afterSeq: 0 } }
 ]);
 assert.equal(observations.pings, 2); assert.equal(observations.pongs, 2); assert.equal(observations.closes, 2);
});

test('standalone smoke accepts Thai draft events above sixteen MiB in single and fragmented frames', async t => {
 const directory = temporary(t);
 for (const fragmented of [false, true]) {
  await t.test(fragmented ? 'fragmented UTF-8 event' : 'single 64-bit-length frame', async child => {
   const { port, observations } = await fixtureServer(child, { mode: 'large-thai', fragmented });
   const result = await runProbe(directory, `http://localhost:${port}`, { timeout: '10' });
   assert.equal(result.status, 0, result.stdout + result.stderr);
   assert.deepEqual(observations.errors, []);
   assert.equal(observations.commands.length, 2);
   assert.equal(observations.pings, 1);
   assert.equal(observations.pongs, fragmented ? 1 : 0);
   assert.equal(observations.closes, 1);
  });
 }
});

test('standalone smoke rejects declared frame and cumulative event overflow before reading payloads', async t => {
 const directory = temporary(t);
 for (const mode of ['oversized-frame', 'oversized-fragments']) {
  await t.test(mode, async child => {
   const { port, observations } = await fixtureServer(child, { mode });
   const result = await runProbe(directory, `http://localhost:${port}`, { timeout: '10' });
   assert.equal(result.status, 1);
   assert.match(result.stdout, /^FAIL fixture handshake/);
   assert.doesNotMatch(result.stdout + result.stderr, /synthetic-private|00000000|Traceback|localhost/);
   assert.equal(observations.waitedForOversizedPayload, false, 'must refuse the header before the fixture payload guard fires');
   assert.equal(observations.commands.length, 0);
   assert.deepEqual(observations.errors, []);
  });
 }
});

test('standalone smoke requires the close reply and reports only bounded failure phases', async t => {
 const directory = temporary(t);
 for (const [mode, phase, status] of [
  ['reject', 'handshake_status', ' status=403'],
  ['bad-handshake', 'handshake_headers', ''],
  ['invalid-state', 'state', ''],
  ['no-pong', 'pong', ''],
  ['close-without-reply', 'close', '']
 ]) {
  await t.test(mode, async child => {
   const { port, observations } = await fixtureServer(child, { mode });
   const result = await runProbe(directory, `http://localhost:${port}`, { timeout: '0.4' });
   assert.equal(result.status, 1);
   assert.equal(result.stderr, `WebSocket acceptance failed phase=${phase}${status}\n`);
   assert.doesNotMatch(result.stdout + result.stderr, /synthetic-private|00000000|Traceback|leaseToken|Synthetic staff|localhost/);
   if (mode === 'close-without-reply') {
    assert.equal(observations.pings, 1);
    assert.equal(observations.closes, 1, 'EOF after the client close must not count as the server close reply');
   }
   assert.deepEqual(observations.errors, []);
  });
 }
});

test('standalone smoke fails closed for invalid upgrades, scope, frames, private leases, credentials and stalled pong without exposing payloads', async t => {
 const directory = temporary(t);
 for (const mode of ['reject', 'bad-handshake', 'wrong-version', 'invalid-state', 'private-lease', 'masked-server', 'no-pong']) {
  await t.test(mode, async child => {
   const { port, observations } = await fixtureServer(child, { mode });
   const result = await runProbe(directory, `http://localhost:${port}`, { timeout: '0.4' });
   assert.equal(result.status, 1, mode);
   assert.match(result.stdout, /^FAIL fixture handshake/);
   assert.doesNotMatch(result.stdout + result.stderr, /synthetic-private|00000000|Traceback|leaseToken|Synthetic staff|localhost/);
   assert.deepEqual(observations.errors, []);
  });
 }
 const { port, observations } = await fixtureServer(t);
 const unauthorized = await runProbe(directory, `http://localhost:${port}`, { cookie: 'incorrect-private-cookie' });
 assert.equal(unauthorized.status, 1); assert.equal(observations.commands.length, 0);
 const invalidHttp = await runProbe(directory, `http://localhost:${port}`, { httpWorkspace: { ...workspace(), version: { ...workspace().version, academicTermId: epoch } } });
 assert.equal(invalidHttp.status, 1); assert.equal(observations.requests.length, 1, 'invalid authoritative HTTP scope must prevent upgrade');
});

function certificate(directory) {
 const key = path.join(directory, 'key.pem'), cert = path.join(directory, 'cert.pem');
 execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert, '-days', '1', '-subj', '/CN=school-api.example.test', '-addext', 'subjectAltName=DNS:school-api.example.test'], { stdio: 'ignore' });
 return { key: readFileSync(key), cert: readFileSync(cert), certPath: cert };
}

test('resolve-pinned TLS preserves the intended Host/SNI and requires trusted matching certificates', async t => {
 const directory = temporary(t), tls = certificate(directory);
 const { port, observations } = await fixtureServer(t, { tls, requiredProbe: probeToken });
 const api = `https://school-api.example.test:${port}`;
 const successful = await runProbe(directory, api, { resolve: '127.0.0.1', ca: tls.certPath, token: probeToken });
 assert.equal(successful.status, 0, successful.stdout + successful.stderr);
 assert.equal(observations.requests[0].headers.host, `school-api.example.test:${port}`);
 const untrusted = await runProbe(directory, api, { resolve: '127.0.0.1', token: probeToken });
 assert.equal(untrusted.status, 1);
 const wrongHost = await runProbe(directory, `https://wrong.example.test:${port}`, { resolve: '127.0.0.1', ca: tls.certPath, token: probeToken });
 assert.equal(wrongHost.status, 1);
 assert.equal(observations.requests.length, 1, 'TLS denial must happen before authenticated headers are sent');
});

test('real Nginx maintenance probe upgrades authenticated read-only draft traffic while public traffic remains blocked', async t => {
 const directory = temporary(t), tls = certificate(directory);
 const { port: upstreamPort, observations } = await fixtureServer(t);
 const docker = (...args) => execFileSync('docker', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
 const suffix = `${process.pid}-${Math.random().toString(16).slice(2, 10)}`;
 const network = `ws-smoke-${suffix}`, backend = `ws-upstream-${suffix}`, proxy = `ws-proxy-${suffix}`;
 t.after(() => {
  for (const name of [proxy, backend]) try { docker('rm', '-f', '-v', name); } catch {}
  try { docker('network', 'rm', network); } catch {}
 });
 docker('network', 'create', network);
 const gateway = JSON.parse(docker('network', 'inspect', network))[0].IPAM.Config[0].Gateway;
 const conf = path.join(directory, 'conf'), ssl = path.join(directory, 'ssl'); mkdirSync(conf); mkdirSync(ssl);
 writeFileSync(path.join(ssl, 'schoolorbit-origin.pem'), tls.cert);
 writeFileSync(path.join(ssl, 'schoolorbit-origin.key'), tls.key, { mode: 0o600 });
 const upstream = path.join(directory, 'upstream.conf');
 writeFileSync(upstream, `server { listen 8081; location / { proxy_pass http://${gateway}:${upstreamPort}; proxy_http_version 1.1; proxy_set_header Upgrade $http_upgrade; proxy_set_header Connection $http_connection; proxy_set_header Host $http_host; } }`);
 docker('create', '--name', backend, '--network', network, '--network-alias', 'schoolorbit-backend-school', nginxImage);
 docker('cp', upstream, `${backend}:/etc/nginx/conf.d/default.conf`); docker('start', backend);
 execFileSync('bash', ['scripts/render_nginx_config.sh', 'nginx-configs/school-api.maintenance.conf.template', path.join(conf, 'school.conf'), 'example.test', 'a'.repeat(40), 'maintenance', probeToken]);
 docker('create', '--name', proxy, '--network', network, '-p', '127.0.0.1::443', nginxImage);
 docker('cp', conf + '/.', `${proxy}:/etc/nginx/conf.d/`); docker('cp', ssl, `${proxy}:/etc/nginx/ssl`); docker('start', proxy);
 const proxyPort = docker('port', proxy, '443/tcp').split(':')[1];
 const api = `https://school-api.example.test:${proxyPort}`;
 const headers = token => ({ Origin: origin, ...(token ? { 'X-Schoolorbit-Release-Probe': token } : {}) });
 const request = (token, pathname, extra = {}) => new Promise((resolve, reject) => {
  const outgoing = https.request({ hostname: '127.0.0.1', port: proxyPort, servername: 'school-api.example.test', ca: tls.cert, path: pathname, headers: { Host: 'school-api.example.test', ...headers(token), ...extra } }, response => {
   response.resume(); response.on('end', () => resolve(response.statusCode));
  });
  outgoing.on('error', reject); outgoing.setTimeout(2000, () => outgoing.destroy(new Error('Proxy fixture timeout'))); outgoing.end();
 });
 // Await actual readiness without blocking the synthetic server's event loop.
 for (let attempt = 0; ; attempt++) {
  try { assert.equal(await request('', '/deployment-status'), 200); break; }
  catch (error) { if (attempt === 30) throw error; await new Promise(resolve => setTimeout(resolve, 100)); }
 }
 assert.equal(await request('', '/ws/timetable'), 503);
 assert.equal(await request('wrong', '/ws/timetable'), 503);
 const unauthorized = await runProbe(directory, api, { ca: tls.certPath, resolve: '127.0.0.1', token: probeToken, cookie: 'incorrect-private-cookie' });
 assert.equal(unauthorized.status, 1, 'maintenance token must not bypass backend authentication');
 const successful = await runProbe(directory, api, { ca: tls.certPath, resolve: '127.0.0.1', token: probeToken });
 assert.equal(successful.status, 0, successful.stdout + successful.stderr);
 assert.equal(await request(probeToken, '/api/fixture', { Cookie: `__Host-schoolorbit_session=${session}` }), 200);
 assert.equal(observations.httpConnections.at(-1), '', 'ordinary probe HTTP must not carry Connection: upgrade');
 assert.equal(observations.pings, 1); assert.equal(observations.closes, 1);
 assert.deepEqual(observations.errors, []);
 assert.equal(observations.requests.at(-1).headers.host, 'school-api.example.test');
 assert.equal(observations.requests.at(-1).headers.origin, origin);
 assert.equal(observations.requests.at(-1).headers['x-forwarded-proto'], 'https');
 assert.doesNotMatch(docker('logs', proxy), /academicTermId|timetableVersionId|synthetic-private|00000000/);
 assert.equal(await request('', '/ws/timetable'), 503, 'successful probe must leave maintenance active');
 // The same real frame exchange also exercises the normal deployed /ws/ owner.
 execFileSync('bash', ['scripts/render_nginx_config.sh', 'nginx-configs/school-api.conf.template', path.join(conf, 'school.conf'), 'example.test', 'a'.repeat(40), 'ready']);
 docker('cp', path.join(conf, 'school.conf'), `${proxy}:/etc/nginx/conf.d/school.conf`);
 docker('exec', proxy, 'nginx', '-t'); docker('exec', proxy, 'nginx', '-s', 'reload');
 for (let attempt = 0; ; attempt++) {
  const result = await runProbe(directory, api, { ca: tls.certPath, resolve: '127.0.0.1' });
  if (result.status === 0) break;
  if (attempt === 20) assert.fail(result.stdout + result.stderr);
  await new Promise(resolve => setTimeout(resolve, 100));
 }
 assert.equal(observations.pings, 2); assert.equal(observations.closes, 2);
 assert.doesNotMatch(docker('logs', proxy), /academicTermId|timetableVersionId|synthetic-private|00000000/);
});
