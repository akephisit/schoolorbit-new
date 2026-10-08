import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { components } from './pipeline-policy.mjs';

export function trustedRun(run, repository, workflow, sha, acceptedAttempt = false) {
  return run.repository?.full_name === repository && run.head_repository?.full_name === repository &&
    run.head_branch === 'main' && ['push', 'workflow_dispatch'].includes(run.event) &&
    run.path?.replace(/@(?:refs\/heads\/)?main$/, '') === `.github/workflows/${workflow}` &&
    (run.conclusion === 'success' || acceptedAttempt) && /^[0-9a-f]{40}$/.test(run.head_sha) && (!sha || run.head_sha === sha);
}

export function validateState(state, run) {
  if (state.schemaVersion !== 2 || state.runId !== String(run.id) || state.sha !== run.head_sha || !state.components) throw new Error('Invalid accepted pipeline state');
  for (const part of components) {
    const entry = state.components[part];
    if (!entry || !/^[0-9a-f]{40}$/.test(entry.sha) || !/^[0-9a-f]{64}$/.test(entry.inputHash)) throw new Error(`Invalid accepted ${part} baseline`);
    if (part.startsWith('backend-') && !/^sha256:[0-9a-f]{64}$/.test(entry.digest)) throw new Error('Invalid accepted OCI digest');
    const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i;
    if (part === 'frontend-admin' && !uuid.test(entry.versionId || '')) throw new Error('Invalid accepted Admin Worker');
    if (part === 'frontend-school' && (!entry.workers || Object.keys(entry.workers).length === 0 || Object.entries(entry.workers).some(([tenant,id]) => !/^[a-z0-9][a-z0-9-]*$/.test(tenant) || !uuid.test(id)))) throw new Error('Invalid accepted School Workers');
    if (part.startsWith('frontend-') && (!/^[0-9a-f]{64}$/.test(entry.bundleDigest || '') || !Number.isSafeInteger(entry.artifactRunId) || entry.artifactRunId < 1)) throw new Error('Invalid frontend artifact provenance');
  }
  return state;
}

const api = (route) => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }));

export function readAcceptedState(repository = process.env.GITHUB_REPOSITORY) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository || '')) throw new Error('Repository is required');
  const list = api(`/repos/${repository}/actions/artifacts?name=pipeline-state&per_page=100`);
  for (const artifact of list.artifacts.filter((item) => !item.expired).sort((a, b) => b.id - a.id)) {
    const run = api(`/repos/${repository}/actions/runs/${artifact.workflow_run.id}`);
    if (!trustedRun(run, repository, 'pipeline.yml', null, true)) continue;
    const temp = mkdtempSync(path.join(os.tmpdir(), 'pipeline-state-'));
    try {
      const archive = execFileSync('gh', ['api', `/repos/${repository}/actions/artifacts/${artifact.id}/zip`], { maxBuffer: 1024 * 1024 });
      writeFileSync(path.join(temp, 'state.zip'), archive);
      execFileSync('python3', ['-c', 'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert z.namelist()==["pipeline-state.json"]; assert z.getinfo("pipeline-state.json").file_size<65536; open(sys.argv[2],"wb").write(z.read("pipeline-state.json"))', path.join(temp, 'state.zip'), path.join(temp, 'state.json')]);
      const state = validateState(JSON.parse(readFileSync(path.join(temp, 'state.json'), 'utf8')), run);
      if (!Number.isSafeInteger(state.attempt) || state.attempt < 1 || state.attempt > run.run_attempt) throw new Error('Invalid accepted attempt');
      const pages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', `/repos/${repository}/actions/runs/${run.id}/attempts/${state.attempt}/jobs?per_page=100`], { encoding: 'utf8' }));
      const acceptance = pages.flatMap((page) => page.jobs).filter((job) => /(?:^| \/ )accept$/.test(job.name));
      if (acceptance.length !== 1 || acceptance[0].conclusion !== 'success' || acceptance[0].head_sha !== state.sha) continue;
      return state;
    } finally { rmSync(temp, { recursive: true, force: true }); }
  }
  return null;
}

if (process.argv[1]?.endsWith('/pipeline-state.mjs')) {
  try { process.stdout.write(JSON.stringify(readAcceptedState())); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
