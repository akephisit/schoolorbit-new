import assert from 'node:assert/strict';
import test from 'node:test';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,rmSync,existsSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { validateCutoverGate } from '../postgres_cutover_gate.mjs';

const sha = 'a'.repeat(40);
const valid = () => ({ repo: 'owner/repo', sha, ref: 'refs/heads/main', runtimeEnabled: 'false', frontendEnabled: 'false',
  main: { object: { sha } }, runs: { workflow_runs: [{ path: '.github/workflows/pipeline.yml', head_sha: sha,
    head_branch: 'main', head_repository: { full_name: 'owner/repo' }, event: 'push', run_attempt: 2 }] },
  jobs: [{ name: 'Pipeline gate', conclusion: 'success', head_sha: sha, run_attempt: 2 }] });

test('database cutover requires trusted current main, paused releases and the latest-attempt build gate', () => {
  assert.doesNotThrow(() => validateCutoverGate(valid()));
  for (const change of [
    (f) => { f.ref = 'refs/heads/feature'; },
    (f) => { f.main.object.sha = 'b'.repeat(40); },
    (f) => { f.runtimeEnabled = 'true'; },
    (f) => { f.frontendEnabled = 'true'; },
    (f) => { f.jobs[0].run_attempt = 1; },
    (f) => { f.jobs[0].conclusion = 'failure'; },
    (f) => { f.runs.workflow_runs[0].head_repository.full_name = 'fork/repo'; },
    (f) => { f.runs.workflow_runs[0].path = '.github/workflows/operations.yml'; },
  ]) {
    const fixture = valid(); change(fixture);
    assert.throws(() => validateCutoverGate(fixture));
  }
});

for (const present of [false,true]) test(`cutover retry preserves maintenance and handles source containers present=${present}`, t => {
  const stack=mkdtempSync(path.join(os.tmpdir(),'postgres-cutover-'));
  t.after(()=>rmSync(stack,{recursive:true,force:true}));
  const release=path.join(stack,'deployment/releases',sha);
  mkdirSync(release,{recursive:true});mkdirSync(path.join(stack,'deployment/scripts/lib/pipeline-remote'),{recursive:true});mkdirSync(path.join(stack,'bin'));
  writeFileSync(path.join(stack,'deployment/scripts/lib/pipeline-remote/maintenance.sh'),'#!/bin/bash\n[[ $1 == enter ]] || exit 64\nprintf entered > "$SCHOOLORBIT_STACK_ROOT/maintenance"\n');
  writeFileSync(path.join(stack,'bin/podman'),`#!/bin/bash
if [[ $1 == container && $2 == exists ]]; then exit ${present?0:1}; fi
if [[ $1 == stop ]]; then printf '%s\\n' "$2" >> "$SCHOOLORBIT_STACK_ROOT/stopped"; exit 0; fi
exit 64
`,{mode:0o755});
  writeFileSync(path.join(release,'database-cutover-ready.json'),JSON.stringify({schemaVersion:1,sha,databases:3,dataReconciled:true,runtimeTlsVerified:true,backupRestorePassed:true}),{mode:0o600});
  const env={...process.env,PATH:path.join(stack,'bin')+':'+process.env.PATH,SCHOOLORBIT_STACK_ROOT:stack,BASE_DOMAIN:'example.test',RELEASE_SHA:sha,EXPECTED_DATABASES:'3'};
  const script=path.resolve(import.meta.dirname,'../lib/pipeline-remote/postgres-cutover.sh');
  const result=spawnSync('bash',[script],{env,encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);assert.equal(readFileSync(path.join(stack,'maintenance'),'utf8'),'entered');
  if(present)assert.equal(readFileSync(path.join(stack,'stopped'),'utf8'),'schoolorbit-backend-school\nschoolorbit-backend-admin\n');
  else assert.equal(existsSync(path.join(stack,'stopped')),false);
  writeFileSync(path.join(release,'database-cutover-ready.json'),JSON.stringify({schemaVersion:1,sha,databases:3,dataReconciled:true,runtimeTlsVerified:true,backupRestorePassed:false}));
  assert.equal(spawnSync('bash',[script],{env,encoding:'utf8'}).status,78);
});
