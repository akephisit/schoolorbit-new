import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
const script = path.join(repo, 'scripts/benchmark_backend_cache.py');
const api = {openapi: '3.1.0', paths: {'/fixture': {}}};
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'cache-benchmark-'));
  t.after(() => rmSync(root, {recursive:true,force:true}));
  const files = {
    'backend-school/Cargo.toml': '[workspace]\nmembers = [\n]\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n',
    'backend-school/Cargo.lock': 'version = 4\n',
    'backend-school/Dockerfile': 'FROM base AS builder\nRUN cargo build\n',
    'backend-school/scripts/build_release.sh': 'export SCCACHE_GHA_VERSION=schoolorbit-backend-school\n',
    'backend-school/src/main.rs': 'fn main() {}\n',
    'contracts/openapi/school-api.json': JSON.stringify(api),
  };
  for (const [file,content] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root,file)),{recursive:true});
    writeFileSync(path.join(root,file),content);
  }
  const git = (...args) => execFileSync('git',['-C',root,...args],{stdio:'pipe',encoding:'utf8'}).trim();
  git('init','-q'); git('config','user.name','Fixture'); git('config','user.email','fixture@example.invalid');
  git('add','.'); git('commit','-qm','fixture');
  return {root,git};
}
function python(root, code, extraEnv={}) {
  return spawnSync('python3',['-c', `import sys, pathlib, json, os\nsys.path.insert(0, ${JSON.stringify(path.join(repo,'scripts'))})\nimport benchmark_backend_cache as b\nroot=pathlib.Path(${JSON.stringify(root)})\n${code}`], {
    cwd:root,encoding:'utf8',env:{...process.env,PYTHONDONTWRITEBYTECODE:'1',...extraEnv}
  });
}
test('paired contexts preserve source/flags and isolate compiler writes while recipe samples change workspace identity', t => {
  const f=fixture(t);
  const result=python(f.root, `
hashes=[b.context(root,root/'a','cache-benchmark-123-1','source',1),b.context(root,root/'b','cache-benchmark-123-1','source',1)]
assert hashes[0]==hashes[1]
assert (root/'a/src/main.rs').read_text().endswith('source sample 1\\n')
assert 'SCCACHE_GHA_RW_MODE=READ_ONLY' in (root/'a/Dockerfile').read_text()
assert 'cache-benchmark-123-1' in (root/'a/scripts/build_release.sh').read_text()
for sample in (1,2):
 p=root/f'workspace-{sample}'
 b.context(root,p,'cache-benchmark-123-1','workspace',sample)
 import tomllib
 manifest=tomllib.loads((p/'Cargo.toml').read_text())
 lock=tomllib.loads((p/'Cargo.lock').read_text())
 assert manifest['workspace']['members']==[f'crates/cache-benchmark-fixture-{sample}']
 assert lock['package']==[{'name':f'cache-benchmark-fixture-{sample}','version':'0.1.0'}]
assert (root/'backend-school/src/main.rs').read_text()=='fn main() {}\\n'
`);
  assert.equal(result.status,0,result.stderr);
  assert.equal(f.git('diff','--name-only'),'');
});
test('summary keeps cold/seed costs separate and marks missing export evidence unavailable',t=>{
  const f=fixture(t);
  const result=python(f.root, `
records=[dict(backend='gha',scenario=case,sample=i,wall_seconds=seconds,phases={},network_received_bytes=1024,network_sent_bytes=2048)
 for case,i,seconds in [('seed',0,999),('source',1,100),('source',2,120)]]
print(b.report(records))
`);
  assert.equal(result.status,0,result.stderr);
  assert.match(result.stdout,/\| seed \| gha \| 1 \| 999.00s/);
  assert.match(result.stdout,/\| source \| gha \| 2 \| 110.00s \| unavailable/);
});
const fakeDocker = `#!/usr/bin/env python3
import json,os,pathlib,sys
a=sys.argv[1:]; root=pathlib.Path(os.environ['FAKE_ROOT'])
with (root/'docker-calls.jsonl').open('a') as f: f.write(json.dumps(a)+'\\n')
if a[:2]==['buildx','build']:
 if '--load' in a:
  if os.environ.get('FAIL_BUILD'): sys.exit(42)
  print('#2 [builder] RUN bash scripts/build_release.sh dependencies\\n#2 CACHED\\n#3 [builder] RUN bash scripts/build_release.sh\\n#3 DONE 190.0s\\n#4 exporting cache\\n#4 DONE 30.0s')
 else:
  dest=pathlib.Path(a[a.index('--output')+1].split('dest=',1)[1]); dest.mkdir(parents=True)
  for prefix in ['', 'dependency-']:
   (dest/(prefix+'build-metrics.json')).write_text(json.dumps({'compileNanoseconds':1000000000,'cacheEnabled':True}))
   stats={'cache_hits':{'counts':{'Rust':31}},'cache_misses':{'counts':{}},'cache_errors':{'counts':{}},'cache_read_errors':0,'cache_write_errors':0,'cache_timeouts':0,'cache_writes':int(os.environ.get('BAD_WRITES','0'))}
   if os.environ.get('MISSING_COUNTER'): stats.pop('cache_hits')
   (dest/(prefix+'sccache-stats.json')).write_text(json.dumps({'stats':stats}))
elif a[:3]==['buildx','imagetools','inspect']: print(json.dumps({'layers':[{'size':1024},{'size':2048}]}))
elif a[:2]==['image','inspect']: print(json.dumps([{'Id':'sha256:fixture','Config':{'Cmd':['/app/backend-school']}}]))
elif a[0]=='exec': print('eth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0')
elif a[0]=='run':
 if '--entrypoint' in a and a[a.index('--entrypoint')+1]=='id': print(os.environ.get('FIXTURE_UID','1000'))
 else: print(os.environ['FIXTURE_API'])
`;
for (const mode of ['gha','registry','failure','missing','writes','api','uid']) test(`fresh builder orchestration validates cache and cleans up (${mode})`,t=>{
  const f=fixture(t),bin=path.join(f.root,'bin'); mkdirSync(bin);
  writeFileSync(path.join(bin,'docker'),fakeDocker,{mode:0o755});
  const result=python(f.root, `
bench=b.Benchmark(root,root/'results','cache-benchmark-123-1','fixture@sha256:'+'a'*64)
record=bench.build(root/'backend-school','same-source',${JSON.stringify(mode==='registry'?'registry':'gha')},'source',1)
assert record['compiler']['application']['cache_hits']==31
assert record['phases']['dependency layer']['cached'] is True
assert record['openapi_equal'] is True
if record['backend']=='registry': assert record['registry_cache_layer_bytes']==3072
`, {PATH:bin+':'+process.env.PATH,FAKE_ROOT:f.root,FIXTURE_API:JSON.stringify(mode==='api'?{paths:{'/wrong':{}}}:api),GITHUB_REPOSITORY_OWNER:'fixture',
      ...(mode==='uid'?{FIXTURE_UID:'2000'}:{}),
      ...(mode==='failure'?{FAIL_BUILD:'1'}:{}),...(mode==='missing'?{MISSING_COUNTER:'1'}:{}),...(mode==='writes'?{BAD_WRITES:'1'}:{})});
  if (['failure','missing','writes','api','uid'].includes(mode)) assert.notEqual(result.status,0);
  else assert.equal(result.status,0,result.stderr);
  const calls=readFileSync(path.join(f.root,'docker-calls.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
  const build=calls.find(c=>c.includes('--load'));
  assert.ok(build.includes('--cache-from'));
  assert.match(build[build.indexOf('--cache-to')+1],mode==='registry'?/type=registry.*mode=max/:/type=gha.*version=2.*mode=max/);
  assert.ok(build.includes('id=sccache_gha_token,env=ACTIONS_RUNTIME_TOKEN'));
  assert.ok(calls.some(c=>c.slice(0,3).join(' ')==='buildx rm --force'));
  assert.ok(calls.some(c=>c.slice(0,2).join(' ')==='image rm'));
});

test('complete experiment counterbalances pairs and uses a distinct fresh builder for every measurement',t=>{
  const f=fixture(t),bin=path.join(f.root,'bin'); mkdirSync(bin);
  writeFileSync(path.join(bin,'docker'),fakeDocker,{mode:0o755});
  const result=python(f.root, `
b.__file__=str(root/'scripts/benchmark_backend_cache.py')
b.main()
records=json.loads((root/'output/results.json').read_text())
assert len(records)==10
for scenario,sample in [('seed',0),('source',1),('source',2),('workspace',1),('workspace',2)]:
 pair=[r for r in records if r['scenario']==scenario and r['sample']==sample]
 assert len(pair)==2 and pair[0]['context_sha256']==pair[1]['context_sha256']
assert [r['backend'] for r in records]==['gha','registry','registry','gha','gha','registry','registry','gha','gha','registry']
assert json.loads((root/'output/complete.json').read_text())['complete'] is True
`, {PATH:bin+':'+process.env.PATH,FAKE_ROOT:f.root,FIXTURE_API:JSON.stringify(api),GITHUB_REPOSITORY_OWNER:'fixture',
    GITHUB_RUN_ID:'123',GITHUB_RUN_ATTEMPT:'1',BENCH_SAMPLES:'2',BENCH_OUTPUT:path.join(f.root,'output'),
    BENCH_BUILDKIT_IMAGE:'fixture@sha256:'+'a'.repeat(64),ACTIONS_RUNTIME_TOKEN:'private-fixture-sentinel',ACTIONS_RESULTS_URL:'private-fixture-sentinel'});
  assert.equal(result.status,0,result.stderr);
  assert.doesNotMatch(result.stdout+result.stderr,/private-fixture-sentinel/);
  const calls=readFileSync(path.join(f.root,'docker-calls.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
  const builders=calls.filter(c=>c.slice(0,2).join(' ')==='buildx create').map(c=>c[c.indexOf('--name')+1]);
  assert.equal(builders.length,11);
  assert.equal(new Set(builders).size,11);
  assert.equal(calls.filter(c=>c.slice(0,3).join(' ')==='buildx rm --force').length,11);
});

test('invalid sample counts are rejected before Docker or source mutation',t=>{
  const f=fixture(t);
  for(const sample of ['1','4','2; touch injected']) {
    const result=python(f.root,'b.main()', {BENCH_SAMPLES:sample});
    assert.notEqual(result.status,0);
    assert.match(result.stderr,/Invalid bounded benchmark inputs/);
  }
  assert.equal(f.git('diff','--name-only'),'');
});

for (const status of ['present','missing','forbidden']) test(`registry cleanup is confined to the run-owned package (${status})`,t=>{
  const f=fixture(t), bin=path.join(f.root,'bin'); mkdirSync(bin);
  writeFileSync(path.join(bin,'gh'), '#!/bin/bash\nprintf "%s\\n" "$*" >> "$FAKE_ROOT/gh-calls"\nif [[ "$*" == *"GET"* ]]; then\ncase "$PACKAGE_STATUS" in missing) echo "HTTP 404" >&2; exit 1;; forbidden) echo "HTTP 403" >&2; exit 1;; esac\nfi\n', {mode:0o755});
  const workflow=readFileSync(path.join(repo,'.github/workflows/backend-cache-benchmark.yml'),'utf8');
  const body=workflow.match(/name: Remove only this run's disposable registry package[\s\S]*?        run: \|\n([\s\S]*)/)[1].replace(/^          /gm,'');
  const result=spawnSync('bash',['-c',body],{cwd:f.root,encoding:'utf8',env:{...process.env,PATH:bin+':'+process.env.PATH,
    FAKE_ROOT:f.root,PACKAGE_STATUS:status,OWNER:'fixture',OWNER_TYPE:'User',GITHUB_RUN_ID:'123',GITHUB_RUN_ATTEMPT:'1',RUNNER_TEMP:f.root}});
  assert.equal(result.status,status==='forbidden'?1:0,result.stderr);
  const calls=readFileSync(path.join(f.root,'gh-calls'),'utf8');
  assert.match(calls,/GET \/users\/fixture\/packages\/container\/schoolorbit-cache-benchmark-123-1/);
  assert.equal(calls.includes('DELETE'),status==='present');
  assert.doesNotMatch(calls,/schoolorbit-backend-school|schoolorbit-backend-admin/);
});
