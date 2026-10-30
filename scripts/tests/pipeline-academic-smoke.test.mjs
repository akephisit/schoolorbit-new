import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';

const helper=path.resolve(import.meta.dirname,'../lib/pipeline-remote/school-verify-academic.sh');
test('Academic maintenance smoke checks both loopback APIs while preserving tenant and authentication requirements',t=>{
 const stack=mkdtempSync(path.join(os.tmpdir(),'academic-private-smoke-'));t.after(()=>rmSync(stack,{recursive:true,force:true}));
 const scripts=path.join(stack,'deployment/scripts');mkdirSync(path.join(scripts,'lib/schoolorbit-installer/remote'),{recursive:true});
 writeFileSync(path.join(scripts,'lib/schoolorbit-installer/remote/deployment_timing.sh'),'schoolorbit_timer_now() { echo 1; }\nschoolorbit_timer_report() { :; }\n');
 writeFileSync(path.join(scripts,'smoke_test.sh'),`#!/bin/bash
set -eu
[[ $SMOKE_API_URL == http://localhost:8081 ]]
[[ $SMOKE_ADMIN_API_URL == http://localhost:8080 ]]
[[ $SMOKE_DIRECT_BACKEND == true && $SMOKE_REQUIRE_AUTH == true && $SMOKE_ACADEMIC_CONTEXT == true ]]
[[ $SMOKE_SUBDOMAIN == sandbox && $SMOKE_TENANT_URL == https://sandbox.example.test && $SMOKE_ORIGIN == https://sandbox.example.test ]]
[[ $SMOKE_USERNAME == fixture-user && $SMOKE_PASSWORD == fixture-password ]]
`);
 const result=spawnSync('bash',[helper],{encoding:'utf8',env:{...process.env,SCHOOLORBIT_STACK_ROOT:stack,BASE_DOMAIN:'example.test',ACADEMIC_CORE_SMOKE_SUBDOMAIN:'sandbox',SMOKE_USERNAME:'fixture-user',SMOKE_PASSWORD:'fixture-password'}});
 assert.equal(result.status,0,result.stderr);assert.doesNotMatch(result.stdout+result.stderr,/fixture-password/);
});

const smokeSource=readFileSync(new URL('../smoke_test.sh',import.meta.url),'utf8');
const academicSource=smokeSource.slice(smokeSource.indexOf('run_academic_context_smoke() {'),smokeSource.indexOf('# Academic Core maintenance read-only smoke end'));
test('academic acceptance preserves published reads, selects only existing in-term drafts and reports missing context honestly',t=>{
 const directory=mkdtempSync(path.join(os.tmpdir(),'academic-ws-selection-'));t.after(()=>rmSync(directory,{recursive:true,force:true}));
 const year='00000000-0000-0000-0000-000000000001',term='00000000-0000-0000-0000-000000000002',published='00000000-0000-0000-0000-000000000003',draft='00000000-0000-0000-0000-000000000004',other='00000000-0000-0000-0000-000000000005';
 const version=(id,status,academicTermId=term)=>({id,status,academicTermId});
 const cases=[
  {name:'published and draft',versions:[version(published,'published'),version(draft,'draft')],draft:true,published:true},
  {name:'draft only',versions:[version(draft,'draft')],draft:true,published:false},
  {name:'no draft',versions:[version(published,'published')],draft:false,published:true},
  {name:'foreign and invalid drafts',versions:[version(draft,'draft',other),version('invalid','draft')],draft:false,published:false},
  {name:'no context',versions:[],noContext:true,draft:false,published:false},
 ];
 for(const fixture of cases){
  writeFileSync(path.join(directory,'context.json'),JSON.stringify({success:true,data:{activeAcademicYearId:year,activeAcademicTermId:term,years:fixture.noContext?[]:[{id:year}],terms:fixture.noContext?[]:[{id:term,academicYearId:year}]}}));
  writeFileSync(path.join(directory,'versions.json'),JSON.stringify({success:true,data:fixture.versions}));
  writeFileSync(path.join(directory,'requests'),'');writeFileSync(path.join(directory,'sockets'),'');
  const result=spawnSync('bash',['-euc',`
tmp_dir=$1
academic_context_get() {
 printf '%s\\t%s\\n' "$1" "$3" >>"$tmp_dir/requests"
 case "$1" in context) cp "$tmp_dir/context.json" "$tmp_dir/academic-context.body";; timetable-versions-*) cp "$tmp_dir/versions.json" "$tmp_dir/academic-$1.body";; esac
}
timetable_websocket_smoke() { printf '%s\\t%s\\t%s\\t%s\\n' "$2" "$3" "$4" "$5" >>"$tmp_dir/sockets"; }
pass() { :; }
fail() { printf '%s\\n' "$1" >&2; exit 1; }
${academicSource}
run_academic_context_smoke
`,'academic-selection',directory],{encoding:'utf8'});
  assert.equal(result.status,0,fixture.name+result.stderr);
  const requests=readFileSync(path.join(directory,'requests'),'utf8');
  const sockets=readFileSync(path.join(directory,'sockets'),'utf8').split('\n').filter(Boolean);
  assert.equal(requests.includes(`timetableVersionId=${published}`),fixture.published,fixture.name);
  assert.equal(requests.includes(`timetableVersionId=${draft}`),fixture.draft,fixture.name);
  assert.equal(sockets.length,fixture.noContext?0:fixture.draft?2:1,fixture.name);
  if(!fixture.noContext)assert.ok(sockets[0].startsWith(`${year}\t${term}\t`));
  if(fixture.draft)assert.ok(sockets[1].startsWith(`${year}\t${term}\t${draft}\t`));
  else assert.match(result.stdout,/SKIP .*protocol.*unrun/);
  assert.doesNotMatch(requests,/POST|PUT|PATCH|DELETE/);
 }
});
