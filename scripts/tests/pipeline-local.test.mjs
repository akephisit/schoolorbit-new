import assert from 'node:assert/strict';
import {execFileSync,spawnSync} from 'node:child_process';
import {cpSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
const repo=path.resolve(import.meta.dirname,'../..');
function fixture(t) {
 const root=mkdtempSync(path.join(os.tmpdir(),'pipeline-local-'));t.after(()=>rmSync(root,{recursive:true,force:true}));
 for(const directory of ['scripts/lib','scripts/tests','backend-school/target/debug','backend-admin','bin'])mkdirSync(path.join(root,directory),{recursive:true});
 for(const source of ['scripts/pipeline.mjs','scripts/lib/pipeline-policy.mjs'])cpSync(path.join(repo,source),path.join(root,source));
 writeFileSync(path.join(root,'scripts/generate-api-contracts.mjs'),`import {writeFileSync} from 'node:fs';\nwriteFileSync(${JSON.stringify(path.join(root,'backend-school/target/debug/backend-school'))}, '#!/bin/sh\\nprintf "{}"\\n', {mode:0o755});\n`);
 writeFileSync(path.join(root,'scripts/tests/postgres-compatibility.test.mjs'),"import test from 'node:test';test('fixture',()=>{});\n");
 writeFileSync(path.join(root,'scripts/test_school_database_suite.sh'),'#!/bin/sh\ntouch fixture-executed\n');
 cpSync(path.join(repo,'scripts/test_backend_admin.sh'),path.join(root,'scripts/test_backend_admin.sh'));
 mkdirSync(path.join(root,'backend-admin/migrations'));
 writeFileSync(path.join(root,'backend-admin/migrations/001.sql'),'SELECT 1;');
 writeFileSync(path.join(root,'bin/docker'),'#!/bin/sh\nif [ "$1" = port ]; then printf "127.0.0.1:15432\\n"; elif [ "$1" = exec ]; then cat >/dev/null; fi\n',{mode:0o755});
 writeFileSync(path.join(root,'backend-school/target/debug/backend-school'),'#!/bin/sh\nprintf "{}"\n',{mode:0o755});
 writeFileSync(path.join(root,'bin/cargo'),`#!/usr/bin/env node
const fs=require('node:fs');const args=process.argv.slice(2);
fs.appendFileSync(process.env.COMMAND_LOG,JSON.stringify(args)+'\\n');
if(args[0]==='clippy'&&process.env.FIXTURE_CLIPPY_FAIL===(args.includes('api-docs')?'api-docs':'default'))process.exit(42);
if(args[0]==='metadata')process.stdout.write(JSON.stringify({target_directory:${JSON.stringify(path.join(root,'backend-school/target'))}}));
if(args[0]==='tree'&&process.env.FIXTURE_DOCS_DEP)process.stdout.write('utoipa v5.5.0\\n');
if(args[0]==='build'&&!process.env.FIXTURE_DOCS_LEAK)fs.writeFileSync(${JSON.stringify(path.join(root,'backend-school/target/debug/backend-school'))}, '#!/bin/sh\\necho "OpenAPI export requires --features api-docs" >&2\\nexit 2\\n', {mode:0o755});
`,{mode:0o755});
 execFileSync('git',['init','-q',root]);execFileSync('git',['-C',root,'config','user.email','fixture@example.invalid']);execFileSync('git',['-C',root,'config','user.name','Fixture']);
 execFileSync('git',['-C',root,'add','.']);execFileSync('git',['-C',root,'commit','-qm','fixture']);
 const env={...process.env,PATH:path.join(root,'bin')+':'+process.env.PATH,COMMAND_LOG:path.join(root,'commands')};
 return {root,env,run:(extra={})=>spawnSync(process.execPath,[path.join(root,'scripts/pipeline.mjs'),'prime','--scope','backend-school'],{cwd:root,env:{...env,...extra},encoding:'utf8'})};
}
test('ordinary verification explicitly refuses an ambient compile-only flag for Admin fixtures',t=>{
 const f=fixture(t);
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'verify','--scope','backend-admin'],{cwd:f.root,env:{...f.env,SCHOOLORBIT_COMPILE_ONLY:'true'},encoding:'utf8'});
 assert.equal(result.status,0,result.stderr);
 const commands=readFileSync(f.env.COMMAND_LOG,'utf8').trim().split('\n').map(JSON.parse);
 assert.ok(commands.some(args=>args[0]==='test'&&args.includes('--locked')&&!args.includes('--no-run')));
 const clippy=commands.find(args=>args[0]==='clippy');
 assert.ok(clippy);
 for(const required of ['--lib','--bins','--locked','--no-deps','clippy::unwrap_used','clippy::expect_used','clippy::panic','clippy::unreachable'])assert.ok(clippy.includes(required),required);
 assert.ok(!clippy.includes('--all-targets')&&!clippy.includes('warnings'));
});
test('removed priming command cannot execute Cargo or fixtures',t=>{
 const f=fixture(t);const result=f.run();assert.notEqual(result.status,0);
 assert.equal(spawnSync('test',['-e',f.env.COMMAND_LOG]).status,1);
});
test('release planning emits bounded per-component selection reasons without changing its selected scope', t => {
 const f=fixture(t);
 execFileSync('git',['-C',f.root,'update-ref','refs/remotes/origin/main','HEAD']);
 const base=execFileSync('git',['-C',f.root,'rev-parse','HEAD'],{encoding:'utf8'}).trim();
 const summary=path.join(f.root,'summary'), output=path.join(f.root,'output');
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'plan','--release','true','--base',base],{
  cwd:f.root,env:{...f.env,GITHUB_STEP_SUMMARY:summary,GITHUB_OUTPUT:output},encoding:'utf8'
 });
 assert.equal(result.status,0,result.stderr);
 const data=readFileSync(output,'utf8').split('\n').find(line=>line.startsWith('plan='));
 const plan=JSON.parse(data.slice(5));
 assert.equal(plan.deploy.length,4);
 assert.ok(Object.values(plan.reasons).every(reasons=>reasons.join(',')==='missing-accepted-baseline'));
 assert.deepEqual(JSON.parse(result.stdout),plan,'stdout remains the machine-readable plan even inside Actions');
 assert.match(result.stderr,/::notice title=Component selection::backend-school selected=true reasons=missing-accepted-baseline/);
 assert.match(readFileSync(summary,'utf8'),/frontend-admin \| deploy \| missing-accepted-baseline/);
 assert.ok(readFileSync(summary).length<4096);
});
test('local School verification executes tests and database fixtures rather than compiling only',t=>{
 const f=fixture(t);
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'verify','--scope','backend-school'],{cwd:f.root,env:f.env,encoding:'utf8'});
 assert.equal(result.status,0,result.stderr);
 const commands=readFileSync(f.env.COMMAND_LOG,'utf8').trim().split('\n').map(JSON.parse);
 const tests=commands.filter(args=>args[0]==='test');assert.ok(tests.length>=3);
 assert.ok(tests.every(args=>!args.includes('--no-run')));
 assert.ok(commands.some(args=>args[0]==='check'&&!args.includes('--features')));
 assert.ok(commands.some(args=>args[0]==='check'&&args.includes('api-docs')));
 assert.ok(tests.some(args=>args.includes('api_contract::tests')&&args.includes('api-docs')));
 assert.ok(tests.some(args=>args.includes('school-staff')&&args.includes('api-docs')));
 const clippy=commands.filter(args=>args[0]==='clippy');
 assert.equal(clippy.length,2);
 for(const args of clippy){
  for(const required of ['--workspace','--lib','--bins','--locked','--no-deps','school-test-db','clippy::unwrap_used','clippy::expect_used','clippy::panic','clippy::unreachable'])assert.ok(args.includes(required),required);
  assert.ok(!args.includes('--all-targets')&&!args.includes('warnings'));
 }
 assert.equal(clippy.filter(args=>args.includes('api-docs')).length,1);
 assert.equal(spawnSync('test',['-e',path.join(f.root,'fixture-executed')]).status,0);
});
for (const [flag, message] of [['FIXTURE_DOCS_LEAK', /Production binary must reject OpenAPI/], ['FIXTURE_DOCS_DEP', /Production dependency graph must exclude Utoipa/]]) test(`School verification refuses production documentation leakage (${flag})`,t=>{
 const f=fixture(t);
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'verify','--scope','backend-school'],{cwd:f.root,env:{...f.env,[flag]:'1'},encoding:'utf8'});
 assert.notEqual(result.status,0);
 assert.match(result.stderr,message);
 assert.equal(spawnSync('test',['-e',path.join(f.root,'fixture-executed')]).status,1);
});

for(const [scope,mode] of [['backend-school','default'],['backend-school','api-docs'],['backend-admin','default']])test(`production panic lint failure stops ${scope} verification (${mode})`,t=>{
 const f=fixture(t);
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'verify','--scope',scope],{cwd:f.root,env:{...f.env,FIXTURE_CLIPPY_FAIL:mode},encoding:'utf8'});
 assert.notEqual(result.status,0);
 assert.match(result.stderr,/cargo failed \(42\)|bash failed \(42\)/);
 const commands=readFileSync(f.env.COMMAND_LOG,'utf8').trim().split('\n').map(JSON.parse);
 assert.equal(commands.at(-1)[0],'clippy');
 assert.equal(spawnSync('test',['-e',path.join(f.root,'fixture-executed')]).status,1);
});
