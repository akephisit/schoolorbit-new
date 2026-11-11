import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
const prepare = readFileSync(path.join(repo, '.github/workflows/prepare.yml'), 'utf8');
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'cargo-wiring-'));
  t.after(() => rmSync(root, {recursive:true,force:true}));
  return root;
}
function stepById(id) {
  const start = prepare.indexOf(`        id: ${id}\n`);
  assert.ok(start > 0);
  const end = prepare.indexOf('\n      - ', start);
  return prepare.slice(start, end < 0 ? undefined : end);
}

test('optional publisher executes a scratch/COPY-only context; compiler-reachable export is strict', t => {
  const root = fixture(t), carrier = path.join(root,'cargo-snapshot-export');
  mkdirSync(carrier);
  writeFileSync(path.join(carrier,'snapshot.tar'),'fixture transport bytes');
  writeFileSync(path.join(carrier,'snapshot.json'),'{}\n');
  const strict = stepById('cargo-snapshot-export');
  assert.match(strict,/target: cargo-snapshot/);
  assert.doesNotMatch(strict,/continue-on-error/);
  const materialize = stepById('cargo-snapshot-metadata');
  const body = materialize.match(/python3 - <<'PYTHON'\n([\s\S]*?)\n          PYTHON/)[1].replace(/^          /gm,'');
  execFileSync('python3',['-B','-c',body],{env:{...process.env,RUNNER_TEMP:root}});
  assert.equal(readFileSync(path.join(carrier,'Dockerfile'),'utf8'),'FROM scratch\nCOPY snapshot.tar /snapshot.tar\nCOPY snapshot.json /snapshot.json\n');
  assert.equal(readFileSync(path.join(root,'cargo-snapshot-metadata/snapshot.json'),'utf8'),'{}\n');
  const optional = stepById('cargo-snapshot-build');
  assert.match(optional,/continue-on-error: true/);
  assert.match(optional,/context: \$\{\{ runner.temp \}\}\/cargo-snapshot-export/);
  assert.doesNotMatch(optional,/target:|build-contexts:|secret-envs:|build-args:/);
});

test('registry-only reuse inspects without a builder and fails when the CLI or registry is unavailable', t => {
  const root = fixture(t), bin=path.join(root,'bin');mkdirSync(bin);
  writeFileSync(path.join(bin,'docker'), '#!/bin/bash\nprintf "%s\\n" "$*" >> "$CALLS"\nif [[ "$*" == *--help* ]]; then exit "$CLI_STATUS"; fi\ncase "$IMAGE_STATUS" in present) exit 0;; missing) echo "manifest unknown" >&2; exit 1;; forbidden) echo "HTTP 403 Forbidden" >&2; exit 1;; esac\n',{mode:0o755});
  const body = stepById('image').match(/        run: \|\n([\s\S]*)/)[1].replace(/^          /gm,'');
  const cli = 'docker buildx imagetools inspect --help >/dev/null';
  assert.ok(prepare.indexOf(cli)<prepare.indexOf('        id: image\n'));
  const builder = prepare.slice(prepare.indexOf('      - uses: docker/setup-buildx-action@v4'));
  assert.match(builder.split('\n').slice(0,3).join('\n'),/steps.image.outputs.reused != 'true'/);
  for (const [index,image,cliStatus,status,reused] of [[0,'present','0',0,'true'],[1,'missing','0',0,'false'],[2,'forbidden','0',1,null],[3,'present','64',64,null]]) {
    const output=path.join(root,`output-${index}`), calls=path.join(root,`calls-${index}`);
    const result=spawnSync('bash',['-e','-c',cli+'\n'+body],{encoding:'utf8',env:{...process.env,PATH:bin+':'+process.env.PATH,
      GITHUB_REPOSITORY_OWNER:'fixture',COMPONENT:'backend-school',INPUT_HASH:'a'.repeat(64),RUNNER_TEMP:root,GITHUB_OUTPUT:output,
      CALLS:calls,IMAGE_STATUS:image,CLI_STATUS:cliStatus}});
    assert.equal(result.status,status,result.stderr);
    const lines=readFileSync(calls,'utf8');
    assert.doesNotMatch(lines,/buildx (?:create|build|inspect --bootstrap)/);
    if(reused)assert.match(readFileSync(output,'utf8'),new RegExp(`reused=${reused}`));
    else assert.throws(()=>readFileSync(output));
  }
});

test('cleanup targets only the already-verified producer package and fails on unknown API errors', t => {
  const root=fixture(t),bin=path.join(root,'bin');mkdirSync(bin);
  writeFileSync(path.join(bin,'gh'),'#!/bin/bash\nprintf "%s\\n" "$*" >> "$CALLS"\nif [[ "$*" == *GET* && "$PACKAGE_STATUS" == forbidden ]]; then echo "HTTP 403" >&2; exit 1; fi\n',{mode:0o755});
  const workflow=readFileSync(path.join(repo,'.github/workflows/backend-cache-benchmark.yml'),'utf8');
  const body=workflow.match(/name: Remove only this run's disposable registry package[\s\S]*?        run: \|\n([\s\S]*)/)[1].replace(/^          /gm,'');
  for(const [index,name,status,expected] of [[0,'schoolorbit-cache-benchmark-77-2','present',0],[1,'schoolorbit-cache-benchmark-77-2','forbidden',1],[2,'schoolorbit-backend-school','present',1]]){
    const calls=path.join(root,`calls-${index}`);
    const result=spawnSync('bash',['-c',body],{encoding:'utf8',env:{...process.env,PATH:bin+':'+process.env.PATH,
      CALLS:calls,PACKAGE_STATUS:status,OWNER:'fixture',OWNER_TYPE:'User',RUNNER_TEMP:root,GITHUB_RUN_ID:'123',GITHUB_RUN_ATTEMPT:'1',VERIFIED_PRODUCER_PACKAGE:name}});
    assert.equal(result.status,expected,result.stderr);
    if(index<2){const value=readFileSync(calls,'utf8');assert.match(value,/container\/schoolorbit-cache-benchmark-77-2/);
      assert.equal(value.includes('DELETE'),status==='present');assert.doesNotMatch(value,/schoolorbit-cache-benchmark-123-1/);}
    else assert.throws(()=>readFileSync(calls));
  }
});
