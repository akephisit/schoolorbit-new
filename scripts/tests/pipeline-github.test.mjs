import assert from 'node:assert/strict';
import {execFileSync,spawnSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
import {components,git,makePlan} from '../lib/pipeline-policy.mjs';
const repo='team/school', root=path.resolve(import.meta.dirname,'../..'), sha='d'.repeat(40), head='b'.repeat(40), base='a'.repeat(40), tree='c'.repeat(40);
function fixture(t) {
 const dir=mkdtempSync(path.join(os.tmpdir(),'pipeline-github-'));t.after(()=>rmSync(dir,{recursive:true,force:true}));
 mkdirSync(path.join(dir,'bin'));const routes={},sequences={};
 const zip=(id,name,body)=>{
  const file=path.join(dir,`${id}.zip`);
  execFileSync('python3',['-c','import sys,zipfile; z=zipfile.ZipFile(sys.argv[1],"w"); z.writestr(sys.argv[2],sys.stdin.read()); z.close()',file,name],{input:JSON.stringify(body)});
  routes[`/repos/${repo}/actions/artifacts/${id}/zip`]={binary:file};
 };
 writeFileSync(path.join(dir,'bin/gh'),`#!/usr/bin/env node
const fs=require('node:fs');
const cfg=JSON.parse(fs.readFileSync(process.env.FAKE_GH_ROUTES,'utf8'));
const args=process.argv.slice(2); const route=args.find(x=>x.startsWith('/repos/'));
let input=''; if(args.includes('--input'))input=fs.readFileSync(0,'utf8');
fs.appendFileSync(process.env.FAKE_GH_LOG,JSON.stringify({route,args,input})+'\\n');
let response=cfg.routes[route];
if(cfg.sequences[route]) {
 const file=process.env.FAKE_GH_LOG+'.counts';let counts={};try{counts=JSON.parse(fs.readFileSync(file,'utf8'))}catch{}
 const count=counts[route]||0;const list=cfg.sequences[route];response=list[Math.min(count,list.length-1)];counts[route]=count+1;fs.writeFileSync(file,JSON.stringify(counts));
}
if(response===undefined){console.error('Unexpected API '+route);process.exit(90)}
if(response.binary)process.stdout.write(fs.readFileSync(response.binary));else process.stdout.write(JSON.stringify(response));
`,{mode:0o755});
 writeFileSync(path.join(dir,'bin/docker'),'#!/bin/sh\nprintf 28.5.0\n',{mode:0o755});
 const env={...process.env,PATH:path.join(dir,'bin')+':'+process.env.PATH,FAKE_GH_ROUTES:path.join(dir,'routes.json'),FAKE_GH_LOG:path.join(dir,'calls.jsonl'),GITHUB_REPOSITORY:repo,GITHUB_REF:'refs/heads/main',GITHUB_SHA:sha,GITHUB_RUN_ID:'42',GITHUB_RUN_ATTEMPT:'1',GITHUB_EVENT_NAME:'push',GITHUB_OUTPUT:path.join(dir,'outputs'),GITHUB_STEP_SUMMARY:path.join(dir,'summary'),RUNNER_OS:'Linux',ImageVersion:'20261001.1.0',REQUESTED_SUITES:'["frontend-school"]',CANDIDATE:'false'};
 const run=(file,extra={},cwd=root)=>{writeFileSync(env.FAKE_GH_ROUTES,JSON.stringify({routes,sequences}));return spawnSync('node',[path.join(root,'scripts',file)],{cwd,env:{...env,...extra},encoding:'utf8'});};
 return {dir,routes,sequences,zip,run,env,calls:()=>readFileSync(env.FAKE_GH_LOG,'utf8').trim().split('\n').map(JSON.parse)};
}
function runIdentity(id=21){return {id,repository:{full_name:repo},head_repository:{full_name:repo},head_sha:head,head_branch:'feature',path:'.github/workflows/pipeline.yml',event:'pull_request',status:'completed',conclusion:'success',run_attempt:1};}
function candidatePlan(f) {
 const pr={number:7,user:{login:'writer'},head:{sha:head,repo:{full_name:repo}},base:{ref:'main'},merged_at:'2026-10-08',merge_commit_sha:sha,draft:false,mergeable_state:'clean'};
 const plan={schemaVersion:1,sha,base,tree,prHead:head,verify:['frontend-school'],runId:'21',attempt:1};
 const run=runIdentity();
 f.routes[`/repos/${repo}/git/commits/${sha}`]={tree:{sha:tree},parents:[{sha:base}]};
 f.routes[`/repos/${repo}/commits/${sha}/pulls?per_page=100`]=[pr];
 f.routes[`/repos/${repo}/collaborators/writer/permission`]={permission:'write'};
 f.routes[`/repos/${repo}/commits/${head}/check-runs?per_page=100`]={check_runs:[{id:99,name:'Pipeline gate',app:{slug:'github-actions'},conclusion:'success',details_url:`https://github.com/${repo}/actions/runs/21`}]};
 f.routes[`/repos/${repo}/actions/runs/21`]=run;
 f.routes[`/repos/${repo}/actions/runs/21/artifacts?per_page=100`]={artifacts:[{id:10,name:'pipeline-plan'}]};
 f.routes[`/repos/${repo}/actions/runs/21/attempts/1/jobs?per_page=100`]=[{jobs:[{name:'Pipeline gate',conclusion:'success',run_attempt:1,head_sha:head}]}];
 f.zip(10,'plan.json',plan);

 return {pr,plan,run};
}
function acceptedState(){return {schemaVersion:2,runId:'21',attempt:1,sha:head,components:Object.fromEntries(components.map(part=>[part,{sha:head,inputHash:'e'.repeat(64),digest:'sha256:'+'f'.repeat(64),bundleDigest:'e'.repeat(64),artifactRunId:21,versionId:'12345678-abcd-1234-abcd-123456789abc',workers:{sandbox:'12345678-abcd-1234-abcd-123456789abc'}}]))};}
for(const accepted of [true,false])test(`accepted state requires its acceptance attempt job success=${accepted}, even on a later rerun`,t=>{
 const f=fixture(t);const run={...runIdentity(),head_branch:'main',event:'push',status:'in_progress',conclusion:null,run_attempt:2};
 f.routes[`/repos/${repo}/actions/artifacts?name=pipeline-state&per_page=100`]={artifacts:[{id:12,workflow_run:{id:21}}]};
 f.routes[`/repos/${repo}/actions/runs/21`]=run;
 f.routes[`/repos/${repo}/actions/runs/21/attempts/1/jobs?per_page=100`]=[{jobs:[{name:'release / accept',head_sha:head,conclusion:accepted?'success':'failure'}]}];f.zip(12,'pipeline-state.json',acceptedState());
 const result=f.run('lib/pipeline-state.mjs');assert.equal(result.status,0,result.stderr);assert.equal(JSON.parse(result.stdout)?.runId,accepted?'21':undefined);
});
for(const stale of [false,true])test(`merge controller serializes two ready PRs and blocks stale base=${stale}`,t=>{
 const f=fixture(t);const p=candidatePlan(f);f.routes[`/repos/${repo}/pulls?state=open&base=main&sort=created&direction=asc&per_page=100`]=[p.pr,{...p.pr,number:8}];
 for(const number of [7,8])f.routes[`/repos/${repo}/pulls/${number}`]=p.pr;
 f.routes[`/repos/${repo}/git/ref/heads/main`]={object:{sha:stale?head:base}};
 f.routes[`/repos/${repo}/pulls/7/merge`]={merged:true};f.routes[`/repos/${repo}/actions/workflows/pipeline.yml/dispatches`]={};
 const result=f.run('team_merge.mjs');assert.equal(result.status,0,result.stderr);
 const merges=f.calls().filter(x=>x.route.endsWith('/merge'));assert.equal(merges.length,stale?0:1);
 const inputs=f.calls().filter(x=>x.input).map(x=>JSON.parse(x.input));
 if(stale) assert.equal(inputs.length,0);
 else assert.ok(inputs.some(x=>x.inputs?.automatic==='true'));
 assert.ok(f.calls().every(x=>!x.route.endsWith('/update-branch')));
});
for(const mode of ['valid','stale-main','failed-gate','wrong-attempt','rollout-disabled'])test(`release preflight ${mode} validates before any production mutation`,t=>{
 const f=fixture(t);const checkout=path.join(f.dir,'checkout');mkdirSync(checkout);execFileSync('git',['init','-q',checkout]);
 git(checkout,'config','user.name','Fixture');git(checkout,'config','user.email','fixture@example.invalid');
 for(const part of components){mkdirSync(path.join(checkout,part));writeFileSync(path.join(checkout,part,'input'),'test');}
 git(checkout,'add','.');git(checkout,'commit','-qm','Fixture');const commit=git(checkout,'rev-parse','HEAD');
 const plan=makePlan({root:checkout,head:commit,base:commit,publicConfig:{'frontend-school':{backend:'',vapid:''},'frontend-admin':{backend:'',schoolBackend:''}}});
 f.routes[`/repos/${repo}/git/ref/heads/main`]={object:{sha:mode==='stale-main'?sha:commit}};
 f.routes[`/repos/${repo}/actions/runs/42`]={...runIdentity(42),event:'push',head_branch:'main',head_sha:commit,run_attempt:mode==='wrong-attempt'?2:1};
 f.routes[`/repos/${repo}/actions/runs/42/attempts/1/jobs?per_page=100`]=[{jobs:[{name:'Pipeline gate',head_sha:commit,run_attempt:1,conclusion:mode==='failed-gate'?'failure':'success'}]}];
 f.routes[`/repos/${repo}/actions/artifacts?name=pipeline-state&per_page=100`]={artifacts:[]};
 const result=f.run('release_preflight.mjs',{PLAN:JSON.stringify(plan),GITHUB_SHA:commit,RUNTIME_DEPLOY_ENABLED:mode==='rollout-disabled'?'false':'true',FRONTEND_DEPLOY_ENABLED:'true',PUBLIC_BACKEND_URL:'',PUBLIC_VAPID_KEY:'',PUBLIC_API_URL:'',BACKEND_SCHOOL_URL:''},checkout);
 assert.equal(result.status,mode==='valid'?0:1,result.stderr);
});

test('a behind branch waits for its developer instead of mutating untested source',t=>{
 const f=fixture(t);const p=candidatePlan(f);p.pr.mergeable_state='behind';
 f.routes[`/repos/${repo}/pulls?state=open&base=main&sort=created&direction=asc&per_page=100`]=[p.pr];
 f.routes[`/repos/${repo}/pulls/7`]=p.pr;f.routes[`/repos/${repo}/git/ref/heads/main`]={object:{sha:base}};
 const result=f.run('team_merge.mjs');assert.equal(result.status,0,result.stderr);
 assert.ok(f.calls().every(x=>!x.args.includes('PUT')&&!x.args.includes('POST')));
});
