import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export const SNAPSHOT_ARTIFACT = 'cargo-snapshot-backend-school';
export const SNAPSHOT_PACKAGE = 'schoolorbit-backend-school-cargo-cache';
const HEX = /^[0-9a-f]{64}$/;
const SHA = /^[0-9a-f]{40}$/;
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;
const WORKFLOW = '.github/workflows/pipeline.yml';

export function trustedSnapshotRun(run, repository) {
  return REPOSITORY.test(repository) && run?.status === 'completed' && run.conclusion === 'success' &&
    run.repository?.full_name === repository && run.head_repository?.full_name === repository &&
    run.head_branch === 'main' && ['push', 'workflow_dispatch'].includes(run.event) &&
    run.path?.replace(/@(?:refs\/heads\/)?main$/, '') === WORKFLOW && SHA.test(run.head_sha || '') &&
    Number.isSafeInteger(run.id) && run.id > 0 && Number.isSafeInteger(run.run_attempt) && run.run_attempt > 0;
}

export function validateSnapshotManifest(manifest, { run, repository, identity, prepared }) {
  if (!trustedSnapshotRun(run, repository)) throw new Error('untrusted_origin');
  const producer = manifest?.producer;
  const snapshot = manifest?.snapshot;
  const saved = manifest?.identity;
  const expectedPackage = `ghcr.io/${repository.split('/')[0].toLowerCase()}/${SNAPSHOT_PACKAGE}`;
  if (manifest.schemaVersion !== 1 || manifest.component !== 'backend-school' ||
      producer?.repository !== repository || producer.workflowPath !== WORKFLOW ||
      producer.runId !== String(run.id) || producer.runAttempt !== run.run_attempt ||
      producer.sha !== run.head_sha || producer.event !== run.event || producer.ref !== 'refs/heads/main' ||
      snapshot?.ghcrRepository !== expectedPackage || !DIGEST.test(snapshot.immutableDigest || '') ||
      snapshot.platform !== 'linux/amd64' || snapshot.sourcePath !== '/app' ||
      !HEX.test(saved?.buildConfigurationHash || '') || !HEX.test(saved?.externalLockIdentityHash || '') ||
      !HEX.test(saved?.sourceManifestSha256 || '') || !HEX.test(saved?.backendInputHash || '') ||
      saved.buildConfigurationHash !== identity.buildConfigurationHash ||
      saved.externalLockIdentityHash !== identity.externalLockIdentityHash ||
      prepared?.schemaVersion !== 1 || prepared.component !== 'backend-school' ||
      prepared.sha !== run.head_sha || prepared.inputHash !== saved.backendInputHash || !DIGEST.test(prepared.digest || '')) {
    throw new Error('snapshot_identity');
  }
  return { reference: `${expectedPackage}@${snapshot.immutableDigest}`,
    configurationHash: saved.buildConfigurationHash, sourceManifestHash: saved.sourceManifestSha256,
    producerRunId: producer.runId, producerRunAttempt: producer.runAttempt };
}

const api = route => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }));
const pages = route => JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', route], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }));

export function decodeArtifact(archive, filename) {
  if (!Buffer.isBuffer(archive) || archive.length > 1024 * 1024) throw new Error('artifact_size');
  // Read one bounded regular JSON member in memory; never extract ZIP paths to the host.
  const code = `import io,json,stat,sys,zipfile
raw=sys.stdin.buffer.read(1048577)
assert len(raw)<=1048576
z=zipfile.ZipFile(io.BytesIO(raw)); names=z.namelist(); assert names==[sys.argv[1]]
i=z.getinfo(sys.argv[1]); assert 0<i.file_size<=65536 and not i.is_dir() and not i.flag_bits&1
mode=i.external_attr>>16; assert not stat.S_ISLNK(mode)
def unique(pairs):
 d={}
 for k,v in pairs:
  if k in d: raise ValueError('duplicate JSON key')
  d[k]=v
 return d
value=json.loads(z.read(i),object_pairs_hook=unique)
assert isinstance(value,dict)
print(json.dumps(value,separators=(',',':')))
`;
  return JSON.parse(execFileSync('python3', ['-c', code, filename], { input: archive, maxBuffer: 128 * 1024, stdio: ['pipe', 'pipe', 'pipe'] }).toString());
}

