import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,cpSync,rmSync,statSync,existsSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
const repo=path.resolve(import.meta.dirname,'../..');
function fixture(t,{fresh=false,publicFailure=false,smokeFailure=false,reloadFailure=false,reloadDelay=false}={}) {
 const stack=mkdtempSync(path.join(os.tmpdir(),'pipeline-maintenance-'));
 t.after(()=>rmSync(stack,{recursive:true,force:true}));
 const root=path.join(stack,'deployment');
 mkdirSync(path.join(stack,'nginx/conf.d'),{recursive:true});mkdirSync(path.join(stack,'bin'));
 mkdirSync(path.join(root,'scripts'),{recursive:true});
 cpSync(path.join(repo,'nginx-configs'),path.join(root,'nginx-configs'),{recursive:true});
 cpSync(path.join(repo,'scripts/render_nginx_config.sh'),path.join(root,'scripts/render_nginx_config.sh'));
 writeFileSync(path.join(root,'podman-compose.yml'),'services: {}');writeFileSync(path.join(stack,'.env'),'');
 writeFileSync(path.join(root,'scripts/smoke_test.sh'),`#!/bin/bash\n[[ $SMOKE_RELEASE_PROBE_TOKEN =~ ^[0-9a-f]{64}$ ]] || exit 64\nexit ${smokeFailure?9:0}\n`);
 writeFileSync(path.join(stack,'bin/podman'),`#!/bin/bash
printf '%s\\n' "$*" >> "$SCHOOLORBIT_STACK_ROOT/commands"
if [[ $1 == container && $2 == exists ]]; then exit ${fresh?1:0}; fi
if [[ $* == *'nginx -s reload'* && -f $SCHOOLORBIT_STACK_ROOT/fail-reload ]]; then rm "$SCHOOLORBIT_STACK_ROOT/fail-reload"; exit 8; fi
exit 0
`,{mode:0o755});
 writeFileSync(path.join(stack,'bin/podman-compose'),'#!/bin/bash\nprintf "compose %s\\n" "$*" >> "$SCHOOLORBIT_STACK_ROOT/commands"\n',{mode:0o755});
 writeFileSync(path.join(stack,'bin/curl'),`#!/bin/bash
file=''; probe=false
while (($#)); do
 case "$1" in -o) file=$2; shift 2;; -H) probe=true; shift 2;; *) shift;; esac
done
if [[ $probe == true ]]; then printf '{"status":"ready"}'; exit 0; fi
if [[ -n $file ]]; then
 if [[ -f $SCHOOLORBIT_STACK_ROOT/delayed-reload ]]; then
  rm "$SCHOOLORBIT_STACK_ROOT/delayed-reload"
  if grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf"; then
   printf '{"status":"ready"}' > "$file"; printf 200
  else
   printf '{"error":"maintenance"}' > "$file"; printf 503
  fi
  exit 0
 fi
 if grep -q '"maintenance"' "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf"; then
  printf '{"error":"maintenance"}' > "$file"; printf 503
 else
  printf '{"status":"ready"}' > "$file"; printf ${publicFailure?500:200}
 fi
fi
`,{mode:0o755});
 const env={...process.env,PATH:path.join(stack,'bin')+':'+process.env.PATH,SCHOOLORBIT_STACK_ROOT:stack,BASE_DOMAIN:'example.test',RELEASE_SHA:'a'.repeat(40)};
 const run=mode=>{
  if(reloadDelay && ['enter','accept'].includes(mode))writeFileSync(path.join(stack,'delayed-reload'),'');
  return spawnSync('bash',[path.join(repo,'scripts/lib/pipeline-remote/maintenance.sh'),mode],{env,encoding:'utf8'});
 };
 return {stack,root,run,reloadFailure};
}
test('fresh origin starts only a maintenance proxy without unresolved backend upstreams',t=>{
 const f=fixture(t,{fresh:true});const result=f.run('enter');assert.equal(result.status,0,result.stderr);
 for(const part of ['school','admin']) {
  const source=readFileSync(path.join(f.stack,'nginx/conf.d',part+'-api.conf'),'utf8');
  assert.match(source,/return 503/);assert.doesNotMatch(source,/proxy_pass|return 418/);
 }
 assert.match(readFileSync(path.join(f.stack,'commands'),'utf8'),/compose .*up -d --no-deps nginx/);
 assert.equal(statSync(path.join(f.root,'releases','a'.repeat(40),'probe-token')).mode&0o777,0o600);
});
test('private verification preserves maintenance and acceptance opens both proxies last',t=>{
 const f=fixture(t);assert.equal(f.run('enter').status,0);const first=readFileSync(path.join(f.root,'releases','a'.repeat(40),'probe-token'),'utf8');
 assert.equal(f.run('enter').status,0);assert.equal(readFileSync(path.join(f.root,'releases','a'.repeat(40),'probe-token'),'utf8'),first);
 assert.equal(f.run('verify').status,0);assert.ok(existsSync(path.join(f.root,'pending-release')));
 assert.equal(f.run('accept').status,0);assert.ok(!existsSync(path.join(f.root,'pending-release')));
 for(const part of ['school','admin']) assert.doesNotMatch(readFileSync(path.join(f.stack,'nginx/conf.d',part+'-api.conf'),'utf8'),/return 503/);
});
for(const failure of ['smokeFailure','publicFailure','reloadFailure','renderFailure']) test(`${failure} refuses acceptance and leaves both public APIs in maintenance`,t=>{
 const f=fixture(t,{[failure]:true});assert.equal(f.run('enter').status,0);
 if(failure==='renderFailure')rmSync(path.join(f.root,'nginx-configs/admin-api.conf.template'));
 if(failure==='reloadFailure')writeFileSync(path.join(f.stack,'fail-reload'),'');
 assert.notEqual(f.run('accept').status,0);
 assert.ok(existsSync(path.join(f.root,'pending-release')));assert.ok(!existsSync(path.join(f.root,'accepted-release')));
 for(const part of ['school','admin']) assert.match(readFileSync(path.join(f.stack,'nginx/conf.d',part+'-api.conf'),'utf8'),/return 503/);
});

test('entering and opening maintenance wait for the asynchronous Nginx reload to take effect',t=>{
 const f=fixture(t,{reloadDelay:true});
 for(const mode of ['enter','verify','accept']) {
  const result=f.run(mode);assert.equal(result.status,0,result.stderr);
 }
 assert.ok(existsSync(path.join(f.root,'accepted-release')));
 assert.ok(!existsSync(path.join(f.root,'pending-release')));
});
