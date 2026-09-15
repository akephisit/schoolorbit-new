import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const selector = path.resolve(
  import.meta.dirname,
  "../resolve_school_release_replay.mjs",
);
const releaseId = "a".repeat(40);

function job({ attempt, name, conclusion, headSha = releaseId }) {
  return {
    id: attempt * 1000 + name.length,
    run_id: 123456,
    run_url: "https://api.github.com/repos/example/schoolorbit/actions/runs/123456",
    node_id: `job-${attempt}-${name}`,
    head_sha: headSha,
    url: `https://api.github.com/repos/example/schoolorbit/actions/jobs/${attempt}`,
    html_url: `https://github.com/example/schoolorbit/actions/runs/123456/job/${attempt}`,
    status: "completed",
    conclusion,
    created_at: "2026-09-15T00:00:00Z",
    started_at: "2026-09-15T00:00:01Z",
    completed_at: "2026-09-15T00:00:02Z",
    name,
    steps: [],
    check_run_url: `https://api.github.com/repos/example/schoolorbit/check-runs/${attempt}`,
    labels: ["ubuntu-latest"],
    runner_id: 1,
    runner_name: "GitHub Actions 1",
    runner_group_id: 1,
    runner_group_name: "GitHub Actions",
    run_attempt: attempt,
    workflow_name: "Deploy School Release",
    head_branch: "main",
  };
}

async function selectReplay(currentAttempt, jobs) {
  const root = await mkdtemp(path.join(tmpdir(), "school-release-replay-"));
  const jobsPath = path.join(root, "jobs.json");
  await writeFile(jobsPath, JSON.stringify(jobs));

  return spawnSync(
    process.execPath,
    [selector, String(currentAttempt), releaseId, jobsPath],
    { encoding: "utf8" },
  );
}

test("a rerun of an accepted full release becomes a no-op", async () => {
  const result = await selectReplay(2, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({
      attempt: 1,
      name: "promote-frontends (sandbox)",
      conclusion: "success",
    }),
    job({ attempt: 1, name: "accept-release", conclusion: "success" }),
    job({ attempt: 2, name: "resolve-scope", conclusion: null }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    [
      "already_deployed=true",
      "accepted_attempt=1",
      "accepted_scope=full",
      "accepted_frontend=true",
      "accepted_backend=true",
      "",
    ].join("\n"),
  );
});

test("a replay preserves the component scope of the accepted attempt", async () => {
  const result = await selectReplay(3, [
    job({
      attempt: 1,
      name: "promote-frontends (sandbox)",
      conclusion: "success",
    }),
    job({ attempt: 1, name: "deploy-backend", conclusion: "skipped" }),
    job({ attempt: 1, name: "accept-release", conclusion: "success" }),
    job({ attempt: 2, name: "deploy-backend", conclusion: "skipped" }),
    job({ attempt: 2, name: "promote-frontends", conclusion: "skipped" }),
    job({ attempt: 2, name: "accept-release", conclusion: "success" }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^already_deployed=true$/m);
  assert.match(result.stdout, /^accepted_attempt=1$/m);
  assert.match(result.stdout, /^accepted_scope=frontend$/m);
  assert.match(result.stdout, /^accepted_frontend=true$/m);
  assert.match(result.stdout, /^accepted_backend=false$/m);
});

test("a backend-only accepted attempt is identified without frontend promotion", async () => {
  const result = await selectReplay(2, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "promote-frontends", conclusion: "skipped" }),
    job({ attempt: 1, name: "accept-release", conclusion: "success" }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^accepted_scope=backend$/m);
  assert.match(result.stdout, /^accepted_frontend=false$/m);
  assert.match(result.stdout, /^accepted_backend=true$/m);
});

test("failed or incomplete attempts continue through normal recovery", async () => {
  const result = await selectReplay(3, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "accept-release", conclusion: "failure" }),
    job({
      attempt: 2,
      name: "accept-release",
      conclusion: "skipped",
      headSha: "b".repeat(40),
    }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    [
      "already_deployed=false",
      "accepted_attempt=",
      "accepted_scope=",
      "accepted_frontend=false",
      "accepted_backend=false",
      "",
    ].join("\n"),
  );
});