const download = (repository, artifact, filename) => decodeArtifact(
  execFileSync('gh', ['api', `/repos/${repository}/actions/artifacts/${artifact.id}/zip`], { maxBuffer: 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] }), filename);

export function resolveSnapshot({ repository, identity, list = pages, get = api, read = download }) {
  if (!REPOSITORY.test(repository || '')) throw new Error('repository');
  const artifacts = list(`/repos/${repository}/actions/artifacts?name=${SNAPSHOT_ARTIFACT}&per_page=100`)
    .flatMap(page => page.artifacts || []).filter(a => !a.expired && a.name === SNAPSHOT_ARTIFACT)
    .sort((a, b) => Date.parse(b.created_at) - Date.parse(a.created_at));
  for (const artifact of artifacts) {
    try {
      const run = get(`/repos/${repository}/actions/runs/${artifact.workflow_run.id}`);
      if (!trustedSnapshotRun(run, repository)) continue;
      const manifest = read(repository, artifact, `${SNAPSHOT_ARTIFACT}.json`);
      if (manifest.producer?.runAttempt !== run.run_attempt) continue;
      const jobs = list(`/repos/${repository}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`)
        .flatMap(page => page.jobs || []).filter(job => /(?:^| \/ )Prepare backend-school$/.test(job.name));
      if (jobs.length !== 1 || jobs[0].status !== 'completed' || jobs[0].conclusion !== 'success' || jobs[0].head_sha !== run.head_sha) continue;
      const prepared = list(`/repos/${repository}/actions/runs/${run.id}/artifacts?per_page=100`)
        .flatMap(page => page.artifacts || []).filter(a => !a.expired && a.name === 'prepared-backend-school' && a.workflow_run?.id === run.id);
      if (prepared.length !== 1) continue;
      return validateSnapshotManifest(manifest, { run, repository, identity,
        prepared: read(repository, prepared[0], 'backend-school.json') });
    } catch {
      // Optional cache lookup fails cold. Never print raw API/download errors or signed URLs.
    }
  }
  return null;
}

