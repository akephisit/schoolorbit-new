import {execFileSync} from 'node:child_process';
import {pathToFileURL} from 'node:url';
import {setTimeout as pause} from 'node:timers/promises';
import {makePlan} from './pipeline-policy.mjs';
import {readAcceptedState} from './pipeline-state.mjs';

export function requireFrontendOnly(plan) {
  if (plan.deploy.length !== 1 || plan.deploy[0] !== 'frontend-admin') {
    throw new Error('Admin recreation requires an accepted baseline with only frontend-admin selected; finish queued releases first');
  }
}

export function preparedGate(jobs) {
  const owners = jobs.filter(job => job.name.startsWith('prepare / Prepare '));
  if (owners.some(job => job.name !== 'prepare / Prepare frontend-admin')) throw new Error('Recreation Pipeline selected another component');
  const gates = jobs.filter(job => job.name === 'Pipeline gate');
  if (gates.length > 1) throw new Error('Ambiguous Pipeline gate');
  if (!gates.length || gates[0].status !== 'completed') return false;
  if (gates[0].conclusion !== 'success' || owners.length !== 1 || owners[0].conclusion !== 'success') {
    throw new Error('Recreation Pipeline build gate did not pass');
  }
  return true;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.env.GITHUB_REF !== 'refs/heads/main' || process.env.GITHUB_EVENT_NAME !== 'workflow_dispatch') throw new Error('Recreation requires explicit Operations on main');
  const repository = process.env.GITHUB_REPOSITORY;
  const sha = process.env.GITHUB_SHA;
  const api = route => JSON.parse(execFileSync('gh', ['api', route], {encoding: 'utf8'}));
  const current = () => api(`repos/${repository}/git/ref/heads/main`).object.sha;
  if (current() !== sha) throw new Error('A newer main exists; no Worker was changed');
  const state = readAcceptedState();
  if (!state) throw new Error('An accepted release is required before Admin recreation');
  const publicConfig = {
    'frontend-school': {backend: process.env.PUBLIC_BACKEND_URL || '', vapid: process.env.PUBLIC_VAPID_KEY || ''},
    'frontend-admin': {backend: process.env.PUBLIC_API_URL || '', schoolBackend: process.env.BACKEND_SCHOOL_URL || ''}
  };
  requireFrontendOnly(makePlan({root: '.', head: sha, base: sha, state, scope: 'frontend-admin', publicConfig}));
  const request = `admin-worker-recreate-${process.env.GITHUB_RUN_ID}-${process.env.GITHUB_RUN_ATTEMPT}`;
  execFileSync('gh', ['workflow', 'run', 'pipeline.yml', '--ref', 'main', '-f', 'scope=frontend-admin', '-f', 'automatic=false', '-f', `deployment_id=${request}`], {stdio: 'inherit'});
  const deadline = Date.now() + 15 * 60 * 1000;
  let run;
  while (Date.now() < deadline) {
    run ??= api(`repos/${repository}/actions/workflows/pipeline.yml/runs?event=workflow_dispatch&branch=main&per_page=30`).workflow_runs
      .find(item => item.display_title === `Pipeline (${request})` && item.head_sha === sha && item.head_repository?.full_name === repository);
    if (run) {
      const jobs = api(`repos/${repository}/actions/runs/${run.id}/jobs?per_page=100`).jobs;
      if (preparedGate(jobs)) {
        if (current() !== sha) throw new Error('Main advanced during preparation; no Worker was changed');
        console.log(`Prepared frontend-only Pipeline ${run.html_url}; production release waits for this operation to release its lock.`);
        break;
      }
      if (api(`repos/${repository}/actions/runs/${run.id}`).status === 'completed') throw new Error('Recreation Pipeline ended before its build gate');
    }
    await pause(3000);
  }
  if (!run || Date.now() >= deadline) throw new Error('Timed out preparing Admin recreation; no Worker was changed');
}
