import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function validateCutoverGate({ repo, sha, ref, runtimeEnabled, frontendEnabled, main, runs, jobs }) {
  if (ref !== 'refs/heads/main' || main.object.sha !== sha) throw new Error('Cutover candidate is not current main');
  if (runtimeEnabled !== 'false' || frontendEnabled !== 'false') throw new Error('Pause automatic runtime/frontend releases before database cutover');
  const run = runs.workflow_runs[0];
  if (!run || run.path !== '.github/workflows/pipeline.yml' || run.head_sha !== sha || run.head_branch !== 'main' ||
      run.head_repository.full_name !== repo || !['push', 'workflow_dispatch'].includes(run.event)) throw new Error('Trusted current-main Pipeline run unavailable');
  const gates = jobs.filter((job) => job.name === 'Pipeline gate');
  if (gates.length !== 1 || gates[0].conclusion !== 'success' || gates[0].head_sha !== sha || gates[0].run_attempt !== run.run_attempt) throw new Error('Exact-attempt Pipeline gate has not passed');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  const api = (route) => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8' }));
  const repo = process.env.GITHUB_REPOSITORY;
  const sha = process.env.GITHUB_SHA;
  const main = api(`/repos/${repo}/git/ref/heads/main`);
  const runs = api(`/repos/${repo}/actions/workflows/pipeline.yml/runs?branch=main&head_sha=${sha}&per_page=1`);
  const run = runs.workflow_runs[0];
  const pages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', `/repos/${repo}/actions/runs/${run?.id}/attempts/${run?.run_attempt}/jobs?per_page=100`], { encoding: 'utf8' }));
  validateCutoverGate({ repo, sha, ref: process.env.GITHUB_REF, runtimeEnabled: process.env.RUNTIME_DEPLOY_ENABLED,
    frontendEnabled: process.env.FRONTEND_DEPLOY_ENABLED, main, runs, jobs: pages.flatMap((page) => page.jobs) });
  console.log('Current main and selected build gate verified; automatic releases are paused.');
}
