import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
function fixture(t, body) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'cargo-trial-fixture-'));
  t.after(() => rmSync(root, {recursive:true,force:true}));
  for (const name of ['backend-school', 'contracts/openapi', 'output', 'compiler']) mkdirSync(path.join(root,name), {recursive:true});
  writeFileSync(path.join(root,'backend-school/Dockerfile'), 'FROM fixture@sha256:'+'a'.repeat(64)+' AS runtime\n');
  writeFileSync(path.join(root,'contracts/openapi/school-api.json'), '{"openapi":"3.1.0","paths":{}}');
  writeFileSync(path.join(root,'compiler/entry'), 'frozen-compiler-bytes');
  return execFileSync('python3', ['-B','-c', `import sys,os,json,pathlib,subprocess\nfrom unittest.mock import patch\nsys.path.insert(0,${JSON.stringify(path.join(repo,'scripts'))})\nimport benchmark_cargo_snapshot as b\nroot=pathlib.Path(${JSON.stringify(root)})\nos.environ.update(GITHUB_RUN_ID='123',GITHUB_RUN_ATTEMPT='1')\n${body}`], {encoding:'utf8',stdio:['ignore','pipe','pipe']});
}

test('trial removes each exact owned verification image and destroys builder before offline API', t => {
  fixture(t, `
calls=[]
def fake(args,log=None):
 calls.append(args)
 if args[:3]==['docker','image','inspect']:return json.dumps([{'Config':{'User':'appuser','Cmd':['/app/backend-school']}}])
 if 'id' in args:return '1000'
 if 'sha256sum' in args:return 'a'*64+'  /app/backend-school'
 return ''
def api(args,**kwargs):
 calls.append(['api-container'])
 return b'{"paths":{},"openapi":"3.1.0"}\\n'
trial=b.Trial(root,root/'output','fixture@sha256:'+'a'*64,'ghcr.io/owner/scratch',root/'compiler')
trial.builders=['owned-builder']
folder=root/'output/proof';folder.mkdir()
with patch.object(b,'run',side_effect=fake),patch.object(b,'resource_checkpoint'),patch.object(b.subprocess,'check_output',side_effect=api):
 result=trial.runtime('ghcr.io/owner/scratch@sha256:'+'b'*64)
 assert result['uid']==1000 and not trial.local_images
 proof=trial.api('owned-builder',root/'backend-school',folder,[])
 assert proof['rawBytes']>0 and len(proof['rawSha256'])==64
 assert (folder/'offline-api.json').read_bytes().endswith(b'\\n')
 assert not trial.builders and not trial.local_images
 assert calls.index(['docker','buildx','rm','--force','owned-builder'])<calls.index(['api-container'])
 assert calls[-1]==['docker','image','rm','schoolorbit-cargo-snapshot-docs:123-0']
 assert all('prune' not in call for call in calls)
`);
});

test('API verification failure still removes the exact image and cannot report success', t => {
  fixture(t, `
calls=[]
trial=b.Trial(root,root/'output','fixture@sha256:'+'a'*64,'ghcr.io/owner/scratch',root/'compiler')
trial.builders=['owned-builder'];folder=root/'output/proof';folder.mkdir()
with patch.object(b,'run',side_effect=lambda args,log=None:calls.append(args) or ''),patch.object(b,'resource_checkpoint'),patch.object(b.subprocess,'check_output',return_value=b'bad-json'):
 try:trial.api('owned-builder',root/'backend-school',folder,[])
 except ValueError:pass
 else:raise AssertionError('Invalid API accepted')
assert calls[-1]==['docker','image','rm','schoolorbit-cargo-snapshot-docs:123-0']
assert not trial.local_images and not trial.builders
`);
});

test('missing fixture/orchestration timings and unavailable disk reserve refuse acceptance', t => {
  fixture(t, `
for value in (None,True,-1,float('nan')):
 try:b.required_seconds({'duration':value},'duration')
 except ValueError:pass
 else:raise AssertionError('Unavailable duration accepted')
try:b.validate_costs({'cost':{},'orchestration':{}})
except ValueError:pass
else:raise AssertionError('Missing costs accepted')
class TinyDisk:
 f_bavail=1
 f_frsize=4096
with patch.object(b.os,'statvfs',return_value=TinyDisk()):
 try:b.resource_checkpoint(root/'output','fixture-low-disk',8)
 except ValueError:pass
 else:raise AssertionError('Unavailable disk reserve accepted')
assert json.loads((root/'output/resource-checkpoints.json').read_text())[0]['freeBytes']==4096
`);
});
