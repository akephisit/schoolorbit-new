import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const repo = process.env.GITHUB_REPOSITORY;
const api = (route, method = 'GET', data) => {
  const args = ['api', route, '--method', method];
  const options = { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 };
  if (data) { args.push('--input', '-'); options.input = JSON.stringify(data); }
  return JSON.parse(execFileSync('gh', args, options) || '{}');
};
const trusted = (pr) => pr.head.repo?.full_name === repo && pr.base.ref === 'main' && !pr.draft && ['admin', 'maintain', 'write'].includes(api(`/repos/${repo}/collaborators/${pr.user.login}/permission`).permission);
const prs = api(`/repos/${repo}/pulls?state=open&base=main&sort=created&direction=asc&per_page=100`);
for (const listed of prs) {
  const pr = api(`/repos/${repo}/pulls/${listed.number}`);
  if (!trusted(pr)) continue;
  const main = api(`/repos/${repo}/git/ref/heads/main`).object.sha;
  if (pr.mergeable_state === 'behind') {
    console.log(`PR #${pr.number} needs its developer to update from main and rerun affected local tests.`);
    continue;
  }
  if (!['clean', 'unstable'].includes(pr.mergeable_state)) continue;
  const checks = api(`/repos/${repo}/commits/${pr.head.sha}/check-runs?per_page=100`).check_runs;
  const gate = checks.filter((check) => check.name === 'Pipeline gate' && check.app?.slug === 'github-actions').sort((a, b) => b.id - a.id)[0];
  if (!gate || gate.conclusion !== 'success') continue;
  const details = gate.details_url?.match(/\/actions\/runs\/([0-9]+)/);
  if (!details) continue;
  const run = api(`/repos/${repo}/actions/runs/${details[1]}`);
  if (run.status !== 'completed' || run.conclusion !== 'success' || run.repository?.full_name !== repo || run.head_repository?.full_name !== repo || run.path !== '.github/workflows/pipeline.yml' || run.event !== 'pull_request' || run.head_sha !== pr.head.sha) continue;
  const artifacts = api(`/repos/${repo}/actions/runs/${run.id}/artifacts?per_page=100`).artifacts;
  const artifact = artifacts.find((entry) => entry.name === 'pipeline-plan' && !entry.expired);
  if (!artifact) continue;
  const temp = mkdtempSync(path.join(os.tmpdir(), 'merge-plan-'));
  let plan;
  try {
    writeFileSync(path.join(temp, 'plan.zip'), execFileSync('gh', ['api', `/repos/${repo}/actions/artifacts/${artifact.id}/zip`], { maxBuffer: 1024 * 1024 }));
    execFileSync('python3', ['-c', 'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert z.namelist()==["plan.json"]; assert z.getinfo("plan.json").file_size<65536; open(sys.argv[2],"wb").write(z.read("plan.json"))', path.join(temp, 'plan.zip'), path.join(temp, 'plan.json')]);
    plan = JSON.parse(readFileSync(path.join(temp, 'plan.json'), 'utf8'));
  } finally { rmSync(temp, { recursive: true, force: true }); }
  const jobPages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', `/repos/${repo}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`], { encoding: 'utf8' }));
  const gates = jobPages.flatMap(page => page.jobs).filter(job => job.name === 'Pipeline gate');
  if (gates.length !== 1 || gates[0].conclusion !== 'success' || gates[0].run_attempt !== run.run_attempt || gates[0].head_sha !== run.head_sha || plan.runId !== String(run.id) || plan.attempt !== run.run_attempt) continue;
  if (plan.base !== main || plan.prHead !== pr.head.sha || plan.tree !== api(`/repos/${repo}/git/commits/${pr.merge_commit_sha}`).tree.sha) {
    console.log(`PR #${pr.number} has a stale candidate; update and retest locally before merging.`);
    continue;
  }
  if (api(`/repos/${repo}/git/ref/heads/main`).object.sha !== main) break;
  api(`/repos/${repo}/pulls/${pr.number}/merge`, 'PUT', { sha: pr.head.sha, merge_method: 'squash' });
  // A GITHUB_TOKEN merge suppresses push workflows; dispatch the production pipeline
  // explicitly. It resolves accumulated changes against accepted baselines.
  api(`/repos/${repo}/actions/workflows/pipeline.yml/dispatches`, 'POST', { ref: 'main', inputs: { scope: 'auto', automatic: 'true' } });
  console.log(`Squash-merged PR #${pr.number} after the latest-main Pipeline gate.`);
  break;
}
