import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
function python(code) {
  const result = spawnSync('python3', ['-B', '-c', `import sys\nsys.path.insert(0, 'scripts')\n${code}`], {cwd:repo,encoding:'utf8',env:{...process.env,PYTHONDONTWRITEBYTECODE:'1'}});
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}
test('private daemon failures expose only whitelisted classes and HTTP statuses', () => {
  const result = python(`from diagnose_compiler_cache import classify_private_log\nimport json\nprint(json.dumps(classify_private_log('''[2026-10-14T00:00:00Z DEBUG sccache::server] Error executing cache write: RateLimited at write, status: 429; url=https://private.invalid/?sig=secret-sentinel; Authorization: Bearer secret-sentinel; too many requests
[2026-10-14T00:00:01Z DEBUG sccache::server] harmless compile request with env secret-sentinel
storage write check failed: PermissionDenied status_code=403 quota exceeded private-key-sentinel''')))\n`);
  assert.deepEqual(result, [
    {stage:'cache_write',kind:'RateLimited',http_statuses:[429],signals:['throttled']},
    {stage:'write_check',kind:'PermissionDenied',http_statuses:[403],signals:['quota']}
  ]);
  assert.doesNotMatch(JSON.stringify(result),/sentinel|private\.invalid|Authorization|Bearer/);
});
test('unknown failure text and injected metadata are never published', () => {
  const result = python(`from diagnose_compiler_cache import classify_private_log\nimport json\nprint(json.dumps(classify_private_log('Error executing cache write: secret-sentinel\\nX-Header: secret-sentinel')))\n`);
  assert.deepEqual(result,[{stage:'cache_write',kind:'unclassified',http_statuses:[],signals:[]}]);
});
test('probe recipe derives the same pinned compiler and binary without release artifacts', () => {
  const result = python(`from diagnose_compiler_cache import probe_recipe\nfrom pathlib import Path\nimport json\nprint(json.dumps(probe_recipe(Path('backend-school/Dockerfile').read_text())))\n`);
  const source = readFileSync(path.join(repo,'backend-school/Dockerfile'),'utf8');
  assert.ok(result.includes(source.match(/^FROM (\S+) AS base$/m)[1]));
  assert.ok(result.includes(source.match(/--checksum=(sha256:[a-f0-9]{64})/)[1]));
  assert.match(result,/COPY --chmod=0755 rustc-cache \/usr\/local\/bin\/rustc-cache/);
  assert.doesNotMatch(result,/TOKEN|RESULTS_URL|PUSH|COPY \. \./);
});
test('fresh containers prove a new remote write and subsequent restoration without exporting private logs', () => {
  const code = `import json,os,tempfile\nfrom pathlib import Path\nimport diagnose_compiler_cache as p\nroot=tempfile.TemporaryDirectory()\nos.environ.update(GITHUB_RUN_ID='12345',GITHUB_RUN_ATTEMPT='1',ACTIONS_RESULTS_URL='private-url-sentinel',ACTIONS_RUNTIME_TOKEN='private-token-sentinel',COMPILER_CACHE_OUTPUT=root.name)\ncommands=[]\ncurrent=0\ndef run(args,stage):\n global current\n commands.append(args)\n if stage=='pinned_image':\n  context=Path(args[-1])\n  assert (context/'rustc-cache').read_bytes()==b'#!/bin/sh\\nexec /usr/local/bin/sccache \"$@\"\\n'\n  assert (context/'fixture/Cargo.toml').is_file()\n if stage=='fresh_container':\n  current+=1\n  assert any(x.startswith('type=bind,src=') and x.endswith(',dst=/fixture,readonly') for x in args)\n if stage=='probe_context': assert args==['docker','exec',f'compiler-cache-probe-12345-1-{current}','sh','-c','cp -a /fixture/. /probe/']\n if stage=='source': return 'a'*40\n return ''\ndef counters(_):\n return dict(cache_hits=int(current==2),cache_misses=int(current==1),cache_writes=int(current==1),cache_errors=0,cache_read_errors=0,cache_write_errors=0,cache_timeouts=0)\nclass Result:\n stdout='private-daemon-sentinel'\np.command=run\np.stats=counters\np.subprocess.run=lambda args,**kw: (commands.append(args) or Result())\np.main()\ne=json.loads((Path(root.name)/'evidence.json').read_text())\nassert e['complete'] and e['healthy'] and len(e['samples'])==2\nassert sum(x[:3]==['docker','rm','--force'] for x in commands)==2\nassert any('/private:rw,nosuid,nodev,noexec,mode=0700,size=16m' in x for x in commands)\nassert not any('private-token-sentinel' in ' '.join(x) for x in commands)\n`;
  const result = spawnSync('python3',['-B','-c',`import sys\nsys.path.insert(0,'scripts')\n${code}`],{cwd:repo,encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);
  const evidence=JSON.parse(result.stdout);
  assert.equal(evidence.healthy,true);
  assert.doesNotMatch(result.stdout+result.stderr,/private-.*sentinel/);
});
test('compiler failures retain only sanitized diagnostics and clean the owned container', () => {
  const code = `import json,os,tempfile\nfrom pathlib import Path\nimport diagnose_compiler_cache as p\nroot=tempfile.TemporaryDirectory()\nos.environ.update(GITHUB_RUN_ID='12345',GITHUB_RUN_ATTEMPT='1',ACTIONS_RESULTS_URL='private-url',ACTIONS_RUNTIME_TOKEN='private-token',COMPILER_CACHE_OUTPUT=root.name)\ncommands=[]\ndef run(args,stage):\n commands.append(args)\n if stage=='compile': raise p.ProbeFailure('compile',2)\n return 'a'*40 if stage=='source' else ''\nclass Result:\n stdout='storage write check failed: Unexpected status=503 private-token-sentinel'\np.command=run\np.subprocess.run=lambda args,**kw: (commands.append(args) or Result())\ntry: p.main()\nexcept SystemExit as e: assert e.code==1\ne=json.loads((Path(root.name)/'evidence.json').read_text())\nassert not e['complete'] and not e['healthy']\nassert e['samples'][0]['counters'] is None\nassert e['samples'][0]['diagnostics'][0]['http_statuses']==[503]\nassert any(x[:3]==['docker','rm','--force'] for x in commands)\n`;
  const result=spawnSync('python3',['-B','-c',`import sys\nsys.path.insert(0,'scripts')\n${code}`],{cwd:repo,encoding:'utf8'});
  assert.equal(result.status,0,result.stderr);
  assert.doesNotMatch(result.stdout+result.stderr,/sentinel/);
});
test('operations publishes only the numeric evidence file and never private daemon logs',()=>{
  const workflow=readFileSync(path.join(repo,'.github/workflows/compiler-cache-diagnostic.yml'),'utf8');
  const operations=readFileSync(path.join(repo,'.github/workflows/operations.yml'),'utf8');
  assert.match(workflow,/core\.setSecret\(token\)/);
  assert.match(workflow,/path: \$\{\{ runner\.temp \}\}\/compiler-cache-evidence\/evidence\.json/);
  assert.doesNotMatch(workflow,/private\/|server\.log|docker push|cache-to:/);
  assert.match(operations,/uses: \.\/\.github\/workflows\/compiler-cache-diagnostic\.yml/);
});
