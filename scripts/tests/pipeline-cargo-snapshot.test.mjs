import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';
import { trustedSnapshotRun, validateSnapshotManifest, resolveSnapshot, decodeArtifact } from '../lib/pipeline-cargo-snapshot.mjs';
import { validateTrialOrigin, validateTrialManifest } from '../lib/pipeline-cargo-snapshot-trial.mjs';

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
  const artifact = { id: 8, name: 'cargo-snapshot-backend-school', created_at: '2026-11-11T00:00:00Z', workflow_run: { id: 123 } };
  const list = route => route.includes('/jobs?') ? [{ jobs: [{ name: 'Prepare backend-school', status: 'completed', conclusion: 'success', head_sha: sha }] }] :
    route.includes('name=') ? [{ artifacts: [artifact] }] : [{ artifacts: [{ id: 9, name: 'prepared-backend-school', workflow_run: { id: 123 } }] }];
  const read = (_repository, item) => item.id === 8 ? manifest : prepared;
  assert.match(resolveSnapshot({ repository, identity, list, get: () => run, read }).reference, /@sha256:/);
  assert.equal(resolveSnapshot({ repository, identity, list, get: () => ({ ...run, head_branch: 'branch' }), read }), null);
  assert.equal(resolveSnapshot({ repository, identity, list, get: () => run, read: () => { throw Error('missing'); } }), null);
  assert.equal(resolveSnapshot({ repository, identity, list: () => [{ artifacts: [] }] }), null);
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
  const producer = { ...run, head_branch: 'perf', event: 'workflow_dispatch', path: '.github/workflows/operations.yml', created_at: '2026-11-11T00:00:00Z' };
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
