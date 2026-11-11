import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';
import { trustedSnapshotRun, validateSnapshotManifest, resolveSnapshot, decodeArtifact, artifactFromJobAttempt, assertPublisher, producerManifest } from '../lib/pipeline-cargo-snapshot.mjs';
import { validateTrialOrigin, validateTrialManifest, validateTrialPublisher } from '../lib/pipeline-cargo-snapshot-trial.mjs';

const repository = 'Owner/SchoolOrbit';
const sha = 'a'.repeat(40);
const hex = 'b'.repeat(64);
const digest = `sha256:${hex}`;
const identity = { buildConfigurationHash: hex, externalLockIdentityHash: hex };
const run = { id: 123, run_attempt: 2, head_sha: sha, head_branch: 'main', event: 'push',
  repository: { full_name: repository }, head_repository: { full_name: repository },
  path: '.github/workflows/pipeline.yml', status: 'completed', conclusion: 'success' };
const prepared = { schemaVersion: 1, component: 'backend-school', sha, inputHash: hex, digest };
const manifest = { schemaVersion: 1, component: 'backend-school',
  producer: { repository, workflowPath: run.path, runId: '123', runAttempt: 2, sha, event: 'push', ref: 'refs/heads/main' },
  snapshot: { ghcrRepository: 'ghcr.io/owner/schoolorbit-backend-school-cargo-cache', immutableDigest: digest,
    platform: 'linux/amd64', sourcePath: '/app' },
  identity: { ...identity, sourceManifestSha256: hex, backendInputHash: hex } };

test('production accepts only successful main Pipeline origin and binds prepared inputs to immutable digest', () => {
  assert.equal(trustedSnapshotRun(run, repository), true);
  assert.equal(validateSnapshotManifest(manifest, { run, repository, identity, prepared }).reference,
    `ghcr.io/owner/schoolorbit-backend-school-cargo-cache@${digest}`);
  for (const alteration of [{ head_branch: 'branch' }, { event: 'pull_request' }, { status: 'in_progress' },
    { conclusion: 'failure' }, { path: '.github/workflows/operations.yml' }, { head_repository: { full_name: 'Other/Fork' } }]) {
    assert.equal(trustedSnapshotRun({ ...run, ...alteration }, repository), false);
  }
  for (const alteration of [{ sha: 'c'.repeat(40) }, { inputHash: 'c'.repeat(64) }]) {
    assert.throws(() => validateSnapshotManifest(manifest, { run, repository, identity, prepared: { ...prepared, ...alteration } }));
  }
  assert.throws(() => validateSnapshotManifest({ ...manifest, producer: { ...manifest.producer, runAttempt: 1 } },
    { run, repository, identity, prepared }));
});

test('mutable registry tags cannot choose production snapshots and invalid/missing artifact falls cold', () => {
  const artifact = { id: 8, name: 'cargo-snapshot-backend-school', created_at: '2026-11-11T00:18:10Z', updated_at: '2026-11-11T00:18:11Z', workflow_run: { id: 123 } };
  const job = { name: 'Prepare backend-school', run_id: 123, run_attempt: 2, status: 'completed', conclusion: 'success', head_sha: sha,
    started_at: '2026-11-11T00:00:00Z', completed_at: '2026-11-11T00:20:00Z', steps: [
      { name: 'Publish immutable OCI manifest', conclusion: 'success', started_at: '2026-11-11T00:15:00Z', completed_at: '2026-11-11T00:16:00Z' },
      { name: 'Publish run-bound Cargo snapshot locator', conclusion: 'success', started_at: '2026-11-11T00:18:00Z', completed_at: '2026-11-11T00:19:00Z' }] };
  const preparedArtifact = { id: 9, name: 'prepared-backend-school', created_at: '2026-11-11T00:15:10Z', updated_at: '2026-11-11T00:15:11Z', workflow_run: { id: 123 } };
  const list = route => route.includes('/jobs?') ? [{ jobs: [job] }] :
    route.includes('name=') ? [{ artifacts: [artifact] }] : [{ artifacts: [preparedArtifact] }];
  const read = (_repository, item) => item.id === 8 ? manifest : prepared;
  assert.match(resolveSnapshot({ repository, identity, list, get: () => run, read }).reference, /@sha256:/);
  assert.equal(resolveSnapshot({ repository, identity, list, get: () => ({ ...run, head_branch: 'branch' }), read }), null);
  assert.equal(resolveSnapshot({ repository, identity, list, get: () => run, read: () => { throw Error('missing'); } }), null);
  assert.equal(resolveSnapshot({ repository, identity, list: () => [{ artifacts: [] }] }), null);
  const oldPrepared = { ...preparedArtifact, created_at: '2026-11-10T23:15:10Z', updated_at: '2026-11-10T23:15:11Z' };
  assert.equal(artifactFromJobAttempt(oldPrepared, job, run, 'Publish immutable OCI manifest'), false);
  assert.equal(artifactFromJobAttempt(preparedArtifact, { ...job, run_attempt: 1 }, run, 'Publish immutable OCI manifest'), false);
  const oldList = route => route.includes('/jobs?') ? [{ jobs: [job] }] : route.includes('name=') ? [{ artifacts: [artifact] }] : [{ artifacts: [oldPrepared] }];
  assert.equal(resolveSnapshot({ repository, identity, list: oldList, get: () => run, read }), null);

});

