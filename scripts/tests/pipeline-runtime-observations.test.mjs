import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
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
  assert.deepEqual(success.metrics.duration_ms, { status: 'available', samples: 20, max_ms: 20, mean_ms: 10.5, p50_ms: 10, p95_ms: 19, p99_ms: null, tail_availability: { p95: 'empirical_only', p99: 'unavailable_sparse' } });
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
  assert.equal(result.groups[1].metrics.execution_ms.p50_ms, 300);
  assert.equal(result.groups[1].metrics.execution_ms.p99_ms, null);
  assert.equal(result.groups[0].labels.status, '401');
  assert.deepEqual(result.selected_routes, {});
  const ordinaryAdmin = aggregate([
    '2026-10-23T01:02:03Z INFO backend_admin::app: operation="http_response_headers" route="/api/v1/schools" method=GET status=200 duration_ms=3',
  ], 'admin');
  assert.equal(ordinaryAdmin.groups[0].labels.route, 'other');
  assert.equal(ordinaryAdmin.groups[0].events, 1);
  assert.deepEqual(ordinaryAdmin.selected_routes, {});
  const emptySchool = aggregate([]);
  assert.deepEqual(emptySchool.selected_routes.dashboard, { events: 0, status: 'unavailable' });
  assert.deepEqual(emptySchool.selected_routes.students, { events: 0, status: 'unavailable' });
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
    '{"fields":{"operation":"http_response_headers","route":"/api/students","method":"GET","status":200,"duration_ms":Infinity}}',
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
  assert.equal(limited.accepted_events, 2);
  assert.equal(limited.aggregated_events, 1);
  assert.equal(limited.dropped_events, 1);
  assert.ok(limited.partial_reasons.includes('group_limit'));
  const oversized = aggregate([sentinel.repeat(1000), http(1)]);
  assert.equal(oversized.oversized_lines, 1);
  assert.equal(oversized.groups[0].events, 1);
});

test('empirical tails require 20/100 observations and retain exact nearest-rank values', () => {
  for (const n of [1, 19, 20, 99, 100]) {
    const result = python(`print(json.dumps(m.metric(list(range(1,data['n']+1)))))`, { n });
    assert.equal(result.samples, n);
    assert.equal(result.p50_ms, Math.ceil(.5 * n));
    assert.equal(result.p95_ms, n >= 20 ? Math.ceil(.95 * n) : null);
    assert.equal(result.p99_ms, n >= 100 ? Math.ceil(.99 * n) : null);
    assert.equal(result.tail_availability.p95, n >= 20 ? 'empirical_only' : 'unavailable_sparse');
    assert.equal(result.tail_availability.p99, n >= 100 ? 'empirical_only' : 'unavailable_sparse');
  }
});

