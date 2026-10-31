import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "../..");
const script = path.join(
  root,
  "scripts/lib/pipeline-academic148-observations.py",
);
const helper = path.join(
  root,
  "scripts/lib/pipeline-runtime-observation-artifacts.py",
);
const sentinel = "private-observation-sentinel";
const loader = `import importlib.util,json,sys,os,tempfile,pathlib,subprocess,urllib.error
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('academic148',sys.argv[1]);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
data=json.load(sys.stdin)
`;
function python(code, data = {}, options = {}) {
  const result = spawnSync("python3", ["-B", "-c", loader + code, script], {
    input: JSON.stringify(data),
    encoding: "utf8",
    timeout: 60000,
    maxBuffer: 2 * 1024 * 1024,
    ...options,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, "");
  assert.ok(!result.stdout.includes(sentinel));
  return result.stdout.trim() ? JSON.parse(result.stdout) : null;
}
const empty = () =>
  python(`value={'operation':'academic148_curriculum','schema_version':2,'history_version':147,'history_verified':True,'counts':{key:0 for key in m.COUNTS},'masks':[{'mask':mask,'count':0} for mask in range(16)],'timing':[{'kind':kind,'count':0,'violating':0,'zero_recorded_positive_created':0} for kind in m.KINDS],'checkpoints':[{'version':version,'present':True,'unique':True,'count_match':False,'hash_match':True} for version in (138,141)],'divergence':{key:0 for key in m.DIVERGENCE}}
print(json.dumps(m.validate(value)))`);

function lowerReceipt(historyVersion = 147) {
  const value = empty();
  value.history_version = historyVersion;
  for (const key of [
    "receipts",
    "violating_receipts",
    "created_count_mismatches",
    "matched_created_lower",
  ])
    value.counts[key] = 1;
  value.masks[1].count = 1;
  value.timing[2].count = 1;
  value.timing[2].violating = 1;
  Object.assign(value.divergence, {
    lower_receipts: 1,
    recorded_created_total: 4,
    timestamp_created_total: 1,
    difference_total: 3,
    distinct_output_total: 4,
    output_entries_equal: 1,
    output_entries_later: 3,
    later_entries_with_current_receipt: 2,
    later_entries_with_delete_item_audit: 1,
  });
  return value;
}

test("aggregate preserves independent checkpoint observations and complete history counts", () => {
  const report = python(
    `print(json.dumps(m.aggregate([m.validate(data),m.validate({**data,'history_version':148})])))`,
    empty(),
  );
  assert.equal(report.tenants, 2);
  assert.deepEqual(report.history_versions, [
    { version: 147, count: 1 },
    { version: 148, count: 1 },
  ]);
  assert.equal(report.checkpoints[0].count_match_tenants, 0);
  assert.equal(report.checkpoints[0].hash_match_tenants, 2);
  assert.deepEqual(
    report.history_observations.map((group) => [group.version, group.tenants]),
    [
      [147, 1],
      [148, 1],
    ],
  );
  assert.ok(!JSON.stringify(report).includes("history_verified"));
});

test("schema2 groups divergence and checkpoint evidence by verified history without mixing units", () => {
  const first = lowerReceipt();
  const second = lowerReceipt(148);
  second.divergence.distinct_output_total = 99;
  second.divergence.later_entries_with_current_receipt = 1;
  second.checkpoints[0].count_match = true;
  second.checkpoints[0].hash_match = false;
  const third = empty();
  third.history_version = 148;
  const report = python(`print(json.dumps(m.aggregate(data)))`, [
    first,
    second,
    third,
  ]);
  assert.equal(report.schema_version, 2);
  assert.deepEqual(
    report.history_observations.map((group) => group.tenants),
    [1, 2],
  );
  assert.equal(
    report.history_observations[0].divergence.distinct_output_total,
    4,
  );
  assert.equal(
    report.history_observations[1].divergence.distinct_output_total,
    99,
  );
  assert.equal(
    report.history_observations[1].checkpoints[0].count_match_tenants,
    1,
  );
  assert.equal(
    report.history_observations[1].checkpoints[0].hash_match_tenants,
    1,
  );
  assert.equal(report.counts.matched_created_lower, 2);
  assert.ok(!JSON.stringify(report).includes("history_verified"));
});

test("schema2 rejects old, private, noninteger and inconsistent divergence evidence", () => {
  python(
    `import copy
cases=[]
value=copy.deepcopy(data);value['schema_version']=1;cases.append(value)
value=copy.deepcopy(data);del value['divergence'];cases.append(value)
value=copy.deepcopy(data);value['divergence']['private']='private-observation-sentinel';cases.append(value)
for key in m.DIVERGENCE:
 for invalid in (True,-1,0.5,9007199254740992):
  value=copy.deepcopy(data);value['divergence'][key]=invalid;cases.append(value)
for key,number in (('lower_receipts',2),('recorded_created_total',3),('timestamp_created_total',2),('difference_total',0),('output_entries_equal',0),('recorded_above_output_receipts',2),('later_entries_with_current_receipt',4),('later_entries_with_delete_item_audit',4)):
 value=copy.deepcopy(data);value['divergence'][key]=number;cases.append(value)
value=copy.deepcopy(data);value['counts']['matched_created_lower']=0;value['counts']['matched_created_higher']=1;cases.append(value)
for value in cases:
 try:m.validate(value)
 except Exception:pass
 else:raise AssertionError('accepted invalid divergence')`,
    lowerReceipt(),
  );
  python(
    `data['divergence']['distinct_output_total']=1
try:m.validate(data)
except m.Refusal:pass
else:raise AssertionError('zero lower receipts retained evidence')`,
    empty(),
  );
});

test("schema2 aggregate rejects malformed history groups, mixed totals and secret fields", () => {
  python(
    `import copy
report=m.aggregate([data]);cases=[]
for mutate in (
 lambda v:v.update(schema_version=1),
 lambda v:v.pop('history_observations'),
 lambda v:v['history_observations'].reverse(),
 lambda v:v['history_observations'][0].update(tenants=0),
 lambda v:v['history_observations'][0].update(private='private-observation-sentinel'),
 lambda v:v['history_observations'][0]['divergence'].update(private='private-observation-sentinel'),
 lambda v:v['history_observations'][0]['divergence'].update(lower_receipts=2),
 lambda v:v['history_observations'][0]['checkpoints'][0].update(hash_match_tenants=0),
 lambda v:v['history_observations'][1]['checkpoints'][0].update(present_tenants=1),
):
 value=copy.deepcopy(report);mutate(value);cases.append(value)
value=copy.deepcopy(report);value['history_observations'][1]['divergence']=value['history_observations'][0]['divergence'];value['history_observations'][0]['divergence']={key:0 for key in m.DIVERGENCE};cases.append(value)
for value in cases:
 try:m.validate_aggregate(value)
 except Exception:pass
 else:raise AssertionError('accepted inconsistent group')`,
    lowerReceipt(),
  );
});

test("internal report rejects malformed shapes, unsafe numbers and inconsistent predicate evidence", () => {
  python(
    `import copy
cases=[]
for field in ('history_version','history_verified','checkpoints','masks','timing'):
 value=copy.deepcopy(data);value[field]=None;cases.append(value)
value=copy.deepcopy(data);value['private']=True;cases.append(value)
for count in (-1,0.5,True,9007199254740992):
 value=copy.deepcopy(data);value['counts']['receipts']=count;cases.append(value)
value=copy.deepcopy(data);value['masks'][1]['count']=1;cases.append(value)
value=copy.deepcopy(data);value['timing'][0]['count']=1;cases.append(value)
value=copy.deepcopy(data);value['checkpoints'][0]['unique']=False;cases.append(value)
for value in cases:
 try:m.validate(value)
 except Exception:pass
 else:raise AssertionError('accepted malformed report')
for raw in (b'{"a":1,"a":2}',b'{"a":NaN}',b'{"a":1}{}',b'x'*(m.MAX_BYTES+1)):
 try:m.decode(raw)
 except Exception:pass
 else:raise AssertionError('accepted hostile JSON')`,
    empty(),
  );
});

test("trusted main preparation requires exact immutable migration prefix and private hash-bound files", (t) => {
  const temporary = mkdtempSync(path.join(os.tmpdir(), "academic148-prepare-"));
  t.after(() => rmSync(temporary, { recursive: true, force: true }));
  const output = path.join(temporary, "academic148-inputs");
  const result = spawnSync("python3", ["-B", script, "prepare"], {
    env: {
      ...process.env,
      GITHUB_REF: "refs/heads/main",
      GITHUB_WORKSPACE: root,
      RUNNER_TEMP: temporary,
    },
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(readFileSync(path.join(output, "manifest.json")));
  assert.equal(manifest.length, 148);
  for (const [index, item] of manifest.entries()) {
    assert.equal(item.version, index + 1);
    assert.match(item.checksum, /^[a-f0-9]{96}$/);
  }
  for (const name of [
    "manifest.json",
    "pipeline-academic148-observations.py",
    "pipeline-academic148-observations.sql",
    "digests.json",
  ])
    assert.equal(statSync(path.join(output, name)).mode & 0o777, 0o600);
  assert.equal(statSync(output).mode & 0o777, 0o700);
  const denied = spawnSync("python3", ["-B", script, "prepare"], {
    env: {
      ...process.env,
      GITHUB_REF: "refs/heads/feature",
      GITHUB_WORKSPACE: root,
      RUNNER_TEMP: temporary,
    },
    encoding: "utf8",
  });
  assert.notEqual(denied.status, 0);
  assert.equal(denied.stdout, "");
  assert.equal(denied.stderr.trim(), "academic148_observation=refused");
});

test("operational provider denies redirects, invalid inventories and owner routing drift", () => {
  python(`tenant={'id':'11111111-1111-4111-8111-111111111111','subdomain':'retained','status':'inactive','db_connection_string':'postgresql://so_11111111111141118111111111111111_runtime:secret@db/fixture?sslmode=verify-full&sslrootcert=%2Frun%2Fsecrets%2Fpostgresql-ca.crt'}
with patch.object(m,'admin_get',return_value={'total':1,'schools':[tenant]}):
 assert m.inventory('secret')==[tenant]
for inventory in ({'total':2,'schools':[tenant]},{'total':2,'schools':[tenant,tenant]},{'total':65,'schools':[tenant]*65}):
 with patch.object(m,'admin_get',return_value=inventory):
  try:m.inventory('secret')
  except Exception:pass
  else:raise AssertionError('accepted incomplete inventory')
for url in (tenant['db_connection_string'].replace('verify-full','require'),tenant['db_connection_string']+'&extra=1'):
 try:m.connection(url)
 except Exception:pass
 else:raise AssertionError('accepted unsafe routing')
try:m.NoRedirect().redirect_request(None,None,None,None,None,None)
except m.Refusal:pass
else:raise AssertionError('redirect accepted')
class Response:
 status=200
 def __enter__(self):return self
 def __exit__(self,*args):pass
 def read(self,maximum):return b'{"total":0,"schools":[]}'
class Opener:
 def open(self,request,timeout):
  assert request.full_url=='http://127.0.0.1:8080/internal/schools'
  assert request.get_header('X-internal-caller')=='backend-school'
  assert request.get_header('X-internal-secret')=='private-observation-sentinel'
  assert timeout==10
  return Response()
def opener(*handlers):
 assert handlers[0].proxies=={} and isinstance(handlers[1],m.NoRedirect)
 return Opener()
with patch.object(m.urllib.request,'build_opener',side_effect=opener):assert m.admin_get('/internal/schools','private-observation-sentinel')['total']==0`);
});

test("capture enforces byte and process deadlines without exposing stderr", () => {
  python(`assert m.capture([sys.executable,'-c','import sys;sys.stderr.write("private-observation-sentinel");print("safe")'])==b'safe\\n'
for command,seconds in (([sys.executable,'-c','print("x"*1100000)'],2),([sys.executable,'-c','import time;time.sleep(2)'],0.05),([sys.executable,'-c','import sys;sys.stderr.write("private-observation-sentinel");sys.exit(1)'],2)):
 try:m.capture(command,seconds=seconds)
 except Exception:pass
 else:raise AssertionError('capture limit ignored')`);
});

test("collector refuses changed source bytes before loading credentials or opening connections", () => {
  python(`import hashlib
with tempfile.TemporaryDirectory() as directory:
 root=pathlib.Path(directory);root.chmod(0o700)
 payloads=[b'fixed collector',b'SELECT fixed',json.dumps([{'version':version,'checksum':'a'*96} for version in range(1,149)]).encode()]
 for name,raw in zip(('pipeline-academic148-observations.py','pipeline-academic148-observations.sql','manifest.json'),payloads):(root/name).write_bytes(raw)
 digests=[hashlib.sha256(raw).hexdigest() for raw in payloads]
 m.__file__=str(root/'pipeline-academic148-observations.py')
 def local_path(value):return root if str(value)=='/opt/stack/deployment/academic148-observations' else pathlib.Path(value)
 (root/'pipeline-academic148-observations.sql').write_text('changed private-observation-sentinel')
 with patch.object(m,'Path',side_effect=local_path),patch.object(m,'capture') as capture:
  try:m.collect(digests)
  except m.Refusal:pass
  else:raise AssertionError('changed query accepted')
  capture.assert_not_called()`);
});

test("complete collection rejects directory drift or any failed tenant without partial aggregates", () => {
  python(
    `tenants=[{'id':'first'},{'id':'retained-inactive'}]
with patch.object(m,'inventory',side_effect=[tenants,tenants]),patch.object(m,'observe',return_value=data) as observation:
 report=m.collect_tenants('secret','fixed SQL',[],None)
 assert report['tenants']==2 and observation.call_count==2
with patch.object(m,'inventory',side_effect=[tenants,[{'id':'changed'}]]),patch.object(m,'observe',return_value=data):
 try:m.collect_tenants('secret','fixed SQL',[],None)
 except m.Refusal:pass
 else:raise AssertionError('directory drift accepted')
with patch.object(m,'inventory',return_value=tenants),patch.object(m,'observe',side_effect=[data,m.Refusal()]),patch.object(m,'aggregate') as aggregate:
 try:m.collect_tenants('secret','fixed SQL',[],None)
 except m.Refusal:pass
 else:raise AssertionError('partial observation accepted')
 aggregate.assert_not_called()`,
    empty(),
  );
});

test("artifact promotion accepts only reconciled aggregate data and never raw private reports", (t) => {
  const directory = mkdtempSync(
    path.join(os.tmpdir(), "academic148-artifact-"),
  );
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const report = python("print(json.dumps(m.aggregate([data])))", empty());
  const candidate = path.join(directory, "candidate.json");
  const destination = path.join(
    directory,
    "artifact/academic148-curriculum.json",
  );
  function promote(raw) {
    writeFileSync(candidate, raw);
    return spawnSync(
      "python3",
      ["-B", helper, "academic148", candidate, destination],
      { encoding: "utf8" },
    );
  }
  assert.equal(promote(JSON.stringify(report)).status, 0);
  assert.equal(statSync(destination).mode & 0o777, 0o600);
  for (const hostile of [
    { ...report, schema_version: 1 },
    { ...report, private: sentinel },
    { ...report, tenants: 2 },
    { ...report, counts: { ...report.counts, receipts: true } },
    {
      ...report,
      checkpoints: [
        { ...report.checkpoints[0], unique_tenants: 0 },
        report.checkpoints[1],
      ],
    },
    {
      ...report,
      history_versions: [
        { version: 147, count: 0 },
        { version: 148, count: 0 },
      ],
    },
    {
      ...report,
      history_observations: [
        {
          ...report.history_observations[0],
          divergence: {
            ...report.history_observations[0].divergence,
            private: sentinel,
          },
        },
        report.history_observations[1],
      ],
    },
  ]) {
    const result = promote(JSON.stringify(hostile));
    assert.notEqual(result.status, 0);
    assert.ok(!(result.stdout + result.stderr).includes(sentinel));
    assert.deepEqual(JSON.parse(readFileSync(destination)), report);
  }
  assert.notEqual(
    promote(
      JSON.stringify(report).replace('"tenants":1', '"tenants":1,"tenants":1'),
    ).status,
    0,
  );
  assert.notEqual(
    promote(
      JSON.stringify(report).replace(
        '"lower_receipts":0',
        '"lower_receipts":0,"lower_receipts":0',
      ),
    ).status,
    0,
  );
});

test("real PostgreSQL TLS client verifies identity and read-only snapshot before fixed observation", () => {
  python(
    `spec=importlib.util.spec_from_file_location('transport',pathlib.Path(sys.argv[1]).parents[1]/'tests/owned-load-cleanup-transport.test.py');fixture=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixture)
case=fixture.OwnerTlsTransport;case.setUpClass()
original=m.capture
try:
 runtime=case.directory/'runtime';runtime.mkdir(mode=0o700);(runtime/str(os.getuid())).mkdir(mode=0o700)
 encoded=urllib.parse.quote(case.password,safe='')
 base=f'postgresql://{{user}}:{encoded}@owned-cleanup-postgres/{fixture.CAT["databaseName"]}?sslmode=verify-full&sslrootcert=%2Frun%2Fsecrets%2Fpostgresql-ca.crt'
 tenant={'id':fixture.CAT['tenantId'],'subdomain':'fixture','status':'inactive','db_connection_string':base.format(user=case.runtime)}
 def provider(route,secret):
  assert route=='/internal/schools/fixture/migration-connection?include_inactive=true'
  return {'db_migration_connection_string':base.format(user=case.owner)}
 non_tmpfs=False
 def local(args,**kwargs):
  if args[:4]==['stat','-f','-c','%T']:return b'overlay\\n' if non_tmpfs else b'tmpfs\\n'
  assert args[:2]==['podman','run'] and case.password not in str(args) and case.password not in str(kwargs['env'])
  assert '--read-only' in args and '--cap-drop=all' in args and '--security-opt=no-new-privileges' in args
  assert kwargs['env']['PGSSLMODE']=='verify-full' and kwargs['env']['PGCONNECT_TIMEOUT']=='5'
  assert 'default_transaction_read_only=on' in kwargs['env']['PGOPTIONS']
  mounted=pathlib.Path(next(arg.split(':/run/academic148:ro')[0] for arg in args if arg.endswith(':/run/academic148:ro')))
  assert mounted.stat().st_mode&0o777==0o700 and (mounted/'pgpass').stat().st_mode&0o777==0o600
  converted=list(args);converted[0]='docker';converted.remove('--userns=keep-id')
  converted[converted.index('--name')+1]=case.prefix+'-academic148-client'
  converted[converted.index('--network')+1]=case.network
  return original(converted,**kwargs)
 query="SELECT '"+json.dumps(data).replace("'","''")+"'::jsonb WHERE current_setting('transaction_read_only')='on' AND current_setting('transaction_isolation')='repeatable read'"
 def local_path(value):return runtime if str(value)=='/run/user' else pathlib.Path(value)
 with patch.object(m,'admin_get',side_effect=provider),patch.object(m,'capture',side_effect=local),patch.object(m,'Path',side_effect=local_path):
  assert m.observe(tenant,'private-observation-sentinel',query,[],case.ca/'ca.crt')==data
  non_tmpfs=True
  try:m.observe(tenant,'secret',query,[],case.ca/'ca.crt')
  except m.Refusal:pass
  else:raise AssertionError('non-tmpfs credential directory accepted')
  non_tmpfs=False
  for bad in ("WITH changed AS (UPDATE cleanup_transport_probe SET value='changed' RETURNING id) SELECT '{}'::jsonb FROM changed",):
   try:m.observe(tenant,'secret',bad,[],case.ca/'ca.crt')
   except m.Refusal:pass
   else:raise AssertionError('read-only write accepted')
  changed={**tenant,'id':'11111111-1111-4111-8111-111111111111'}
  try:m.observe(changed,'secret',query,[],case.ca/'ca.crt')
  except m.Refusal:pass
  else:raise AssertionError('wrong identity accepted')
  bad_host={**tenant,'db_connection_string':tenant['db_connection_string'].replace('@owned-cleanup-postgres/','@wrong-name/')}
  def wrong_hostname(route,secret):return {'db_migration_connection_string':base.format(user=case.owner).replace('@owned-cleanup-postgres/','@wrong-name/')}
  with patch.object(m,'admin_get',side_effect=wrong_hostname):
   try:m.observe(bad_host,'secret',query,[],case.ca/'ca.crt')
   except m.Refusal:pass
   else:raise AssertionError('verify-full hostname refusal bypassed')
  case.invoke(['docker','exec',case.server,'psql','-X','-U','postgres','-c',f"COMMENT ON ROLE {case.owner} IS 'untrusted-marker'"])
  try:m.observe(tenant,'secret',query,[],case.ca/'ca.crt')
  except m.Refusal:pass
  else:raise AssertionError('database marker prelude bypassed')
 result=case.invoke(['docker','exec',case.server,'psql','-X','-A','-t','-U','postgres','-d',fixture.CAT['databaseName'],'-c','SELECT value FROM cleanup_transport_probe'])
 assert result.stdout.strip()==b'preserved'
finally:case.tearDownClass()`,
    empty(),
    { timeout: 120000 },
  );
});

test("workflow exposes only a locked main-only boolean, bound files and validated artifact", () => {
  const operations = readFileSync(
    path.join(root, ".github/workflows/operations.yml"),
    "utf8",
  );
  const workflow = readFileSync(
    path.join(root, ".github/workflows/runtime-diagnostics.yml"),
    "utf8",
  );
  assert.match(
    operations,
    /diagnostics:[\s\S]*?group: schoolorbit-production\n\s+cancel-in-progress: false/,
  );
  assert.match(
    operations,
    /observe_academic148_curriculum: \$\{\{ fromJson\(inputs.options\).observe_academic148_curriculum == true \}\}/,
  );
  assert.match(
    workflow,
    /observe_academic148_curriculum:\n\s+type: boolean\n\s+default: false/,
  );
  assert.match(
    workflow,
    /github.ref == 'refs\/heads\/main' && inputs.observe_academic148_curriculum/,
  );
  assert.match(workflow, /academic148_observation=unavailable/);
  assert.match(
    workflow,
    /pipeline-runtime-observation-artifacts.py academic148/,
  );
  assert.match(workflow, /collect \$collector_sha \$sql_sha \$manifest_sha/);
  const interlock = workflow
    .split(
      "name: Refuse owned cleanup before academic148 observation uploads",
    )[1]
    .split("      - name: Observe academic148")[0];
  assert.match(
    interlock,
    /uses: \.\/\.github\/actions\/owned-cleanup-interlock/,
  );
  assert.match(
    interlock,
    /github.ref == 'refs\/heads\/main' && inputs.observe_academic148_curriculum/,
  );
  assert.ok(
    workflow.indexOf("name: Refuse owned cleanup before academic148") <
      workflow.indexOf("name: Observe academic148"),
  );
  assert.match(workflow, /sha256sum --check --status; exec timeout/);
  assert.doesNotMatch(
    workflow
      .split("name: Observe academic148 curriculum guard")[1]
      .split("name: Upload bounded runtime")[0],
    /inputs\.(?:subdomain|api_url|school_id)|cat .*stderr|cat .*report/,
  );
});

test("actual workflow transport keeps credentials and hostile output private before artifact promotion", (t) => {
  const workflow = readFileSync(
    path.join(root, ".github/workflows/runtime-diagnostics.yml"),
    "utf8",
  );
  const body = workflow
    .split("name: Observe academic148 curriculum guard")[1]
    .split("      - name: Retain academic148")[0]
    .split("        run: |\n")[1]
    .split("\n")
    .map((line) => line.slice(10))
    .join("\n");
  const report = python("print(json.dumps(m.aggregate([data])))", empty());
  for (const scenario of [
    "valid",
    "private-extra",
    "failed-ssh",
    "invalid-selector",
    "wrong-upload-digest",
    "active-cleanup",
  ]) {
    const temporary = mkdtempSync(
      path.join(os.tmpdir(), "academic148-workflow-"),
    );
    t.after(() => rmSync(temporary, { recursive: true, force: true }));
    const bin = path.join(temporary, "bin");
    mkdirSync(bin);
    const stub = `#!/usr/bin/env node
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const name=path.basename(process.argv[1]),args=process.argv.slice(2),root=process.env.FIXTURE_TEMP;
if(args.some(arg=>arg.includes('private-observation-sentinel')))throw Error('credential argument');
fs.appendFileSync(path.join(root,'events'),name+'\\n');
if(name==='podman')process.exit(0);
if(name==='ssh-keyscan'){console.log('fixed-host-key');process.exit(0);}
const key=args[args.indexOf('-i')+1];if(fs.statSync(key).mode&63)throw Error('public key mode');
if(name==='scp'){
 const files=args.slice(-4,-1);const hashes={};
 const remote=path.join(root,'remote');fs.mkdirSync(remote,{mode:448});
 for(const file of files){if((fs.statSync(file).mode&511)!==384)throw Error('public input mode');hashes[path.basename(file)]=crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');fs.copyFileSync(file,path.join(remote,path.basename(file)));}
 if(process.env.SCENARIO==='wrong-upload-digest')fs.appendFileSync(path.join(remote,'pipeline-academic148-observations.py'),'tampered');
 fs.writeFileSync(path.join(root,'hashes'),JSON.stringify(hashes));process.exit(0);
}
const command=args.at(-1);if(!command.includes(' collect '))process.exit(0);
const hashes=JSON.parse(fs.readFileSync(path.join(root,'hashes')));
const actual=command.trim().split(' ').slice(-3);
if(JSON.stringify(actual)!==JSON.stringify(['pipeline-academic148-observations.py','pipeline-academic148-observations.sql','manifest.json'].map(name=>hashes[name])))throw Error('unbound files');
if(process.env.SCENARIO==='failed-ssh'){console.error('private-observation-sentinel');process.exit(1);}
const remoteBin=path.join(root,'remote-bin');fs.mkdirSync(remoteBin);
fs.writeFileSync(path.join(remoteBin,'python3'),'#!/usr/bin/env node\\nconst fs=require("node:fs"),path=require("node:path");fs.writeFileSync(path.join(process.env.FIXTURE_TEMP,"python-started"),"yes");const report=JSON.parse(process.env.INPUT_REPORT);if(process.env.SCENARIO==="private-extra")report.private="private-observation-sentinel";console.log(JSON.stringify(report));',{mode:493});
const child=require('node:child_process').spawnSync('bash',['-euc',command.split('/opt/stack/deployment/academic148-observations').join(path.join(root,'remote'))],{env:{...process.env,PATH:remoteBin+':'+process.env.PATH},stdio:'inherit'});process.exit(child.status??1);
`;
    for (const command of ["ssh-keyscan", "ssh", "scp", "podman"])
      writeFileSync(path.join(bin, command), stub, { mode: 0o755 });
    const guard = path.join(
      root,
      "scripts/lib/pipeline-remote/owned-load-cleanup-interlock.sh",
    );
    const command =
      scenario === "active-cleanup" ? 'bash "$1" && bash -euc "$2"' : body;
    const result = spawnSync(
      "bash",
      ["-euc", command, "fixture", guard, body],
      {
        cwd: root,
        env: {
          ...process.env,
          PATH: bin + ":" + process.env.PATH,
          GITHUB_REF: "refs/heads/main",
          GITHUB_WORKSPACE: root,
          RUNNER_TEMP: temporary,
          SERVER_IP:
            scenario === "invalid-selector"
              ? "-private-observation-sentinel"
              : "127.0.0.1",
          SERVER_PORT: "22",
          SERVER_USER: "fixture",
          SSH_PRIVATE_KEY: sentinel,
          FIXTURE_TEMP: temporary,
          INPUT_REPORT: JSON.stringify(report),
          SCENARIO: scenario,
        },
        encoding: "utf8",
        timeout: 30000,
      },
    );
    assert.ok(!(result.stdout + result.stderr).includes(sentinel));
    const artifact = path.join(
      temporary,
      "runtime-observation-artifacts/academic148-curriculum.json",
    );
    if (scenario === "valid") {
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(readFileSync(artifact)), report);
    } else {
      assert.notEqual(result.status, 0);
      assert.equal(existsSync(artifact), false);
    }
    assert.equal(
      readdirSync(temporary).some(
        (name) =>
          name.startsWith("academic148-ssh.") || name === "academic148-inputs",
      ),
      false,
    );
    if (scenario === "invalid-selector")
      assert.equal(existsSync(path.join(temporary, "events")), false);
    if (scenario === "wrong-upload-digest" || scenario === "active-cleanup")
      assert.equal(existsSync(path.join(temporary, "python-started")), false);
    if (scenario === "active-cleanup") {
      assert.equal(result.status, 78);
      assert.equal(
        readFileSync(path.join(temporary, "events"), "utf8"),
        "podman\n",
      );
    }
  }
});