test('artifact decoder rejects duplicate JSON keys, duplicate paths and traversal without extraction', () => {
  const zip = (name, value, duplicate = false) => execFileSync('python3', ['-c',
    'import io,sys,zipfile\nb=io.BytesIO()\nwith zipfile.ZipFile(b,"w") as z:\n z.writestr(sys.argv[1],sys.argv[2])\n if sys.argv[3]=="yes":z.writestr(sys.argv[1],sys.argv[2])\nsys.stdout.buffer.write(b.getvalue())',
    name, value, duplicate ? 'yes' : 'no'], { stdio: ['ignore', 'pipe', 'pipe'] });
  assert.deepEqual(decodeArtifact(zip('snapshot.json', '{"x":1}'), 'snapshot.json'), { x: 1 });
  for (const archive of [zip('../snapshot.json', '{}'), zip('snapshot.json', '{"x":1,"x":2}'), zip('snapshot.json', '{}', true)]) {
    assert.throws(() => decodeArtifact(archive, 'snapshot.json'));
  }
});

test('diagnostic branch origin is separate, same reviewed SHA and distinct workflow runs are mandatory', () => {
  const current = { repository, sha, ref: 'refs/heads/perf', runId: '456' };
  const producer = { ...run, head_branch: 'perf', event: 'workflow_dispatch', path: '.github/workflows/operations.yml', created_at: '2026-11-11T00:00:00Z', updated_at: '2026-11-11T00:01:00Z' };
  const value = { schemaVersion: 1, producer: { repository, workflowPath: producer.path, runId: '123', runAttempt: 2,
    sha, event: 'workflow_dispatch', ref: current.ref }, registryPackage: 'ghcr.io/owner/schoolorbit-cache-benchmark-123-2',
    images: { cargoSnapshot: digest, compilerCache: digest, buildkitCache: digest }, identity,
    sourceManifestHash: hex, compilerSnapshotHash: hex, sourceContextHash: hex,
    buildkitImage: `moby/buildkit@${digest}`, cpus: 4, expiresAt: '2026-11-12T00:00:00Z' };
  assert.equal(validateTrialManifest(value, producer, current, Date.parse('2026-11-11T01:00:00Z')), value);
  for (const changed of [{ ...current, runId: '123' }, { ...current, sha: 'c'.repeat(40) }, { ...current, ref: 'refs/heads/other' }]) {
    assert.throws(() => validateTrialOrigin(producer, changed, { producerRunId: '123', producerAttempt: 2 }));
  }
  assert.throws(() => validateTrialManifest(value, producer, current, Date.parse('2026-11-13T00:00:00Z')));
});

