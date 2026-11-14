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
test('academic acceptance reads the exact current singleton and reports absent context without creating resources',t=>{
 const directory=mkdtempSync(path.join(os.tmpdir(),'academic-current-selection-'));t.after(()=>rmSync(directory,{recursive:true,force:true}));
 const year='00000000-0000-0000-0000-000000000001',term='00000000-0000-0000-0000-000000000002',current='00000000-0000-0000-0000-000000000003',other='00000000-0000-0000-0000-000000000004';
 const timetable={id:current,academicYearId:year,academicTermId:term,bellScheduleId:other,rowVersion:1};
 const cases=[
  {name:'current singleton',payload:{success:true,data:timetable},current:true},
  {name:'no current timetable',payload:{success:true,data:null}},
  {name:'foreign term',payload:{success:true,data:{...timetable,academicTermId:other}},invalid:true},
  {name:'foreign year',payload:{success:true,data:{...timetable,academicYearId:other}},invalid:true},
  {name:'malformed identity',payload:{success:true,data:{...timetable,id:'invalid'}},invalid:true},
  {name:'retired list shape',payload:{success:true,data:[timetable]},invalid:true},
  {name:'missing data',payload:{success:true},invalid:true},
  {name:'failed response',payload:{success:false,data:timetable},invalid:true},
  {name:'invalid technical revision',payload:{success:true,data:{...timetable,rowVersion:true}},invalid:true},
  {name:'no academic context',payload:{success:true,data:null},noContext:true},
 ];
 for(const fixture of cases){
  writeFileSync(path.join(directory,'context.json'),JSON.stringify({success:true,data:{defaultAcademicYearId:year,defaultAcademicTermId:term,years:fixture.noContext?[]:[{id:year}],terms:fixture.noContext?[]:[{id:term,academicYearId:year}]}}));
  writeFileSync(path.join(directory,'timetable.json'),JSON.stringify(fixture.payload));
  writeFileSync(path.join(directory,'requests'),'');writeFileSync(path.join(directory,'sockets'),'');
  const result=spawnSync('bash',['-euc',`
tmp_dir=$1
academic_context_get() {
 printf '%s\\t%s\\n' "$1" "$3" >>"$tmp_dir/requests"
 case "$1" in context) cp "$tmp_dir/context.json" "$tmp_dir/academic-context.body";; timetable-1) cp "$tmp_dir/timetable.json" "$tmp_dir/academic-$1.body";; esac
}
timetable_websocket_smoke() { printf '%s\\t%s\\t%s\\t%s\\n' "$2" "$3" "$4" "$5" >>"$tmp_dir/sockets"; }
pass() { :; }
fail() { printf '%s\\n' "$1" >&2; exit 1; }
${academicSource}
run_academic_context_smoke
`,'academic-selection',directory],{encoding:'utf8'});
  assert.equal(result.status,fixture.invalid?1:0,fixture.name+result.stderr);
  const requests=readFileSync(path.join(directory,'requests'),'utf8');
  const sockets=readFileSync(path.join(directory,'sockets'),'utf8').split('\n').filter(Boolean);
  assert.equal(requests.includes(`/api/academic/timetable?academicTermId=${term}`),!fixture.noContext,fixture.name);
  assert.equal(requests.includes(`timetableId=${current}`),Boolean(fixture.current),fixture.name);
  assert.equal(sockets.length,fixture.noContext?0:fixture.current?2:1,fixture.name);
  if(!fixture.noContext)assert.ok(sockets[0].startsWith(`${year}\t${term}\t\t`),fixture.name);
  if(fixture.current)assert.ok(sockets[1].startsWith(`${year}\t${term}\t${current}\t${directory}/academic-timetable-workspace-1.body`),fixture.name);
  else if(!fixture.invalid)assert.match(result.stdout,/SKIP .*protocol.*unrun/,fixture.name);
  assert.doesNotMatch(requests,/POST|PUT|PATCH|DELETE|timetable-versions|timetableVersionId|\/clone|\/publish/);
 }
});
