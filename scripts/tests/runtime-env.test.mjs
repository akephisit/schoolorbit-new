import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,writeFileSync,chmodSync,symlinkSync,rmSync,readFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';

const repo=path.resolve(import.meta.dirname,'../..');
test('runtime decoder round-trips installer encoding without evaluating or leaking secret expressions',t=>{
 const dir=mkdtempSync(path.join(os.tmpdir(),'runtime-env-'));t.after(()=>rmSync(dir,{recursive:true,force:true}));
 const file=path.join(dir,'runtime.env');
 const value="postgresql://fixture:synthetic@localhost/db?sslmode=verify-full&test='quoted'\\path$(touch NEVER_CREATE)";
 const encoded="'"+value.replaceAll('\\','\\\\').replaceAll("'","\\'")+"'";
 const run=()=>spawnSync('bash',['-euc','. "$1"; schoolorbit_runtime_env_value ADMIN_MIGRATION_DATABASE_URL "$2"','fixture',path.join(repo,'scripts/lib/pipeline-remote/runtime-env.sh'),file],{cwd:dir,encoding:'utf8'});
 writeFileSync(file,'ADMIN_MIGRATION_DATABASE_URL='+encoded+'\n',{mode:0o600});
 let result=run();assert.equal(result.status,0,result.stderr);assert.equal(result.stdout,value);assert.equal(result.stderr,'');
 assert.equal(spawnSync('test',['!','-e',path.join(dir,'NEVER_CREATE')]).status,0);
 for(const malformed of [encoded+'\nADMIN_MIGRATION_DATABASE_URL='+encoded,"'invalid\\q'",'unquoted',"'incomplete\\'"]){
  writeFileSync(file,'ADMIN_MIGRATION_DATABASE_URL='+malformed+'\n');result=run();assert.notEqual(result.status,0);assert.equal(result.stdout,'');assert.ok(!result.stderr.includes(value));
 }
 writeFileSync(file,'ADMIN_MIGRATION_DATABASE_URL='+encoded+'\n');chmodSync(file,0o644);assert.notEqual(run().status,0);
 rmSync(file);const target=path.join(dir,'target.env');writeFileSync(target,'ADMIN_MIGRATION_DATABASE_URL='+encoded+'\n',{mode:0o600});symlinkSync(target,file);assert.notEqual(run().status,0);
});
test('Admin one-shot migration passes only a decoded credential by environment name',()=>{
 const source=readFileSync(path.join(repo,'scripts/lib/pipeline-remote/admin-deploy.sh'),'utf8');
 assert.match(source,/schoolorbit_runtime_env_value ADMIN_MIGRATION_DATABASE_URL \/opt\/stack\/\.env/);
 assert.match(source,/export ADMIN_MIGRATION_DATABASE_URL/);
 assert.match(source,/--env ADMIN_MIGRATION_DATABASE_URL/);
 assert.doesNotMatch(source,/--env-file \/opt\/stack\/\.env/);
 assert.doesNotMatch(source,/--env ADMIN_MIGRATION_DATABASE_URL=/);
});
