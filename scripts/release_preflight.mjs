import { execFileSync } from 'node:child_process';
import { readAcceptedState } from './lib/pipeline-state.mjs';
import { git, makePlan } from './lib/pipeline-policy.mjs';

const api = (route) => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8' }));
const plan = JSON.parse(process.env.PLAN);
const repo = process.env.GITHUB_REPOSITORY;
const sha = process.env.GITHUB_SHA;
if (process.env.GITHUB_EVENT_NAME === 'push' || process.env.AUTOMATIC_DISPATCH === 'true') {
  if (plan.deploy.some((part) => part.startsWith('backend-')) && process.env.RUNTIME_DEPLOY_ENABLED !== 'true') throw new Error('RUNTIME_DEPLOY_ENABLED must allow this automatic release');
  if (plan.deploy.some((part) => part.startsWith('frontend-')) && process.env.FRONTEND_DEPLOY_ENABLED !== 'true') throw new Error('FRONTEND_DEPLOY_ENABLED must allow this automatic release');
}
if (process.env.GITHUB_REF !== 'refs/heads/main' || plan.sha !== sha || plan.tree !== git('.', 'rev-parse', 'HEAD^{tree}')) throw new Error('Production candidate identity mismatch');
if (api(`/repos/${repo}/git/ref/heads/main`).object.sha !== sha) throw new Error('A newer main exists; this stale candidate cannot change production');
const run = api(`/repos/${repo}/actions/runs/${process.env.GITHUB_RUN_ID}`);
if (run.path !== '.github/workflows/pipeline.yml' || run.head_sha !== sha || run.run_attempt !== Number(process.env.GITHUB_RUN_ATTEMPT) || run.head_repository.full_name !== repo || run.head_branch !== 'main' || !['push', 'workflow_dispatch'].includes(run.event)) throw new Error('Untrusted production caller');
const pages = execFileSync('gh', ['api', '--paginate', '--slurp', `/repos/${repo}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`], { encoding: 'utf8' });
const jobs = JSON.parse(pages).flatMap((page) => page.jobs);
const gates = jobs.filter((job) => job.name === 'Pipeline gate');
if (gates.length !== 1 || gates[0].conclusion !== 'success' || gates[0].head_sha !== sha || gates[0].run_attempt !== run.run_attempt) throw new Error('Exact-attempt Pipeline gate has not passed');
const config = {
  'frontend-school': { backend: process.env.PUBLIC_BACKEND_URL || '', vapid: process.env.PUBLIC_VAPID_KEY || '' },
  'frontend-admin': { backend: process.env.PUBLIC_API_URL || '', schoolBackend: process.env.BACKEND_SCHOOL_URL || '' }
};
const fresh = makePlan({ root: '.', head: sha, base: plan.base, state: readAcceptedState(), publicConfig: config });
if (fresh.deploy.some((part) => !plan.deploy.includes(part)) || fresh.verify.some((suite) => !plan.verify.includes(suite)) || Object.keys(plan.hashes).some((part) => fresh.hashes[part] !== plan.hashes[part])) throw new Error('Production baselines or inputs changed; replan and verify before maintenance');
console.log('Current main, selected inputs and exact-attempt CI gate verified under production lock.');
