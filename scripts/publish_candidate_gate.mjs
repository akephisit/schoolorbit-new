import { execFileSync } from 'node:child_process';

const api = (route, method = 'GET', body) => JSON.parse(execFileSync('gh', ['api', route, '--method', method, ...(body ? ['--input', '-'] : [])], { encoding: 'utf8', ...(body ? { input: JSON.stringify(body) } : {}) }) || '{}');
const repo = process.env.GITHUB_REPOSITORY;
const plan = JSON.parse(process.env.PLAN);
const number = process.env.PR_NUMBER;
if (!/^[1-9][0-9]*$/.test(number || '')) throw new Error('Invalid PR number');
const pr = api(`/repos/${repo}/pulls/${number}`);
const main = api(`/repos/${repo}/git/ref/heads/main`).object.sha;
const passing = process.env.GATE_RESULT === 'success' && pr.head.repo?.full_name === repo && pr.head.sha === plan.prHead &&
  main === plan.base && api(`/repos/${repo}/git/commits/${pr.merge_commit_sha}`).tree.sha === plan.tree;
api(`/repos/${repo}/check-runs`, 'POST', {
  name: 'Pipeline gate', head_sha: pr.head.sha, status: 'completed', conclusion: passing ? 'success' : 'failure',
  details_url: `https://github.com/${repo}/actions/runs/${process.env.GITHUB_RUN_ID}`,
  output: { title: passing ? 'Latest-main candidate passed' : 'Candidate verification failed or became stale', summary: `Candidate base ${plan.base}; current main ${main}.` }
});
if (!passing) throw new Error('The required PR gate cannot pass for a stale or failed candidate');
