import assert from 'node:assert/strict';
import {execFileSync,spawnSync} from 'node:child_process';
import {cpSync,mkdirSync,mkdtempSync,readFileSync,rmSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
const repo=path.resolve(import.meta.dirname,'../..');
function fixture(t) {
 const root=mkdtempSync(path.join(os.tmpdir(),'pipeline-prime-'));t.after(()=>rmSync(root,{recursive:true,force:true}));
 for(const directory of ['scripts/lib','scripts/tests','backend-school/target/debug','backend-admin','bin'])mkdirSync(path.join(root,directory),{recursive:true});
 for(const source of ['scripts/pipeline.mjs','scripts/lib/pipeline-policy.mjs'])cpSync(path.join(repo,source),path.join(root,source));
 writeFileSync(path.join(root,'scripts/generate-api-contracts.mjs'),'// fixture exporter check\n');
 writeFileSync(path.join(root,'scripts/tests/neon-compatibility.test.mjs'),"import test from 'node:test';test('fixture',()=>{});\n");
 writeFileSync(path.join(root,'scripts/test_school_database_suite.sh'),'#!/bin/sh\ntouch fixture-executed\n');
 writeFileSync(path.join(root,'backend-school/target/debug/backend-school'),'#!/bin/sh\nprintf "{}"\n',{mode:0o755});
 writeFileSync(path.join(root,'bin/cargo'),`#!/usr/bin/env node
const fs=require('node:fs');const args=process.argv.slice(2);
fs.appendFileSync(process.env.COMMAND_LOG,JSON.stringify(args)+'\\n');
if(args[0]==='metadata')process.stdout.write(JSON.stringify({target_directory:${JSON.stringify(path.join(root,'backend-school/target'))}}));
`,{mode:0o755});
 execFileSync('git',['init','-q',root]);execFileSync('git',['-C',root,'config','user.email','fixture@example.invalid']);execFileSync('git',['-C',root,'config','user.name','Fixture']);
 execFileSync('git',['-C',root,'add','.']);execFileSync('git',['-C',root,'commit','-qm','fixture']);
 const tree=execFileSync('git',['-C',root,'rev-parse','HEAD^{tree}'],{encoding:'utf8'}).trim();
 const env={...process.env,PATH:path.join(root,'bin')+':'+process.env.PATH,GITHUB_REF:'refs/heads/main',GITHUB_EVENT_NAME:'push',COMMAND_LOG:path.join(root,'commands'),PR_VERIFICATION_PROOF:JSON.stringify({runId:'21',attempt:1,tree,suites:['backend-school']})};
 return {root,env,run:(extra={})=>spawnSync(process.execPath,[path.join(root,'scripts/pipeline.mjs'),'prime','--scope','backend-school'],{cwd:root,env:{...env,...extra},encoding:'utf8'})};
}
test('proven main priming compiles every selected test target without executing database fixtures',t=>{
 const f=fixture(t);const result=f.run();assert.equal(result.status,0,result.stderr);
 const commands=readFileSync(f.env.COMMAND_LOG,'utf8').trim().split('\n').map(JSON.parse);
 const tests=commands.filter(args=>args[0]==='test');assert.equal(tests.length,8);assert.ok(tests.every(args=>args.includes('--no-run')));
 assert.ok(tests.some(args=>args.includes('delivery_versions')));assert.ok(tests.some(args=>args.includes('seed_sandbox')));
 for(const pkg of ['school-auth','school-navigation','school-certificates'])assert.ok(tests.some(args=>args.includes(pkg)),pkg);
 assert.equal(spawnSync('test',['-e',path.join(f.root,'fixture-executed')]).status,1);
});
for(const extra of [{PR_VERIFICATION_PROOF:'null'},{GITHUB_REF:'refs/pull/7/merge'},{GITHUB_EVENT_NAME:'pull_request_target'},{PR_VERIFICATION_PROOF:JSON.stringify({runId:'21',attempt:1,tree:'a'.repeat(40),suites:['backend-school']})}])test(`invalid priming context ${JSON.stringify(extra)} fails before any Cargo work`,t=>{
 const f=fixture(t);const result=f.run(extra);assert.notEqual(result.status,0);
 assert.equal(spawnSync('test',['-e',f.env.COMMAND_LOG]).status,1);
});