export function currentIdentity(root, cacheMode = 'gha') {
  return JSON.parse(execFileSync('python3', [path.join(root, 'scripts/cargo_snapshot.py'), 'identity', root, '--cache-mode', cacheMode],
    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
}

export function assertPublisher(run, repository, prepared) {
  if (!REPOSITORY.test(repository || '') || run?.repository?.full_name !== repository || run.head_repository?.full_name !== repository ||
      run.head_branch !== 'main' || !['push', 'workflow_dispatch'].includes(run.event) ||
      run.path?.replace(/@(?:refs\/heads\/)?main$/, '') !== WORKFLOW || !SHA.test(run.head_sha || '') ||
      String(run.id) !== process.env.GITHUB_RUN_ID || run.run_attempt !== Number(process.env.GITHUB_RUN_ATTEMPT) ||
      run.head_sha !== process.env.GITHUB_SHA || process.env.GITHUB_REF !== 'refs/heads/main' ||
      process.env.GITHUB_JOB !== 'prepare' || process.env.COMPONENT !== 'backend-school' ||
      prepared?.schemaVersion !== 1 || prepared.component !== 'backend-school' || prepared.sha !== run.head_sha ||
      prepared.inputHash !== process.env.INPUT_HASH || !HEX.test(prepared.inputHash || '') || !DIGEST.test(prepared.digest || '')) {
    throw new Error('publisher_origin');
  }
}

function sourceManifestHash(records) {
  const ordered = records.map(record => ({ mode: record.mode, path: record.path, sha256: record.sha256 }));
  const ascii = JSON.stringify(ordered).replace(/[^\x00-\x7f]/g,
    character => `\\u${character.charCodeAt(0).toString(16).padStart(4, '0')}`);
  return createHash('sha256').update(ascii).digest('hex');
}

export function producerManifest({ repository, run, prepared, digest, metadata, identity }) {
  assertPublisher(run, repository, prepared);
  if (!REPOSITORY.test(repository || '') || run?.repository?.full_name !== repository || run.head_repository?.full_name !== repository ||
      run.head_branch !== 'main' || !['push', 'workflow_dispatch'].includes(run.event) ||
      run.path?.replace(/@(?:refs\/heads\/)?main$/, '') !== WORKFLOW || !SHA.test(run.head_sha || '') ||
      String(run.id) !== process.env.GITHUB_RUN_ID || run.run_attempt !== Number(process.env.GITHUB_RUN_ATTEMPT) ||
      run.head_sha !== process.env.GITHUB_SHA || process.env.GITHUB_REF !== 'refs/heads/main' || !DIGEST.test(digest || '') ||
      prepared?.schemaVersion !== 1 || prepared.component !== 'backend-school' || prepared.sha !== run.head_sha ||
      !HEX.test(prepared.inputHash || '') || !DIGEST.test(prepared.digest || '') ||
      metadata?.schemaVersion !== 1 || metadata.configurationHash !== identity.buildConfigurationHash ||
      !HEX.test(metadata.sourceManifestSha256 || '') ||
      sourceManifestHash(metadata.sourceManifest) !== metadata.sourceManifestSha256) {
    throw new Error('publisher_identity');
  }
  return { schemaVersion: 1, component: 'backend-school', producer: { repository, workflowPath: WORKFLOW,
    runId: String(run.id), runAttempt: run.run_attempt, sha: run.head_sha, event: run.event, ref: 'refs/heads/main' },
    snapshot: { ghcrRepository: `ghcr.io/${repository.split('/')[0].toLowerCase()}/${SNAPSHOT_PACKAGE}`,
      immutableDigest: digest, platform: 'linux/amd64', sourcePath: '/app' },
    identity: { ...identity, sourceManifestSha256: metadata.sourceManifestSha256, backendInputHash: prepared.inputHash } };
}

function output(values) {
  for (const [key, value] of Object.entries(values)) {
    if (/[\r\n]/.test(String(value))) throw new Error('invalid_output');
    appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = path.resolve('backend-school');
  const identity = currentIdentity(root);
  const mode = process.argv[2];
  if (mode === 'resolve') {
    let found = null;
    const enabled = process.env.SCHOOL_CARGO_SNAPSHOT_ENABLED === 'true';
    if (enabled) {
      try { found = resolveSnapshot({ repository: process.env.GITHUB_REPOSITORY, identity }); } catch { /* Cold fallback. */ }
    }
    output({ configuration_hash: identity.buildConfigurationHash, external_lock_hash: identity.externalLockIdentityHash,
      reference: found?.reference || '', source_manifest_hash: found?.sourceManifestHash || '',
      snapshot_package: `ghcr.io/${process.env.GITHUB_REPOSITORY_OWNER.toLowerCase()}/${SNAPSHOT_PACKAGE}` });
    console.log(`Cargo snapshot lookup=${enabled ? (found ? 'trusted-main-digest' : 'cold-fallback') : 'disabled-pending-net-gain'}`);
  } else if (mode === 'publisher-check') {
    const repository = process.env.GITHUB_REPOSITORY;
    const run = api(`/repos/${repository}/actions/runs/${process.env.GITHUB_RUN_ID}`);
    const prepared = JSON.parse(readFileSync(path.join(process.env.RUNNER_TEMP, 'backend-school.json'), 'utf8'));
    assertPublisher(run, repository, prepared);
    console.log('Cargo snapshot publisher matches this main Pipeline prepare job.');
  } else if (mode === 'publish') {
    const repository = process.env.GITHUB_REPOSITORY;
    const run = api(`/repos/${repository}/actions/runs/${process.env.GITHUB_RUN_ID}`);
    const prepared = JSON.parse(readFileSync(path.join(process.env.RUNNER_TEMP, 'backend-school.json'), 'utf8'));
    const metadata = JSON.parse(readFileSync(path.join(process.env.RUNNER_TEMP, 'cargo-snapshot-metadata/snapshot.json'), 'utf8'));
    const manifest = producerManifest({ repository, run, prepared, metadata, identity, digest: process.env.SNAPSHOT_DIGEST });
    writeFileSync(path.join(process.env.RUNNER_TEMP, `${SNAPSHOT_ARTIFACT}.json`), JSON.stringify(manifest) + '\n');
  } else throw new Error('Use resolve or publish');
}
