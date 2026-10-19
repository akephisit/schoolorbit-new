import {execFileSync} from 'node:child_process';
import {appendFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {git, makePlan} from './pipeline-policy.mjs';
import {readAcceptedState, trustedRun} from './pipeline-state.mjs';

export function requirePreparedTlsGate(jobs, sha, attempt) {
  const gate = jobs.filter(job => job.name === 'Pipeline gate');
  const plan = jobs.filter(job => job.name === 'plan');
  const builds = jobs.filter(job => job.name.startsWith('prepare / Prepare '));
  if (gate.length !== 1 || plan.length !== 1 || [...gate, ...plan, ...builds].some(job =>
    job.conclusion !== 'success' || job.head_sha !== sha || job.run_attempt !== attempt)) {
    throw new Error('Latest exact-attempt Pipeline planning and selected build gate must pass before TLS changes');
  }
}

export function requireTlsRuntimePlan(plan, mode) {
  if (!['renew', 'migrate-origin-ca', 'dns-only', 'dns-rollback'].includes(mode)) throw new Error('Unknown TLS operation');
  if (mode !== 'migrate-origin-ca' && plan.deploy.length) {
    throw new Error('Queued runtime changes require the normal Pipeline before certificate renewal');
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const repo = process.env.GITHUB_REPOSITORY;
  const sha = process.env.GITHUB_SHA;
  const mode = process.env.API_TLS_MODE;
  if (process.env.GITHUB_REF !== 'refs/heads/main' || !/^[\w.-]+\/[\w.-]+$/.test(repo || '') ||
      sha !== git('.', 'rev-parse', 'HEAD')) throw new Error('TLS operations require exact main checkout');
  if (mode !== 'renew' && process.env.GITHUB_EVENT_NAME !== 'workflow_dispatch') {
    throw new Error('Migration and DNS cutover require an explicit operation');
  }
  const api = route => JSON.parse(execFileSync('gh', ['api', route], {encoding: 'utf8'}));
  if (api(`repos/${repo}/git/ref/heads/main`).object.sha !== sha) throw new Error('A newer main exists');
  const runs = api(`repos/${repo}/actions/workflows/pipeline.yml/runs?head_sha=${sha}&per_page=100`).workflow_runs;
  const run = runs.filter(item => trustedRun(item, repo, 'pipeline.yml', sha, true)).sort((a, b) => b.id - a.id)[0];
  if (!run) throw new Error('No prepared Pipeline exists for this exact main SHA');
  const pages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp',
    `repos/${repo}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`], {encoding: 'utf8'}));
  requirePreparedTlsGate(pages.flatMap(page => page.jobs), sha, run.run_attempt);
  const state = readAcceptedState();
  if (!state || git('.', 'merge-base', state.sha, sha) !== state.sha) throw new Error('Trusted accepted application release must be an ancestor of main');
  const publicConfig = {
    'frontend-school': {backend: process.env.PUBLIC_BACKEND_URL || '', vapid: process.env.PUBLIC_VAPID_KEY || ''},
    'frontend-admin': {backend: process.env.PUBLIC_API_URL || '', schoolBackend: process.env.BACKEND_SCHOOL_URL || ''}
  };
  requireTlsRuntimePlan(makePlan({root: '.', head: sha, base: sha, state, scope: 'auto', publicConfig}), mode);
  appendFileSync(process.env.GITHUB_OUTPUT, `accepted_app_sha=${state.sha}\n`);
  console.log('Current main, latest prepared gate and trusted accepted application identity verified.');
}
