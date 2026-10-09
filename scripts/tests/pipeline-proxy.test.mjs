import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import test from 'node:test';

test('real Nginx serves public maintenance, CORS preflight and token probes without bypassing backend authorization', async (t) => {
 const temp=mkdtempSync(path.join(os.tmpdir(),'pipeline-proxy-'));
 const suffix=`${process.pid}-${Math.random().toString(16).slice(2,10)}`;
 const network=`pipeline-proxy-${suffix}`,backend=`pipeline-upstream-${suffix}`,proxy=`pipeline-proxy-${suffix}`;
 const docker=(...args)=>execFileSync('docker',args,{encoding:'utf8',stdio:['ignore','pipe','pipe']}).trim();
 t.after(()=>{
  for(const name of [proxy,backend])try{docker('rm','-f','-v',name)}catch{}
  try{docker('network','rm',network)}catch{}
  rmSync(temp,{recursive:true,force:true});
 });
 const image='public.ecr.aws/docker/library/nginx:stable-alpine@sha256:0985e772fb9f729e6fa0980da05fca5d9c468e870eed43071545afa9d2e27d94';
 docker('network','create',network);
 docker('create','--name',backend,'--network',network,'--network-alias','schoolorbit-backend-school','--network-alias','schoolorbit-backend-admin',image);
 const upstream=path.join(temp,'upstream.conf');
 writeFileSync(upstream,'server { listen 8080; listen 8081; location = /ready { default_type application/json; return 200 \'{"status":"ready"}\'; } location / { if ($request_method = OPTIONS) { return 405; } default_type application/json; return 401 \'{"error":"unauthorized"}\'; } }');
 docker('cp',upstream,`${backend}:/etc/nginx/conf.d/default.conf`);docker('start',backend);
 const conf=path.join(temp,'conf'),ssl=path.join(temp,'ssl');mkdirSync(conf);mkdirSync(ssl);
 execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',path.join(ssl,'schoolorbit-origin.key'),'-out',path.join(ssl,'schoolorbit-origin.pem'),'-days','1','-subj','/CN=example.test','-addext','subjectAltName=DNS:school-api.example.test,DNS:admin-api.example.test'],{stdio:'ignore'});
 const probe='e'.repeat(64);
 for(const part of ['school','admin'])execFileSync('bash',['scripts/render_nginx_config.sh',`nginx-configs/${part}-api.maintenance.conf.template`,path.join(conf,`${part}.conf`),'example.test','a'.repeat(40),'maintenance',probe]);
 docker('run','-d','--name',proxy,'--network',network,'-p','127.0.0.1::443','--entrypoint','sh',image,'-c','while [ ! -f /etc/nginx/ssl/schoolorbit-origin.pem ]; do sleep 0.1; done; exec nginx -g "daemon off;"');
 docker('cp',conf+'/.',`${proxy}:/etc/nginx/conf.d/`);docker('cp',ssl,`${proxy}:/etc/nginx/ssl`);
 let binding; for(let attempt=0;;attempt++){try{binding=docker('port',proxy,'443/tcp');break}catch{if(attempt===50)throw new Error(docker('inspect','--format','{{json .State}}',proxy));await delay(100)}}assert.match(binding,/^127\.0\.0\.1:[0-9]+$/);const port=binding.split(':')[1];
 const request=(part,url,extra=[])=>execFileSync('curl',['--noproxy','*','--silent','--show-error','--max-time','5','--cacert',path.join(ssl,'schoolorbit-origin.pem'),'--resolve',`${part}-api.example.test:${port}:127.0.0.1`,'-w','\n%{http_code}',...extra,`https://${part}-api.example.test:${port}${url}`],{encoding:'utf8'});
 for(let attempt=0;;attempt++){try{request('school','/deployment-status');break}catch(error){if(attempt===30){const result=spawnSync('docker',['logs',proxy],{encoding:'utf8'});throw new Error(result.stdout+result.stderr)};await delay(100)}}
 for(const part of ['school','admin']) {
  assert.match(request(part,'/deployment-status'),/"status":"maintenance"[\s\S]*\n200$/);
  assert.match(request(part,'/ready'),/"error":"maintenance"[\s\S]*\n503$/);
  assert.match(request(part,'/ready',['-H','X-Schoolorbit-Release-Probe: wrong']),/\n503$/);
  assert.match(request(part,'/ready',['-H',`X-Schoolorbit-Release-Probe: ${probe}`]),/"status":"ready"[\s\S]*\n200$/);
  assert.match(request(part,'/api/private',['-H',`X-Schoolorbit-Release-Probe: ${probe}`]),/"error":"unauthorized"[\s\S]*\n401$/);
  for (const headers of [[], ['-H','X-Schoolorbit-Release-Probe: wrong'], ['-H',`X-Schoolorbit-Release-Probe: ${probe}`]]) {
   const response=request(part,'/api/private',['-X','OPTIONS','-H','Origin: https://sandbox.example.test','-H','Access-Control-Request-Method: POST','-H','Access-Control-Request-Headers: content-type,x-school-subdomain,x-csrf-token','-i',...headers]);
   assert.match(response,/Access-Control-Allow-Origin: https:\/\/sandbox.example.test[\s\S]*\n204$/i);
   assert.match(response,/Access-Control-Allow-Headers: [^\r\n]*X-School-Subdomain[^\r\n]*X-CSRF-Token/i);
  }
 }
 assert.match(request('admin','/internal/schools'),/"error":"unauthorized"[\s\S]*\n401$/);
});
