import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import test from 'node:test';

const script = path.resolve(import.meta.dirname, '../lib/pipeline-runtime-observations.py');
const sentinel = 'synthetic-private-observation-marker';
const loader = `import importlib.util,json,sys
spec=importlib.util.spec_from_file_location('observations',sys.argv[1])
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
data=json.load(sys.stdin)
`;
function python(code, input = {}) {
  const result = spawnSync('python3', ['-B', '-c', loader + code, script], {
    input: JSON.stringify(input), encoding: 'utf8', timeout: 10000, maxBuffer: 2 * 1024 * 1024
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, '');
  assert.ok(!result.stdout.includes(sentinel), 'collector must never echo private input');
  return JSON.parse(result.stdout);
}
function aggregate(lines, source = 'school', reason = 'complete') {
  return python(`print(json.dumps(m.aggregate(('\\n'.join(data['lines'])+'\\n').encode(),data['source'],data['reason']),allow_nan=False))`, { lines, source, reason });
}
const school = (fields, extra = {}) => JSON.stringify({ fields, target: 'backend_school::app', ...extra });
const http = (duration, extra = {}) => school({ operation: 'http_response_headers', route: '/api/students', method: 'GET', status: 200, duration_ms: duration, ...extra });

test('HTTP distributions use nearest rank and separate errors, cancellation and routes', () => {
  const result = aggregate([
    ...Array.from({ length: 20 }, (_, index) => http(index + 1)),
    http(1000, { status: 503 }),
    school({ operation: 'http_request_cancelled', route: '/api/students', method: 'GET', duration_ms: 15 }),
    http(1, { route: sentinel, method: sentinel, diagnostic_id: sentinel }),
  ]);
  const success = result.groups.find(group => group.labels.route === 'students' && group.labels.status === '2xx');
  assert.deepEqual(success.metrics.duration_ms, { status: 'available', samples: 20, max_ms: 20, mean_ms: 10.5, p50_ms: 10, p95_ms: 19, p99_ms: 20 });
  assert.equal(result.groups.find(group => group.labels.status === '503').events, 1);
  assert.equal(result.groups.find(group => group.labels.status === 'cancelled').metrics.duration_ms.samples, 1);
  assert.equal(result.groups.find(group => group.labels.route === 'other').labels.method, 'OTHER');
});

test('Admin fmt and ANSI parse only known targets and numeric producer fields', () => {
  const result = aggregate([
    '\u001b[32m2026-10-23T01:02:03Z INFO backend_admin::app: operation="http_response_headers" route="/api/auth/login" method=POST status=401 duration_ms=2.5\u001b[0m',
    `2026-10-23T01:02:03Z INFO backend_admin::auth::password_work: operation="password_work" kind="verify" outcome="completed" admission_ms=1 blocking_queue_ms=2 execution_ms=300 capacity=4 ignored="${sentinel}"`,
    `2026-10-23T01:02:03Z INFO arbitrary::${sentinel}: operation="http_response_headers" route="/api/students" method=GET status=200 duration_ms=999`,
  ], 'admin');
  assert.equal(result.accepted_events, 2);
  assert.equal(result.groups[1].metrics.execution_ms.p99_ms, 300);
  assert.equal(result.groups[0].labels.status, '401');
});

test('password outcomes and absent stages stay separate; missing is never zero duration', () => {
  const result = aggregate([
    school({ operation: 'password_work', kind: 'hash', outcome: 'admission_timeout', admission_ms: 1001, capacity: 4, error: sentinel }),
    school({ operation: 'password_work', kind: 'verify', outcome: 'cancelled_before_admission', admission_ms: 4, capacity: 4 }),
    school({ operation: 'password_work', kind: 'verify', outcome: sentinel, admission_ms: 1, capacity: 4 }),
  ]);
  assert.equal(result.groups.length, 2);
  assert.deepEqual(result.groups[0].metrics.execution_ms, { status: 'unavailable', samples: 0 });
  assert.equal(result.groups[0].metrics.admission_ms.samples, 1);
  assert.equal(result.invalid_events, 1);
  assert.equal(result.status, 'partial');
});

test('SQLx successful acquisition samples convert seconds and never publish population quantiles', () => {
  const result = aggregate([
    school({ message: 'acquired connection, but time to acquire exceeded slow threshold', acquired_after_secs: .375, slow_acquire_threshold_secs: .25 }, { target: 'sqlx::pool::acquire' }),
    school({ message: sentinel, acquired_after_secs: .5 }, { target: 'sqlx::pool::acquire' }),
  ]);
  const group = result.groups[0];
  assert.equal(group.labels.coverage, 'logged_successes_only');
  assert.equal(group.labels.sample_kind, 'slow');
  assert.deepEqual(group.metrics.acquisition_ms, { status: 'available', samples: 1, max_ms: 375, mean_ms: 375 });
  const admin = aggregate(['2026-10-23T01:02:03Z INFO sqlx::pool::acquire: acquired connection acquired_after_secs=0.001'], 'admin');
  assert.equal(admin.groups[0].metrics.acquisition_ms.max_ms, 1);
});

test('proxy lifetime and upstream header/body stages remain separate; null/multiple attempts unavailable', () => {
  const result = aggregate([JSON.stringify({ operation: 'school_proxy_request', route_family: 'identity', method: 'GET', status: 499, request_time: 2, upstream_connect_time: null, upstream_header_time: .5, upstream_response_time: '1, 2', request_id: sentinel, diagnostic_id: sentinel })], 'proxy');
  assert.equal(result.groups[0].labels.status, '499');
  assert.equal(result.groups[0].metrics.request_time_ms.p50_ms, 2000);
  assert.equal(result.groups[0].metrics.upstream_header_time_ms.p50_ms, 500);
  assert.deepEqual(result.groups[0].metrics.upstream_connect_time_ms, { status: 'unavailable', samples: 0 });
  assert.deepEqual(result.groups[0].metrics.upstream_response_time_ms, { status: 'unavailable', samples: 0 });
});

test('malformed, hostile types, nonfinite, negative, boolean and huge inputs cannot leak or invent samples', () => {
  const result = aggregate([
    sentinel, '{', JSON.stringify([sentinel]),
    http(-1), http(true), http(1e100), http(0, { status: sentinel }),
    http(1, { method: { secret: sentinel }, route: { secret: sentinel } }),
    school({ operation: { secret: sentinel } }),
    school({ operation: 'password_work', kind: { secret: sentinel }, outcome: 'completed', admission_ms: 1, capacity: 4 }),
    '{"fields":{"operation":"http_response_headers","duration_ms":NaN}}',
    http(0, { error: sentinel }),
  ]);
  assert.equal(result.accepted_events, 2);
  assert.equal(result.groups.find(group => group.labels.route === 'students').metrics.duration_ms.max_ms, 0);
  assert.ok(result.invalid_events >= 5);
});

test('zero events and failed, capped or incomplete collections report coverage explicitly', () => {
  assert.equal(aggregate([]).status, 'unavailable');
  assert.equal(aggregate([http(2)], 'school', 'command_timeout').status, 'partial');
  const failed = aggregate([sentinel], 'school', 'command_failed');
  assert.equal(failed.accepted_events, 0);
  assert.equal(failed.status, 'partial');
  const limited = python(`m.MAX_LINES=2;m.MAX_LINE_BYTES=1000;m.MAX_GROUPS=1
print(json.dumps(m.aggregate(('\\n'.join(data['lines'])+'\\n').encode(),'school')))`, { lines: [http(1), http(2, { status: 503 }), http(3)] });
  assert.equal(limited.lines, 2);
  assert.equal(limited.groups.length, 1);
  assert.equal(limited.status, 'partial');
  const oversized = aggregate([sentinel.repeat(1000), http(1)]);
  assert.equal(oversized.oversized_lines, 1);
  assert.equal(oversized.groups[0].events, 1);
});

test('subprocess collector caps time and bytes and never surfaces stderr or command failures', () => {
  const result = python(`samples=[]
for code in ["import sys;sys.stderr.write('synthetic-private-observation-marker');sys.exit(3)","import time;time.sleep(2)","print('x'*10000)"]:
 raw,reason=m.capture([sys.executable,'-c',code],timeout=.1,byte_limit=100)
 samples.append({'reason':reason,'bytes':len(raw)})
print(json.dumps(samples))`);
  assert.deepEqual(result.map(value => value.reason), ['command_failed', 'command_timeout', 'byte_limit']);
  assert.ok(result.every(value => value.bytes <= 100));
});

test('container snapshots allow numeric units only and never echo names/errors from podman', () => {
  const result = python(`print(json.dumps(m.container_snapshot(json.dumps(data).encode(),'complete')))` , [{ cpu: '12.5%', mem_usage: '1.5MiB / 2GB', pids: '4', name: sentinel, error: sentinel }]);
  assert.equal(result.status, 'available');
  assert.equal(result.memory_used_bytes, 1.5 * 1024 * 1024);
  assert.equal(result.memory_limit_bytes, 2e9);
  const absent = python(`print(json.dumps(m.container_snapshot(json.dumps(data).encode(),'complete')))` , [{ cpu: sentinel, mem_usage: sentinel, pids: sentinel }]);
  assert.equal(absent.status, 'unavailable');
});

test('production command plan uses only fixed containers and bounds every raw-log source', () => {
  const result = python(`commands=[]
def capture(command,**kwargs):
 commands.append(command)
 return b'', 'command_unavailable'
m.capture=capture;m.host_snapshot=lambda:{'scope':'backend_host','status':'unavailable'}
report=m.collect()
print(json.dumps({'commands':commands,'report':report}))`);
  assert.equal(result.commands.length, 7);
  assert.ok(result.commands.slice(0, 3).every(command => command.slice(0, 6).join(' ') === 'podman logs --since 15m --tail 5000'));
  assert.equal(result.report.database_host_resources, 'unavailable');
  assert.ok(result.report.sources.every(source => source.accepted_events === 0 && source.groups.length === 0));
  assert.ok(result.report.containers.every(container => container.status === 'unavailable'));
});
