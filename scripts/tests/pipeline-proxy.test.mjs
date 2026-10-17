import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { Agent, request as httpsRequest } from 'node:https';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import test from 'node:test';

test('real Nginx preserves maintenance, authorization, socket reuse and privacy-safe School request timings', async (t) => {
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
 writeFileSync(upstream,`server {
  listen 8080; listen 8081;
  location = /ready { default_type application/json; return 200 '{"status":"ready"}'; }
  location = /api/proxy-fixture {
   default_type application/json;
   add_header Set-Cookie "fixture_session=response-$http_x_school_subdomain; Path=/; Secure; HttpOnly";
   add_header X-CSRF-Token "response-$http_x_csrf_token";
   return 200 '{"socket":"$connection","requestNumber":$connection_requests,"method":"$request_method","uri":"$request_uri","protocol":"$server_protocol","connectionHeader":"$http_connection","host":"$http_host","origin":"$http_origin","tenant":"$http_x_school_subdomain","cookie":"$http_cookie","csrf":"$http_x_csrf_token","authorization":"$http_authorization","realIp":"$http_x_real_ip","forwardedFor":"$http_x_forwarded_for","forwardedProto":"$http_x_forwarded_proto","diagnosticId":"$http_x_schoolorbit_diagnostic_id"}';
  }
  location / { if ($request_method = OPTIONS) { return 405; } add_header X-Fixture-Diagnostic-Id $http_x_schoolorbit_diagnostic_id always; default_type application/json; return 401 '{"error":"unauthorized"}'; }
 }`);
 docker('cp',upstream,`${backend}:/etc/nginx/conf.d/default.conf`);docker('start',backend);
 const conf=path.join(temp,'conf'),ssl=path.join(temp,'ssl');mkdirSync(conf);mkdirSync(ssl);
 execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',path.join(ssl,'schoolorbit-origin.key'),'-out',path.join(ssl,'schoolorbit-origin.pem'),'-days','1','-subj','/CN=example.test','-addext','subjectAltName=DNS:school-api.example.test,DNS:admin-api.example.test,DNS:admin.example.test'],{stdio:'ignore'});
 const probe='e'.repeat(64);
 for(const part of ['school','admin'])execFileSync('bash',['scripts/render_nginx_config.sh',`nginx-configs/${part}-api.maintenance.conf.template`,path.join(conf,`${part}.conf`),'example.test','a'.repeat(40),'maintenance',probe]);
 docker('run','-d','--name',proxy,'--network',network,'-p','127.0.0.1::443','--entrypoint','sh',image,'-c','while [ ! -f /etc/nginx/ssl/schoolorbit-origin.pem ]; do sleep 0.1; done; exec nginx -g "daemon off;"');
 docker('cp',conf+'/.',`${proxy}:/etc/nginx/conf.d/`);docker('cp',ssl,`${proxy}:/etc/nginx/ssl`);
 let binding; for(let attempt=0;;attempt++){try{binding=docker('port',proxy,'443/tcp');break}catch{if(attempt===50)throw new Error(docker('inspect','--format','{{json .State}}',proxy));await delay(100)}}assert.match(binding,/^127\.0\.0\.1:[0-9]+$/);const port=binding.split(':')[1];
 const requestHost=(hostname,url,extra=[])=>execFileSync('curl',['--noproxy','*','--silent','--show-error','--max-time','5','--cacert',path.join(ssl,'schoolorbit-origin.pem'),'--resolve',`${hostname}:${port}:127.0.0.1`,'-w','\n%{http_code}',...extra,`https://${hostname}:${port}${url}`],{encoding:'utf8'});
 const request=(part,url,extra=[])=>requestHost(`${part}-api.example.test`,url,extra);
 for(let attempt=0;;attempt++){try{request('school','/deployment-status');break}catch(error){if(attempt===30){const result=spawnSync('docker',['logs',proxy],{encoding:'utf8'});throw new Error(result.stdout+result.stderr)};await delay(100)}}
 for(const part of ['school','admin']) {
  assert.match(request(part,'/deployment-status'),/"status":"maintenance"[\s\S]*\n200$/);
  assert.match(request(part,'/ready'),/"error":"maintenance"[\s\S]*\n503$/);
  assert.match(request(part,'/ready',['-H','X-Schoolorbit-Release-Probe: wrong']),/\n503$/);
  assert.match(request(part,'/ready',['-H',`X-Schoolorbit-Release-Probe: ${probe}`]),/"status":"ready"[\s\S]*\n200$/);
  assert.match(request(part,'/api/private',['-H',`X-Schoolorbit-Release-Probe: ${probe}`]),/"error":"unauthorized"[\s\S]*\n401$/);
  for (const headers of [[], ['-H','X-Schoolorbit-Release-Probe: wrong'], ['-H',`X-Schoolorbit-Release-Probe: ${probe}`]]) {
   const origin=part==='school'?'https://sandbox.example.test':'https://example.test';
   const response=request(part,'/api/private',['-X','OPTIONS','-H',`Origin: ${origin}`,'-H','Access-Control-Request-Method: POST','-H','Access-Control-Request-Headers: content-type,x-school-subdomain,x-csrf-token','-i',...headers]);
   assert.ok(response.toLowerCase().includes(`access-control-allow-origin: ${origin}`));
   assert.match(response,/\n204$/);
   assert.match(response,/Access-Control-Allow-Headers: [^\r\n]*X-School-Subdomain[^\r\n]*X-CSRF-Token/i);
  }
 }
 assert.match(request('admin','/internal/schools'),/"error":"unauthorized"[\s\S]*\n401$/);
 const assertRetired=()=>{
  const response=requestHost('admin.example.test','/login',['-i']);
  assert.match(response,/\n410$/);
  assert.match(response,/Cache-Control: no-store/i);
  assert.doesNotMatch(response,/Location:|unauthorized/i);
  for(const origin of ['https://admin.example.test','https://sandbox.example.test']){
   const cors=request('admin','/api/private',['-X','OPTIONS','-H',`Origin: ${origin}`,'-i']);
   assert.match(cors,/\n204$/);
   assert.doesNotMatch(cors,/Access-Control-Allow-Origin:/i);
  }
 };
 assertRetired();
 execFileSync('bash',['scripts/render_nginx_config.sh','nginx-configs/admin-api.conf.template',path.join(conf,'admin.conf'),'example.test','a'.repeat(40),'ready']);
 docker('cp',path.join(conf,'admin.conf'),`${proxy}:/etc/nginx/conf.d/admin.conf`);
 docker('exec',proxy,'nginx','-t');docker('exec',proxy,'nginx','-s','reload');
 for(let attempt=0;;attempt++){
  if(request('admin','/deployment-status').includes('"status":"ready"'))break;
  if(attempt===30)throw new Error('Admin ready configuration did not activate');
  await delay(100);
 }
 assertRetired();
 assert.match(request('admin','/api/private'),/"error":"unauthorized"[\s\S]*\n401$/);
 assert.match(request('admin','/api/private',['-X','OPTIONS','-H','Origin: https://example.test','-i']),/Access-Control-Allow-Origin: https:\/\/example.test[\s\S]*\n204$/i);
 assert.match(request('school','/ready'),/"error":"maintenance"[\s\S]*\n503$/);

 execFileSync('bash',['scripts/render_nginx_config.sh','nginx-configs/school-api.conf.template',path.join(conf,'school.conf'),'example.test','a'.repeat(40),'ready']);
 docker('cp',path.join(conf,'school.conf'),`${proxy}:/etc/nginx/conf.d/school.conf`);
 docker('exec',proxy,'nginx','-t');docker('exec',proxy,'nginx','-s','reload');
 for(let attempt=0;;attempt++){
  if(request('school','/deployment-status').includes('"status":"ready"'))break;
  if(attempt===30)throw new Error('School ready configuration did not activate');
  await delay(100);
 }
 // One verified TLS connection pins these sequential requests to one proxy worker.
 // The upstream's connection ID and request counter prove backend socket reuse.
 const agent=new Agent({keepAlive:true,maxSockets:1,ca:readFileSync(path.join(ssl,'schoolorbit-origin.pem'))});
 t.after(()=>agent.destroy());
 const ordinary=(url,headers={})=>new Promise((resolve,reject)=>{
  const outgoing=httpsRequest({host:'127.0.0.1',port,servername:'school-api.example.test',path:url,agent,headers:{Host:'school-api.example.test',...headers}},response=>{
   let body='';response.setEncoding('utf8');response.on('data',chunk=>body+=chunk);
   response.on('end',()=>resolve({status:response.statusCode,headers:response.headers,body:JSON.parse(body)}));
   response.on('error',reject);
  });
  outgoing.setTimeout(5000,()=>outgoing.destroy(new Error('School proxy fixture timed out')));
  outgoing.on('error',reject);outgoing.end();
 });
 const replies=[];
 for(const [index,tenant] of ['alpha','beta',''].entries()){
  const origin=tenant?`https://${tenant}.example.test`:'https://untrusted.invalid';
  const credentials=tenant?{
   Cookie:`fixture_session=${tenant}`,
   'X-School-Subdomain':tenant,
   'X-CSRF-Token':`csrf-${tenant}`,
   Authorization:`Bearer fixture-${tenant}`,
  }:{};
  const response=await ordinary(`/api/proxy-fixture?request=${index}`,{Origin:origin,...credentials});
  replies.push(response.body);
  assert.equal(response.status,200);
  assert.equal(response.body.uri,`/api/proxy-fixture?request=${index}`);
  assert.equal(response.body.method,'GET');
  assert.equal(response.body.protocol,'HTTP/1.1');
  assert.equal(response.body.connectionHeader,'');
  assert.equal(response.body.host,'school-api.example.test');
  assert.equal(response.body.origin,origin);
  assert.equal(response.body.tenant,tenant);
  assert.equal(response.body.cookie,tenant?`fixture_session=${tenant}`:'');
  assert.equal(response.body.csrf,tenant?`csrf-${tenant}`:'');
  assert.equal(response.body.authorization,tenant?`Bearer fixture-${tenant}`:'');
  assert.ok(response.body.realIp);
  assert.equal(response.body.forwardedFor,response.body.realIp);
  assert.equal(response.body.forwardedProto,'https');
  assert.equal(response.headers['access-control-allow-origin'],tenant?origin:undefined);
  assert.equal(response.headers['access-control-allow-credentials'],'true');
  assert.match(response.headers['access-control-expose-headers'],/X-CSRF-Token/i);
  assert.deepEqual(response.headers['set-cookie'],[`fixture_session=response-${tenant}; Path=/; Secure; HttpOnly`]);
  assert.equal(response.headers['x-csrf-token'],tenant?`response-csrf-${tenant}`:'response-');
 }
 assert.equal(new Set(replies.map(reply=>reply.socket)).size,1,'ordinary requests must reuse the backend socket');
 assert.deepEqual(replies.map(reply=>reply.requestNumber),[1,2,3]);
 const denied=await ordinary('/api/private');
 assert.equal(denied.status,401);assert.deepEqual(denied.body,{error:'unauthorized'});
 assert.match(request('school','/api/private',['-X','OPTIONS','-H','Origin: https://alpha.example.test','-i']),/Access-Control-Allow-Origin: https:\/\/alpha.example.test[\s\S]*\n204$/i);
 const rendered=readFileSync(path.join(conf,'school.conf'),'utf8');
 assert.equal((rendered.match(/proxy_set_header X-SchoolOrbit-Diagnostic-Id \$school_diagnostic_id;/g)||[]).length,(rendered.match(/proxy_pass /g)||[]).length);
 const timingFormat=rendered.match(/log_format school_request_timing[\s\S]*?;/)[0];
 assert.match(timingFormat,/escape=json/);
 assert.doesNotMatch(timingFormat,/\$(?:uri|request_uri|args|query_string|remote_addr|http_|request_body|request_method)/);
 assert.match(request('school','/private-path-secret?private-query-secret',['-X','PRIVATESECRET','-H','Cookie: private-cookie-secret']),/\n401$/);
 const diagnosticId='9ab961be-2edd-4502-b26c-1943cf272380';
 const diagnostic=await ordinary('/api/proxy-fixture?private-query-secret',{Cookie:'private-cookie-secret',Authorization:'Bearer private-authorization-secret','X-SchoolOrbit-Diagnostic-Id':diagnosticId});
 assert.equal(diagnostic.body.diagnosticId,diagnosticId);
 for(const invalid of ['private-header-secret',diagnosticId.toUpperCase(),'9ab961be-2edd-5502-b26c-1943cf272380',`${diagnosticId}, ${diagnosticId}`]){
  const response=await ordinary('/api/proxy-fixture',{'X-SchoolOrbit-Diagnostic-Id':invalid});
  assert.equal(response.body.diagnosticId,'-');
 }
 // Separate wire header lines must be rejected even when each value alone is valid.
 for(const duplicate of [diagnosticId,'private-duplicate-secret']){
  const response=request('school','/api/proxy-fixture',['-H',`X-SchoolOrbit-Diagnostic-Id: ${diagnosticId}`,'-H',`X-SchoolOrbit-Diagnostic-Id: ${duplicate}`]);
  const [body,status]=response.split('\n');
  assert.equal(status,'200');
  assert.equal(JSON.parse(body).diagnosticId,'-');
 }
 assert.equal(replies[0].diagnosticId,'-');
 // Sanitized forwarding is explicit in every special location as well.
 for(const url of ['/api/files/fixture','/api/admission/portal/upload','/api/notifications/stream','/ws/fixture']){
  const valid=request('school',url,['-i','-H',`X-SchoolOrbit-Diagnostic-Id: ${diagnosticId}`]);
  assert.match(valid,/\n401$/);
  assert.ok(valid.toLowerCase().includes(`x-fixture-diagnostic-id: ${diagnosticId}`));
  const invalid=request('school',url,['-i','-H','X-SchoolOrbit-Diagnostic-Id: private-header-secret']);
  assert.match(invalid,/x-fixture-diagnostic-id: -/i);
 }
 const preflight=request('school','/api/proxy-fixture',['-X','OPTIONS','-H','Origin: https://alpha.example.test','-H','Access-Control-Request-Headers: x-schoolorbit-diagnostic-id','-i']);
 assert.match(preflight,/Access-Control-Allow-Headers: [^\r\n]*X-SchoolOrbit-Diagnostic-Id/i);
 assert.match(preflight,/\n204$/);
 // The real Nginx parser and request path validate maps, JSON escaping and numeric fields.
 const events=docker('logs',proxy).split('\n').flatMap(line=>{
  try {const event=JSON.parse(line);return event.operation==='school_proxy_request'?[event]:[];} catch{return [];}
 });
 const correlated=events.filter(event=>event.diagnostic_id===diagnosticId);
 assert.equal(correlated.length,4);
 for(const event of events){
  assert.match(event.request_id,/^[0-9a-f]{32}$/);
  assert.equal(typeof event.request_time,'number');
  assert.equal(typeof event.status,'number');
  for(const field of ['upstream_connect_time','upstream_header_time','upstream_response_time']){
   assert.ok(event[field]===null || typeof event[field]==='number',`${field} must be numeric or null`);
  }
  assert.deepEqual(Object.keys(event).sort(),['operation','timestamp','request_id','diagnostic_id','method','route_family','status','request_time','upstream_connect_time','upstream_header_time','upstream_response_time'].sort());
  assert.doesNotMatch(JSON.stringify(event),/private-|PRIVATESECRET|fixture_session|Bearer|csrf-/);
 }
 assert.ok(events.some(event=>event.method==='OTHER'&&event.route_family==='other'));
 const proxied=correlated.find(event=>event.status===200);
 assert.equal(proxied.route_family,'other');
 assert.equal(proxied.method,'GET');
 for(const field of ['upstream_connect_time','upstream_header_time','upstream_response_time']) assert.equal(typeof proxied[field],'number');
 const local=events.find(event=>event.status===204);
 for(const field of ['upstream_connect_time','upstream_header_time','upstream_response_time']) assert.equal(local[field],null);

});
