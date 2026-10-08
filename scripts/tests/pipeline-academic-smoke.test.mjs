import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdirSync,mkdtempSync,rmSync,writeFileSync} from 'node:fs';
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
