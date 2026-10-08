import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const requested = JSON.parse(process.env.REQUESTED_SUITES);
let reused = [];
const repo = process.env.GITHUB_REPOSITORY;
const api = (route) => JSON.parse(execFileSync('gh', ['api', route], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }));
if (process.env.GITHUB_REF === 'refs/heads/main' && process.env.CANDIDATE !== 'true') {
  const sha = process.env.GITHUB_SHA;
  const commit = api(`/repos/${repo}/git/commits/${sha}`);
  const prs = api(`/repos/${repo}/commits/${sha}/pulls?per_page=100`);
  const pr = prs.find((item) => item.merged_at && item.merge_commit_sha === sha && item.head.repo?.full_name === repo && item.base.ref === 'main');
  if (pr) {
    const permission = api(`/repos/${repo}/collaborators/${pr.user.login}/permission`).permission;
    if (['admin', 'maintain', 'write'].includes(permission)) {
      const checks = api(`/repos/${repo}/commits/${pr.head.sha}/check-runs?per_page=100`).check_runs;
      const gate = checks.filter((entry) => entry.name === 'Pipeline gate' && entry.app?.slug === 'github-actions').sort((a, b) => b.id - a.id)[0];
      const id = gate?.details_url?.match(/\/actions\/runs\/([0-9]+)/)?.[1];
      if (id && gate.conclusion === 'success') {
        const run = api(`/repos/${repo}/actions/runs/${id}`);
        if (run.path === '.github/workflows/pipeline.yml' && run.status === 'completed' && run.conclusion === 'success' && run.head_repository?.full_name === repo &&
          ((run.event === 'pull_request' && run.head_sha === pr.head.sha) || (run.event === 'workflow_dispatch' && run.head_branch === 'main'))) {
          const artifacts = api(`/repos/${repo}/actions/runs/${id}/artifacts?per_page=100`).artifacts;
          const artifact = artifacts.find((item) => item.name === 'pipeline-plan' && !item.expired);
          if (artifact) {
            const temp = mkdtempSync(path.join(os.tmpdir(), 'verification-proof-'));
            try {
              writeFileSync(path.join(temp, 'plan.zip'), execFileSync('gh', ['api', `/repos/${repo}/actions/artifacts/${artifact.id}/zip`], { maxBuffer: 1024 * 1024 }));
              execFileSync('python3', ['-c', 'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert z.namelist()==["plan.json"]; assert z.getinfo("plan.json").file_size<65536; open(sys.argv[2],"wb").write(z.read("plan.json"))', path.join(temp, 'plan.zip'), path.join(temp, 'plan.json')]);
              const plan = JSON.parse(readFileSync(path.join(temp, 'plan.json'), 'utf8'));
              if (plan.schemaVersion === 1 && plan.runId === String(run.id) && plan.attempt === run.run_attempt && plan.tree === commit.tree.sha && plan.base === commit.parents[0].sha && plan.prHead === pr.head.sha && Array.isArray(plan.verify)) {
                const pages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', `/repos/${repo}/actions/runs/${id}/attempts/${run.run_attempt}/jobs?per_page=100`], { encoding: 'utf8' }));
                const jobs = pages.flatMap((page) => page.jobs);
                reused = [];
                const docker = execFileSync('docker', ['version', '--format', '{{.Server.Version}}'], { encoding: 'utf8' }).trim();
                for (const suite of requested) {
                  // Main owns fresh DB acceptance and the mutable compiled-cache snapshot.
                  // Keep that one backend owner running; equivalent static/frontend receipts
                  // can skip duplicate checks without starving the shared compiler cache.
                  if (suite.startsWith('backend-')) continue;
                  if (!plan.verify.includes(suite) || jobs.filter((job) => job.name.endsWith(`/ Verify ${suite}`) && job.conclusion === 'success' && job.run_attempt === run.run_attempt).length !== 1) continue;
                  const receipt = artifacts.find((item) => item.name === `verification-${suite}` && !item.expired);
                  if (!receipt) continue;
                  writeFileSync(path.join(temp, 'receipt.zip'), execFileSync('gh', ['api', `/repos/${repo}/actions/artifacts/${receipt.id}/zip`], { maxBuffer: 1024 * 1024 }));
                  execFileSync('python3', ['-c', 'import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); assert z.namelist()==[sys.argv[3]]; assert z.getinfo(sys.argv[3]).file_size<65536; open(sys.argv[2],"wb").write(z.read(sys.argv[3]))', path.join(temp, 'receipt.zip'), path.join(temp, 'receipt.json'), `verification-${suite}.json`]);
                  const evidence = JSON.parse(readFileSync(path.join(temp, 'receipt.json'), 'utf8'));
                  if (evidence.schemaVersion !== 1 || evidence.runId !== String(run.id) || evidence.attempt !== run.run_attempt || evidence.suite !== suite || evidence.tree !== commit.tree.sha || evidence.node !== process.versions.node || !process.env.ImageVersion || evidence.runnerImage !== process.env.ImageVersion || evidence.runnerOS !== process.env.RUNNER_OS || evidence.docker !== docker || evidence.profile?.incremental !== '0' || evidence.profile?.devDebug !== '0' || evidence.profile?.testDebug !== '0' || (suite.startsWith('backend-') && !evidence.rust?.startsWith('rustc 1.98.1 '))) continue;
                  reused.push(suite);
                }
                const latest = api(`/repos/${repo}/actions/runs/${id}`);
                if (latest.run_attempt !== run.run_attempt || latest.conclusion !== 'success') reused = [];
              }
            } finally { rmSync(temp, { recursive: true, force: true }); }
          }
        }
      }
    }
  }
}
const remaining = requested.filter((suite) => !reused.includes(suite));
appendFileSync(process.env.GITHUB_OUTPUT, `remaining=${JSON.stringify(remaining)}\nreused=${JSON.stringify(reused)}\n`);
appendFileSync(process.env.GITHUB_STEP_SUMMARY, `Verified PR checks reused: ${reused.join(', ') || 'none'}. Checks to execute: ${remaining.join(', ') || 'none'}. Runtime readiness/smoke always execute.\n`);