test('cache refusal is the only fallback boundary; compiler/native-tool attempts remain strict', async () => {
  const { readFile } = await import('node:fs/promises');
  const action = await readFile(new URL('../../.github/actions/build-backend-image/action.yml', import.meta.url), 'utf8');
  const step = id => {
    const start = action.indexOf(`      id: ${id}\n`);
    assert.ok(start > 0, `${id} exists`);
    const end = action.indexOf('\n    - ', start);
    return action.slice(start, end < 0 ? undefined : end);
  };
  assert.match(step('snapshot-preflight'), /continue-on-error: true/);
  assert.doesNotMatch(step('snapshot-native-gate'), /continue-on-error/);
  assert.doesNotMatch(step('snapshot-materialization-gate'), /continue-on-error/);
  assert.doesNotMatch(step('build'), /continue-on-error/);
  assert.equal((action.match(/target: runtime/g) || []).length, 1, 'Exactly one strict application build');
  assert.match(step('select'), /snapshot\.select_recipe\(/);
  assert.match(step('build'), /SCHOOL_CARGO_BUILD_STAGE=\$\{\{ steps\.select\.outputs\.stage \}\}/);
  assert.doesNotMatch(action, /snapshot-build\.outcome/);

});

test('prepared attempt binding accepts inclusive API upload boundaries and refuses failed/duplicate uploads', () => {
  // Public timing observation from trusted-main run42972709750/attempt1/job130364817437.
  const observedRun = { id: 42972709750, run_attempt: 1, head_sha: '431ef887bc94f608df6684b8a1f777a570c81a40' };
  const upload = { name: 'Publish immutable OCI manifest', conclusion: 'success',
    started_at: '2026-11-11T06:40:04Z', completed_at: '2026-11-11T06:40:05Z' };
  const job = { run_id: observedRun.id, run_attempt: 1, head_sha: observedRun.head_sha,
    status: 'completed', conclusion: 'success', started_at: '2026-11-11T06:29:20Z',
    completed_at: '2026-11-11T06:40:16Z', steps: [upload] };
  const artifact = { id: 13834492333, workflow_run: { id: observedRun.id },
    created_at: '2026-11-11T06:40:05Z', updated_at: '2026-11-11T06:40:05Z' };
  assert.equal(artifactFromJobAttempt(artifact, job, observedRun, upload.name), true);
  assert.equal(artifactFromJobAttempt(artifact, { ...job, steps: [{ ...upload, conclusion: 'failure' }] }, observedRun, upload.name), false);
  assert.equal(artifactFromJobAttempt(artifact, { ...job, steps: [upload, upload] }, observedRun, upload.name), false);
  assert.equal(artifactFromJobAttempt({ ...artifact, updated_at: '2026-11-11T06:40:06Z' }, job, observedRun, upload.name), false);
});

test('publisher validates current main job and attempt before registry authority is emitted', t => {
  const keys = ['GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'GITHUB_SHA', 'GITHUB_REF', 'GITHUB_JOB', 'COMPONENT', 'INPUT_HASH'];
  const prior = Object.fromEntries(keys.map(key => [key, process.env[key]]));
  t.after(() => { for (const key of keys) { if (prior[key] === undefined) delete process.env[key]; else process.env[key] = prior[key]; } });
  Object.assign(process.env, { GITHUB_RUN_ID: '123', GITHUB_RUN_ATTEMPT: '2', GITHUB_SHA: sha,
    GITHUB_REF: 'refs/heads/main', GITHUB_JOB: 'prepare', COMPONENT: 'backend-school', INPUT_HASH: hex });
  assert.doesNotThrow(() => assertPublisher(run, repository, prepared));
  for (const [key, value] of [['GITHUB_REF', 'refs/heads/branch'], ['GITHUB_RUN_ATTEMPT', '1'], ['GITHUB_JOB', 'other']]) {
    const original = process.env[key]; process.env[key] = value;
    assert.throws(() => assertPublisher(run, repository, prepared)); process.env[key] = original;
  }
  const sourceManifest = [{ mode: 420, path: 'src/ไทย🙂.rs', sha256: hex }];
  const sourceHash = execFileSync('python3', ['-c',
    'import hashlib,json,sys;print(hashlib.sha256(json.dumps(json.loads(sys.argv[1]),sort_keys=True,separators=(",",":")).encode()).hexdigest())',
    JSON.stringify(sourceManifest)], { encoding: 'utf8' }).trim();
  const value = producerManifest({ repository, run, prepared, digest, identity,
    metadata: { schemaVersion: 1, configurationHash: hex, sourceManifest, sourceManifestSha256: sourceHash } });
  assert.equal(value.producer.runAttempt, 2);
  assert.equal(value.snapshot.immutableDigest, digest);
  assert.equal(value.identity.sourceManifestSha256, sourceHash);
  assert.throws(() => producerManifest({ repository, run: { ...run, event: 'pull_request' }, prepared, digest, identity, metadata: {} }));
});

test('the action executes the tested recipe selector and publishes a real newline-delimited output', async t => {
  const fs = await import('node:fs');
  const os = await import('node:os');
  const path = await import('node:path');
  const { fileURLToPath } = await import('node:url');
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const action = fs.readFileSync(path.join(root, '.github/actions/build-backend-image/action.yml'), 'utf8');
  const body = action.slice(action.indexOf('      id: select\n')).match(/python3 -B - <<'PYTHON'\n([\s\S]*?)\n        PYTHON/)[1].replace(/^        /gm, '');
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'cargo-recipe-action-'));
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }));
  const native = path.join(temporary, 'native.json'), materialized = path.join(temporary, 'materialized.json');
  for (const [index, outcome, n, m, expected] of [
    [0, 'skipped', true, true, 'fresh-builder'], [1, 'failure', true, true, 'fresh-builder'],
    [2, 'success', false, true, 'fresh-builder'], [3, 'success', true, false, 'fresh-builder'],
    [4, 'success', true, true, 'snapshot-builder'],
  ]) {
    fs.writeFileSync(native, JSON.stringify({ compatible: n }));
    fs.writeFileSync(materialized, JSON.stringify({ compatible: m }));
    const output = path.join(temporary, `output-${index}`);
    execFileSync('python3', ['-B', '-c', body], { cwd: root, env: { ...process.env,
      PREFLIGHT_OUTCOME: outcome, NATIVE_GATE: native, MATERIALIZATION_GATE: materialized, GITHUB_OUTPUT: output } });
    assert.equal(fs.readFileSync(output, 'utf8'), `stage=${expected}\n`);
  }
  fs.writeFileSync(native, '{"compatible":"true"}');
  assert.throws(() => execFileSync('python3', ['-B', '-c', body], { cwd: root, stdio: 'pipe', env: { ...process.env,
    PREFLIGHT_OUTCOME: 'success', NATIVE_GATE: native, MATERIALIZATION_GATE: materialized, GITHUB_OUTPUT: path.join(temporary, 'bad-output') } }));
});