test('requested lookback is not presented as observed event coverage or container uptime', () => {
  const result = aggregate([
    school({ operation: 'http_response_headers', route: '/api/students', method: 'GET', status: 200, duration_ms: 1 }, { timestamp: '2026-10-23T01:00:00Z' }),
    school({ operation: 'http_response_headers', route: '/api/students', method: 'GET', status: 200, duration_ms: 1 }, { timestamp: '2026-10-23T01:00:02Z' }),
  ]);
  assert.equal(result.observed_event_span_seconds, 2);
  assert.equal(result.timestamped_events, 2);
  assert.equal(result.selected_routes.dashboard.status, 'unavailable');
  assert.equal(result.selected_routes.students.events, 2);
  assert.equal(aggregate([http(1)]).observed_event_span_seconds, null);
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
  const actual = python(`print(json.dumps(m.container_snapshot(json.dumps(data).encode(),'complete')))` , [{ id: sentinel, name: sentinel, cpu_time: '3.142s', cpu_percent: '2.35%', avg_cpu: '1.20%', mem_usage: '12.5MB / 4.096GB', mem_percent: '0.31%', net_io: '0B / 0B', block_io: '0B / 0B', pids: '12' }]);
  assert.equal(actual.status, 'available');
  assert.equal(actual.cpu_percent, 2.35);
  assert.equal(actual.memory_used_bytes, 12500000);
  const unlimited = python(`print(json.dumps(m.container_snapshot(json.dumps(data).encode(),'complete')))` , [{ cpu_percent: '0.00%', mem_usage: '0B / 0B', pids: '1' }]);
  assert.equal(unlimited.status, 'partial');
  assert.equal(unlimited.memory_limit_bytes, null);
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

const fakeProbe = `
import email.message,os,urllib.parse,http.cookiejar
os.environ.update({'SMOKE_USERNAME':'synthetic-private-observation-marker','SMOKE_PASSWORD':'synthetic-private-observation-marker','BASE_DOMAIN':'example.test','BACKEND_SCHOOL_URL':'https://school-api.example.test'})
calls=[];seen={}
class Response:
 def __init__(self,path):
  self.status=200;self.headers=email.message.Message();self.path=path
  if path=='/api/auth/login':self.headers['X-CSRF-Token']=('a'*42+'Y')
 def __enter__(self):return self
 def __exit__(self,*args):return False
 def read(self,limit):
  path=self.path
  if path=='/api/auth/login' and data.get('scenario')=='malformed_login':return b'{synthetic-private-observation-marker'
  if path=='/api/auth/login' and data.get('scenario')=='oversized_login':return b'x'*limit
  user={'username':os.environ['SMOKE_USERNAME'],'userType':'staff','status':'active','permissions':['student.read.school']}
  if data.get('scenario')=='wrong_user':user['username']='another-user'
  if data.get('scenario')=='no_student_permission':user['permissions']=[]
  if path=='/api/auth/me':value=user
  elif path=='/api/academic/context/options':
   year='11111111-1111-4111-8111-111111111111';value={'activeAcademicYearId':year,'years':[{'id':year}]}
   if data.get('scenario')=='no_context':value={'years':[]}
  elif path=='/api/staff/dashboard':value={'totalStaff':2,'totalStudents':20,'activeHomerooms':1}
  elif path=='/api/students':value={'items':[{'private':'synthetic-private-observation-marker'}],'page':1,'page_size':20}
  elif path=='/api/me/work-items/counts':value={key:0 for key in ['open','dueSoon','overdue','submitted','closed','total']}
  else:value={}
  return json.dumps({'success':True,'data':value,'private':'synthetic-private-observation-marker'}).encode()
class Opener:
 def __init__(self,jar):self.jar=jar
 def open(self,request,timeout):
  parsed=urllib.parse.urlsplit(request.full_url);path=parsed.path
  assert parsed.scheme=='https' and parsed.netloc=='school-api.example.test'
  assert request.get_header('Origin')=='https://sandbox.example.test'
  assert request.get_header('X-school-subdomain')=='sandbox'
  assert request.get_header('Cache-control')=='no-store'
  assert 0<timeout<=15
  calls.append({'method':request.method,'path':path});seen[path]=seen.get(path,0)+1
  if path=='/api/auth/login':
   payload=json.loads(request.data);assert payload['rememberMe'] is False
   if data.get('scenario') not in ['no_cookie','timeout']:
    scenario=data.get('scenario')
    self.jar.set_cookie(http.cookiejar.Cookie(version=0,name='__Host-schoolorbit_session',value='b'*43 if scenario=='bad_cookie_token' else ('b'*42+'Y'),port=None,port_specified=False,domain='school-api.example.test',domain_specified=scenario=='domain_cookie',domain_initial_dot=False,path='/wrong' if scenario=='wrong_cookie_path' else '/',path_specified=True,secure=scenario!='insecure_cookie',expires=None,discard=True,comment=None,comment_url=None,rest={'HttpOnly':None,'SameSite':'Lax'},rfc2109=False))
  if path=='/api/students':assert parsed.query=='page=1&pageSize=20'
  if path=='/api/staff/dashboard':assert parsed.query=='academicYearId=11111111-1111-4111-8111-111111111111'
  if path=='/api/auth/logout':
   assert request.get_header('X-csrf-token')==('a'*42+'Y')
   assert request.get_header('Cookie')=='__Host-schoolorbit_session='+('b'*42+'Y')
  if (path=='/api/students' and data.get('scenario')=='forbidden') or (path=='/api/auth/logout' and data.get('scenario')=='cleanup_failure') or (path=='/api/me/work-items/counts' and seen[path]==3 and data.get('scenario')=='partial_route'):
   raise m.urllib.error.HTTPError('https://synthetic-private-observation-marker',403,'synthetic-private-observation-marker',{},None)
  if path=='/api/auth/login' and data.get('scenario')=='timeout':raise m.ProbeFailure('request_timeout')
  return Response(path)
m.urllib.request.build_opener=lambda *handlers:Opener(next(handler.cookiejar for handler in handlers if isinstance(handler,m.urllib.request.HTTPCookieProcessor)))
report=m.sandbox_probe()
print(json.dumps({'report':report,'calls':calls}))
`;

test('optional sandbox probe uses one owned login, warmup plus five sequential reads and own logout', () => {
  const { report, calls } = python(fakeProbe);
  assert.equal(report.status, 'available');
  assert.equal(report.cleanup, 'completed');
  assert.equal(calls.filter(call => call.method === 'POST').length, 2);
  assert.equal(calls[0].path, '/api/auth/login');
  assert.equal(calls.at(-1).path, '/api/auth/logout');
  for (const path of ['/api/staff/dashboard', '/api/students', '/api/me/work-items/counts'])
    assert.equal(calls.filter(call => call.path === path).length, 6);
  for (const route of report.routes) {
    assert.equal(route.samples, 5);
    assert.ok(route.client_wall_ms.min >= 0);
    assert.ok(route.response_body_bytes.min > 0);
    assert.ok(!Object.hasOwn(route, 'p95_ms'));
  }
});

test('malformed or oversized login body still logs out using CSRF captured before parsing', () => {
  for (const scenario of ['malformed_login', 'oversized_login']) {
    const { report, calls } = python(fakeProbe, { scenario });
    assert.equal(report.cleanup, 'completed');
    assert.equal(report.status, 'unavailable');
    assert.deepEqual(calls.map(call => call.path), ['/api/auth/login', '/api/auth/logout']);
  }
});

test('probe eligibility and HTTP denial stop unavailable endpoints without retries or privilege changes', () => {
  for (const scenario of ['no_student_permission', 'forbidden', 'no_context', 'wrong_user']) {
    const { report, calls } = python(fakeProbe, { scenario });
    assert.equal(report.cleanup, 'completed');
    if (scenario === 'wrong_user') {
      assert.equal(report.reason, 'active_staff_required');
      assert.equal(report.routes.length, 0);
    } else {
      const route = report.routes.find(route => route.route === (scenario === 'no_context' ? 'dashboard' : 'students'));
      assert.equal(route.status, 'unavailable');
      assert.equal(calls.filter(call => call.path === (scenario === 'no_context' ? '/api/staff/dashboard' : '/api/students')).length, scenario === 'forbidden' ? 1 : 0);
    }
  }
});

test('probe reports partial samples, unknown login cleanup and failed logout explicitly', () => {
  const partial = python(fakeProbe, { scenario: 'partial_route' });
  assert.equal(partial.report.routes.find(route => route.route === 'work_counts').samples, 1);
  assert.equal(partial.calls.filter(call => call.path === '/api/me/work-items/counts').length, 3);
  const timeout = python(fakeProbe, { scenario: 'timeout' });
  assert.equal(timeout.report.cleanup, 'unavailable');
  assert.equal(timeout.report.cleanup_reason, 'cleanup_unknown');
  assert.equal(timeout.calls.length, 1);
  const cleanup = python(fakeProbe, { scenario: 'cleanup_failure' });
  assert.equal(cleanup.report.cleanup, 'unavailable');
  assert.equal(cleanup.report.status, 'partial');
});

test('missing or ineligible session cookie cannot turn idempotent logout into claimed cleanup', () => {
  for (const scenario of ['no_cookie', 'domain_cookie', 'wrong_cookie_path', 'insecure_cookie', 'bad_cookie_token']) {
    const { report, calls } = python(fakeProbe, { scenario });
    assert.equal(report.cleanup, 'unavailable');
    assert.equal(report.cleanup_reason, 'cleanup_unknown');
    assert.equal(report.reason, 'session_cookie_unavailable');
    assert.deepEqual(calls.map(call => call.path), ['/api/auth/login']);
  }
});

test('missing credentials, untrusted target and redirects cannot cause arbitrary requests', () => {
  const result = python(`import os
os.environ.pop('SMOKE_USERNAME',None);os.environ.pop('SMOKE_PASSWORD',None)
m.urllib.request.build_opener=lambda *args:(_ for _ in ()).throw(AssertionError('must not connect'))
missing=m.sandbox_probe()
os.environ.update({'SMOKE_USERNAME':'synthetic-private-observation-marker','SMOKE_PASSWORD':'synthetic-private-observation-marker','BASE_DOMAIN':'example.test','BACKEND_SCHOOL_URL':'http://synthetic-private-observation-marker'})
invalid=m.sandbox_probe()
class Response:
 closed=False
 def close(self):self.closed=True
response=Response()
try:m.RefuseRedirect().redirect_request(None,response,302,'',{},'https://synthetic-private-observation-marker')
except m.ProbeFailure as error:redirect={'reason':error.reason,'closed':response.closed}
print(json.dumps({'missing':missing,'invalid':invalid,'redirect':redirect}))`);
  assert.equal(result.missing.reason, 'credentials_missing');
  assert.equal(result.invalid.reason, 'trusted_configuration_invalid');
  assert.deepEqual(result.redirect, { reason: 'redirect_refused', closed: true });
});

test('hard request deadline interrupts a blocking provider without printing its exception', () => {
  const result = python(`import time
try:
 with m.request_deadline(.025):time.sleep(2)
except m.ProbeFailure as error:print(json.dumps({'reason':error.reason}))`);
  assert.equal(result.reason, 'request_timeout');
});

test('workflow exposes only boolean opt-in on main and probes before bounded log collection', () => {
  const workflow = readFileSync(path.resolve(import.meta.dirname, '../../.github/workflows/runtime-diagnostics.yml'), 'utf8');
  const operations = readFileSync(path.resolve(import.meta.dirname, '../../.github/workflows/operations.yml'), 'utf8');
  assert.match(workflow, /run_sandbox_probe:\s+type: boolean\s+default: false/);
  assert.match(operations, /run_sandbox_probe: \$\{\{ fromJson\(inputs\.options\)\.run_sandbox_probe == true \}\}/);
  assert.ok(workflow.indexOf('name: Measure selected sandbox reads') < workflow.indexOf('name: Collect sanitized runtime observations'));
  assert.doesNotMatch(workflow, /podman logs|inputs\.(?:api_url|subdomain)|artifact|trace:/);
  const probeStep = workflow.split('name: Measure selected sandbox reads')[1].split('      - name:')[0];
  assert.match(probeStep, /github.ref == 'refs\/heads\/main' && inputs.run_sandbox_probe/);
  assert.match(probeStep, /vars\.BASE_DOMAIN/);
  assert.match(probeStep, /vars\.BACKEND_SCHOOL_URL/);
  assert.match(probeStep, /secrets\.SMOKE_USERNAME/);
  assert.match(probeStep, /secrets\.SMOKE_PASSWORD/);
});
