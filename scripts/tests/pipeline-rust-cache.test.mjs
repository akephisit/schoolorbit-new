import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,utimesSync,statSync,rmSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
const script=path.resolve(import.meta.dirname,'../rust_ci_cache.mjs');
function fixture(t) {
 const root=mkdtempSync(path.join(os.tmpdir(),'pipeline-rust-cache-'));t.after(()=>rmSync(root,{recursive:true,force:true}));
 const component='backend-school';mkdirSync(path.join(root,component,'target'),{recursive:true});mkdirSync(path.join(root,'bin'));
 spawnSync('git',['init','-q'],{cwd:root});
 for(const [file,source]of Object.entries({'Cargo.lock':'# fixture','stable.rs':'pub fn stable() {}','changed.rs':'pub fn old() {}'}))writeFileSync(path.join(root,component,file),source);
 for(const [file,source]of Object.entries({'contracts/permissions.lock.json':'original permission lock','frontend-school/static/fonts/Sarabun-Regular.ttf':'font input','backend-school/migrations/001.sql':'SELECT 1;'})) {
  mkdirSync(path.dirname(path.join(root,file)),{recursive:true});writeFileSync(path.join(root,file),source);
 }
 spawnSync('git',['add',component,'contracts','frontend-school'],{cwd:root});
 writeFileSync(path.join(root,'bin/rustc'),'#!/bin/sh\nprintf "rustc 1.98.1 fixture\\n"\n',{mode:0o755});
 const env={...process.env,PATH:path.join(root,'bin')+':'+process.env.PATH,RUST_COMPONENT:component,RUNNER_OS:'Linux',RUNNER_ARCH:'X64',CARGO_PROFILE_DEV_DEBUG:'0',CARGO_PROFILE_TEST_DEBUG:'0',CARGO_INCREMENTAL:'0',GITHUB_OUTPUT:path.join(root,'outputs')};
 const run=(mode,extra={})=>spawnSync('node',[script,mode],{cwd:root,env:{...env,...extra},encoding:'utf8'});
 return {root,component,run,env};
}
test('source-keyed cache preserves unchanged Cargo inputs and forces edited backdated sources dirty',t=>{
 const f=fixture(t);assert.equal(f.run('key').status,0);const initial=readFileSync(f.env.GITHUB_OUTPUT,'utf8');assert.equal(f.run('save').status,0);
 const stable=path.join(f.root,f.component,'stable.rs'),changed=path.join(f.root,f.component,'changed.rs');
 utimesSync(stable,Date.now()/1000,Date.now()/1000);writeFileSync(changed,'pub fn edited() {}');utimesSync(changed,1,1);
 assert.equal(f.run('restore').status,0);assert.equal(statSync(stable).mtimeMs,1000);assert.ok(statSync(changed).mtimeMs>Date.now());
 writeFileSync(f.env.GITHUB_OUTPUT,'');assert.equal(f.run('key').status,0);const updated=readFileSync(f.env.GITHUB_OUTPUT,'utf8');
 assert.equal(initial.split('\n')[1],updated.split('\n')[1]);assert.notEqual(initial.split('\n')[0],updated.split('\n')[0]);
});
test('cross-component include inputs participate in cache identity and backdated-change invalidation',t=>{
 const f=fixture(t);assert.equal(f.run('key').status,0);const old=readFileSync(f.env.GITHUB_OUTPUT,'utf8');assert.equal(f.run('save').status,0);
 const contract=path.join(f.root,'contracts/permissions.lock.json'),font=path.join(f.root,'frontend-school/static/fonts/Sarabun-Regular.ttf');
 writeFileSync(contract,'changed permission lock');utimesSync(contract,1,1);utimesSync(font,Date.now()/1000,Date.now()/1000);
 writeFileSync(f.env.GITHUB_OUTPUT,'');assert.equal(f.run('key').status,0);assert.notEqual(old.split('\n')[0],readFileSync(f.env.GITHUB_OUTPUT,'utf8').split('\n')[0]);
 assert.equal(f.run('restore').status,0);assert.ok(statSync(contract).mtimeMs>Date.now());assert.equal(statSync(font).mtimeMs,1000);
});
test('unchanged watched migration directory and its sources remain older than cached outputs',t=>{
 const f=fixture(t);assert.equal(f.run('save').status,0);const directory=path.join(f.root,f.component,'migrations');
 utimesSync(directory,Date.now()/1000,Date.now()/1000);assert.equal(f.run('restore').status,0);assert.equal(statSync(directory).mtimeMs,1000);
});
for(const change of ['edit','add','delete','untracked','ignored','empty-directory']) test(`migration ${change} keeps the watched directory dirty`,t=>{
 const f=fixture(t);assert.equal(f.run('save').status,0);const directory=path.join(f.root,f.component,'migrations');
 if(change==='edit'){writeFileSync(path.join(directory,'001.sql'),'SELECT 2;');utimesSync(path.join(directory,'001.sql'),1,1);}
 if(change==='delete') {
  assert.equal(spawnSync('git',['rm','--cached','-f',f.component+'/migrations/001.sql'],{cwd:f.root}).status,0);
  rmSync(path.join(directory,'001.sql'));
 }
 if(['add','untracked','ignored'].includes(change)) {
  writeFileSync(path.join(directory,'002.sql'),'SELECT 2;');utimesSync(path.join(directory,'002.sql'),1,1);
  if(change==='add')assert.equal(spawnSync('git',['add',f.component+'/migrations/002.sql'],{cwd:f.root}).status,0);
  if(change==='ignored')writeFileSync(path.join(f.root,'.gitignore'),'**/002.sql\n');
 }
 if(change==='empty-directory')mkdirSync(path.join(directory,'new-directory'));
 utimesSync(directory,1,1);assert.equal(f.run('restore').status,0);assert.ok(statSync(directory).mtimeMs>Date.now());
});
test('compiler profile or lock changes cannot reuse the previous compiled source evidence',t=>{
 const f=fixture(t);assert.equal(f.run('save').status,0);
 assert.notEqual(f.run('restore',{CARGO_PROFILE_DEV_DEBUG:'2'}).status,0);
 assert.equal(f.run('key').status,0);const old=readFileSync(f.env.GITHUB_OUTPUT,'utf8');writeFileSync(path.join(f.root,f.component,'Cargo.lock'),'# new lock');writeFileSync(f.env.GITHUB_OUTPUT,'');assert.equal(f.run('key').status,0);
 assert.notEqual(old.split('\n')[1],readFileSync(f.env.GITHUB_OUTPUT,'utf8').split('\n')[1]);
 assert.notEqual(f.run('restore').status,0);
});
