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
 writeFileSync(path.join(root,'scripts/generate-api-contracts.mjs'),'// fixture exporter check\n');
 writeFileSync(path.join(root,'scripts/tests/neon-compatibility.test.mjs'),"import test from 'node:test';test('fixture',()=>{});\n");
 writeFileSync(path.join(root,'scripts/tests/neon-create-test-branch.test.mjs'),"import test from 'node:test';test('fixture',()=>{});\n");
 writeFileSync(path.join(root,'scripts/test_school_database_suite.sh'),'#!/bin/sh\ntouch fixture-executed\n');
 cpSync(path.join(repo,'scripts/test_backend_admin.sh'),path.join(root,'scripts/test_backend_admin.sh'));
 mkdirSync(path.join(root,'backend-admin/migrations'));
 writeFileSync(path.join(root,'backend-admin/migrations/001.sql'),'SELECT 1;');
 writeFileSync(path.join(root,'bin/docker'),'#!/bin/sh\nif [ "$1" = port ]; then printf "127.0.0.1:15432\\n"; elif [ "$1" = exec ]; then cat >/dev/null; fi\n',{mode:0o755});
 writeFileSync(path.join(root,'backend-school/target/debug/backend-school'),'#!/bin/sh\nprintf "{}"\n',{mode:0o755});
 writeFileSync(path.join(root,'bin/cargo'),`#!/usr/bin/env node
const fs=require('node:fs');const args=process.argv.slice(2);
fs.appendFileSync(process.env.COMMAND_LOG,JSON.stringify(args)+'\\n');
if(args[0]==='metadata')process.stdout.write(JSON.stringify({target_directory:${JSON.stringify(path.join(root,'backend-school/target'))}}));
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
});
test('removed priming command cannot execute Cargo or fixtures',t=>{
 const f=fixture(t);const result=f.run();assert.notEqual(result.status,0);
 assert.equal(spawnSync('test',['-e',f.env.COMMAND_LOG]).status,1);
});
test('local School verification executes tests and database fixtures rather than compiling only',t=>{
 const f=fixture(t);
 const result=spawnSync(process.execPath,[path.join(f.root,'scripts/pipeline.mjs'),'verify','--scope','backend-school'],{cwd:f.root,env:f.env,encoding:'utf8'});
 assert.equal(result.status,0,result.stderr);
 const commands=readFileSync(f.env.COMMAND_LOG,'utf8').trim().split('\n').map(JSON.parse);
 const tests=commands.filter(args=>args[0]==='test');assert.ok(tests.length>=3);
 assert.ok(tests.every(args=>!args.includes('--no-run')));
 assert.equal(spawnSync('test',['-e',path.join(f.root,'fixture-executed')]).status,0);
});