test('explicit cleanup can reclaim an older owned producer while consumption and publisher remain exact', () => {
  const current={repository,sha,ref:'refs/heads/current',runId:'456'};
  const old={...run,head_sha:'c'.repeat(40),head_branch:'old',event:'workflow_dispatch',path:'.github/workflows/operations.yml',conclusion:'failure'};
  const options={producerRunId:'123',producerAttempt:2};
  assert.throws(()=>validateTrialOrigin(old,current,options));
  assert.equal(validateTrialOrigin(old,current,{...options,cleanup:true}),'schoolorbit-cache-benchmark-123-2');
  for(const changed of [{...old,status:'in_progress'}, {...old,event:'push'}, {...old,path:'.github/workflows/pipeline.yml'},
    {...old,repository:{full_name:'Other/Repo'}},{...old,head_repository:{full_name:'Other/Fork'}},{...old,run_attempt:1}]) {
    assert.throws(()=>validateTrialOrigin(changed,current,{...options,cleanup:true}));
  }
  assert.throws(()=>validateTrialOrigin(old,{...current,runId:'123'},{...options,cleanup:true}));
  assert.throws(()=>validateTrialOrigin(old,current,{producerRunId:'../runtime',producerAttempt:2,cleanup:true}));
  const live={...old,id:456,head_sha:sha,head_branch:'current',run_attempt:1,status:'in_progress',conclusion:null};
  const value={producer:{repository,workflowPath:live.path,runId:'456',runAttempt:1,sha,event:'workflow_dispatch',ref:current.ref}};
  assert.doesNotThrow(()=>validateTrialPublisher(value,live,current,{job:'benchmark',attempt:1}));
  assert.throws(()=>validateTrialPublisher(value,live,current,{job:'other',attempt:1}));
  assert.throws(()=>validateTrialPublisher(value,live,current,{job:'benchmark',attempt:2}));
});
