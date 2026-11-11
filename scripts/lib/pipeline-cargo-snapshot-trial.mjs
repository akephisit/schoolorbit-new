import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { decodeArtifact, artifactFromJobAttempt } from './pipeline-cargo-snapshot.mjs';

export const TRIAL_ARTIFACT = 'cargo-snapshot-trial-producer';
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const HEX = /^[0-9a-f]{64}$/;
const SHA = /^[0-9a-f]{40}$/;
const REPOSITORY = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;
const WORKFLOW = '.github/workflows/operations.yml';
const api = route => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 30_000, stdio: ['ignore', 'pipe', 'pipe'] }));

export function validateTrialOrigin(run, current, { producerRunId, producerAttempt, cleanup = false }) {
  if (!REPOSITORY.test(current.repository || '') || !/^[1-9][0-9]*$/.test(producerRunId) ||
      !Number.isSafeInteger(producerAttempt) || producerAttempt < 1 || String(run?.id) !== producerRunId ||
      run.run_attempt !== producerAttempt || !SHA.test(run.head_sha || '') || (!cleanup && run.head_sha !== current.sha) ||
      run.repository?.full_name !== current.repository || run.head_repository?.full_name !== current.repository ||
      (!cleanup && run.head_branch !== current.ref.replace(/^refs\/heads\//, '')) || run.event !== 'workflow_dispatch' ||
      run.path?.replace(/@refs\/heads\/[^@]+$/, '') !== WORKFLOW || run.status !== 'completed' ||
      (!cleanup && run.conclusion !== 'success') ||
      (cleanup && !['success', 'failure', 'cancelled', 'timed_out'].includes(run.conclusion)) || producerRunId === current.runId) {
    throw new Error('untrusted_trial_producer');
  }
  return `schoolorbit-cache-benchmark-${producerRunId}-${producerAttempt}`;
}

export function validateTrialManifest(value, run, current, now = Date.now()) {
  const producer = value?.producer;
  const packageName = validateTrialOrigin(run, current, { producerRunId: String(run.id), producerAttempt: run.run_attempt });
  const expected = `ghcr.io/${current.repository.split('/')[0].toLowerCase()}/${packageName}`;
  if (value.schemaVersion !== 1 || producer?.repository !== current.repository || producer.workflowPath !== WORKFLOW ||
      producer.runId !== String(run.id) || producer.runAttempt !== run.run_attempt || producer.sha !== run.head_sha ||
      producer.ref !== current.ref || producer.event !== 'workflow_dispatch' || value.registryPackage !== expected ||
      !DIGEST.test(value.images?.cargoSnapshot || '') || !DIGEST.test(value.images?.compilerCache || '') ||
      !DIGEST.test(value.images?.buildkitCache || '') || !HEX.test(value.identity?.buildConfigurationHash || '') ||
      !HEX.test(value.identity?.externalLockIdentityHash || '') || !HEX.test(value.sourceManifestHash || '') ||
      !HEX.test(value.compilerSnapshotHash || '') || !HEX.test(value.sourceContextHash || '') ||
      !/^.+@sha256:[0-9a-f]{64}$/.test(value.buildkitImage || '') || value.cpus !== 4 ||
      !Number.isFinite(Date.parse(run.updated_at)) || !Number.isFinite(Date.parse(value.expiresAt)) || Date.parse(value.expiresAt) <= now ||
      Date.parse(value.expiresAt) > Date.parse(run.updated_at) + 25 * 60 * 60 * 1000) {
    throw new Error('invalid_trial_manifest');
  }
  return value;
}

export function validateTrialPublisher(value, run, current, { job, attempt }) {
  const producer = value?.producer;
  if (String(run?.id) !== current.runId || run.run_attempt !== attempt || job !== 'benchmark' ||
      run.repository?.full_name !== current.repository || run.head_repository?.full_name !== current.repository ||
      run.head_sha !== current.sha || run.head_branch !== current.ref.replace(/^refs\/heads\//, '') ||
      run.event !== 'workflow_dispatch' || run.path?.replace(/@refs\/heads\/[^@]+$/, '') !== WORKFLOW ||
      producer?.runId !== current.runId || producer.runAttempt !== attempt || producer.sha !== current.sha ||
      producer.repository !== current.repository || producer.workflowPath !== WORKFLOW ||
      producer.ref !== current.ref || producer.event !== 'workflow_dispatch') {
    throw new Error('trial_publisher_origin');
  }
}

function context() {
  return { repository: process.env.GITHUB_REPOSITORY, sha: process.env.GITHUB_SHA,
    ref: process.env.GITHUB_REF, runId: process.env.GITHUB_RUN_ID };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const started = performance.now();
    const operation = process.argv[2];
    if (!['resolve', 'cleanup', 'publish'].includes(operation)) throw new Error('Use resolve, cleanup or publish');
    const current = context();
    if (!REPOSITORY.test(current.repository || '')) throw new Error('repository');
    if (operation === 'publish') {
      const value = JSON.parse(readFileSync(path.join(process.env.BENCH_OUTPUT, `${TRIAL_ARTIFACT}.json`), 'utf8'));
      const run = api(`/repos/${current.repository}/actions/runs/${current.runId}`);
      validateTrialPublisher(value, run, current, { job: process.env.GITHUB_JOB, attempt: Number(process.env.GITHUB_RUN_ATTEMPT) });
    } else {
      const producerRunId = process.env.PRODUCER_RUN_ID || '';
      const producerAttempt = Number(process.env.PRODUCER_RUN_ATTEMPT);
      if (!/^[1-9][0-9]*$/.test(producerRunId) || !Number.isSafeInteger(producerAttempt) || producerAttempt < 1) throw new Error('producer_identity');
      const run = api(`/repos/${current.repository}/actions/runs/${producerRunId}`);
      const name = validateTrialOrigin(run, current, { producerRunId, producerAttempt, cleanup: operation === 'cleanup' });
      // Cleanup can reclaim an expired/failed producer, but only this exact verified run-owned package.
      if (operation === 'resolve') {
        const artifacts = api(`/repos/${current.repository}/actions/runs/${producerRunId}/artifacts?per_page=100`).artifacts
          .filter(a => !a.expired && a.name === TRIAL_ARTIFACT && a.workflow_run?.id === run.id);
        if (artifacts.length !== 1) throw new Error('trial_artifact_inventory');
        const archive = execFileSync('gh', ['api', `/repos/${current.repository}/actions/artifacts/${artifacts[0].id}/zip`],
          { maxBuffer: 1024 * 1024, timeout: 30_000, stdio: ['ignore', 'pipe', 'pipe'] });
        const value = validateTrialManifest(decodeArtifact(archive, `${TRIAL_ARTIFACT}.json`), run, current);
        const jobs = api(`/repos/${current.repository}/actions/runs/${run.id}/attempts/${producerAttempt}/jobs?per_page=100`).jobs
          .filter(job => /(?:^| \/ )benchmark$/.test(job.name));
        if (jobs.length !== 1 || !artifactFromJobAttempt(artifacts[0], jobs[0], run, 'Publish cross-run Cargo snapshot locator')) throw new Error('trial_job_origin');
        const chargedNames = ['Validate trial producer authority', 'Publish cross-run Cargo snapshot locator'];
        const chargedSteps = chargedNames.map(name => {
          const matching = jobs[0].steps?.filter(step => step.name === name && step.conclusion === 'success') || [];
          if (matching.length !== 1) throw new Error('trial_publisher_step_evidence');
          const step = matching[0];
          const seconds = (Date.parse(step.completed_at) - Date.parse(step.started_at)) / 1000;
          if (!Number.isFinite(seconds) || seconds < 0) throw new Error('trial_publisher_step_duration');
          return { name, seconds: seconds + 1, observedStepSeconds: seconds, timestampResolutionPaddingSeconds: 1 };
        });
        value.orchestration = { producerSnapshotOnlySteps: chargedSteps,
          producerSnapshotOnlySeconds: chargedSteps.reduce((sum, step) => sum + step.seconds, 0),
          consumerResolveSeconds: (performance.now() - started) / 1000 };
        const target = path.join(process.env.RUNNER_TEMP, 'cargo-snapshot-trial-producer.json');
        writeFileSync(target, JSON.stringify(value) + '\n');
        // Include final file publication/readback in the actual lookup duration too.
        JSON.parse(readFileSync(target, 'utf8'));
        value.orchestration.consumerResolveSeconds = (performance.now() - started) / 1000;
        writeFileSync(target, JSON.stringify(value) + '\n');
      }
      appendFileSync(process.env.GITHUB_OUTPUT, `cleanup_package=${name}\n`);
    }
  } catch {
    // Never print gh download exceptions, signed URLs, archive bytes or credentials.
    console.error('Cargo snapshot trial authority validation failed.');
    process.exitCode = 1;
  }
}
