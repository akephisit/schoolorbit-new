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
const runId = 123456;
const workflowId = 98765;
const repository = "example/schoolorbit";
let nextJobId = 1;

function job({ attempt, name, conclusion, headSha = releaseId, id }) {
  const jobId = id ?? nextJobId++;
  return {
    id: jobId,
    run_id: runId,
    run_url: `https://api.github.com/repos/${repository}/actions/runs/${runId}`,
    node_id: `job-${jobId}`,
    head_sha: headSha,
    url: `https://api.github.com/repos/${repository}/actions/jobs/${jobId}`,
    html_url: `https://github.com/${repository}/actions/runs/${runId}/job/${jobId}`,
    status: conclusion === null ? "in_progress" : "completed",
    conclusion,
    created_at: "2026-09-15T10:00:00Z",
    started_at: "2026-09-15T10:00:01Z",
    completed_at: conclusion === null ? null : "2026-09-15T10:00:02Z",
    name,
    steps: [],
    check_run_url: `https://api.github.com/repos/${repository}/check-runs/${jobId}`,
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

function workflowRun({
  id = runId,
  createdAt = "2026-09-15T10:00:00Z",
  conclusion = null,
} = {}) {
  return {
    id,
    name: "Deploy School Release",
    node_id: `run-${id}`,
    head_branch: "main",
    head_sha: id === runId ? releaseId : "b".repeat(40),
    path: ".github/workflows/deploy-school-release.yml",
    display_title: "Deploy School Release",
    run_number: id,
    event: "push",
    status: conclusion === null ? "in_progress" : "completed",
    conclusion,
    workflow_id: workflowId,
    check_suite_id: id,
    check_suite_node_id: `suite-${id}`,
    url: `https://api.github.com/repos/${repository}/actions/runs/${id}`,
    html_url: `https://github.com/${repository}/actions/runs/${id}`,
    created_at: createdAt,
    updated_at: createdAt,
    actor: { login: "release-operator" },
    run_attempt: id === runId ? 2 : 1,
    referenced_workflows: [],
    run_started_at: createdAt,
    triggering_actor: { login: "release-operator" },
    jobs_url: `https://api.github.com/repos/${repository}/actions/runs/${id}/jobs`,
    logs_url: `https://api.github.com/repos/${repository}/actions/runs/${id}/logs`,
    check_suite_url: `https://api.github.com/repos/${repository}/check-suites/${id}`,
    artifacts_url: `https://api.github.com/repos/${repository}/actions/runs/${id}/artifacts`,
    cancel_url: `https://api.github.com/repos/${repository}/actions/runs/${id}/cancel`,
    rerun_url: `https://api.github.com/repos/${repository}/actions/runs/${id}/rerun`,
    previous_attempt_url: null,
    workflow_url: `https://api.github.com/repos/${repository}/actions/workflows/${workflowId}`,
    head_commit: null,
    repository: { full_name: repository },
    head_repository: { full_name: repository },
  };
}

function page(collectionName, items, totalCount = items.length) {
  return { total_count: totalCount, [collectionName]: items };
}

async function selectReplay(currentAttempt, jobs, options = {}) {
  const currentRun = options.currentRun ?? {
    ...workflowRun(),
    run_attempt: currentAttempt,
  };
  const jobPages = options.jobPages ?? [page("jobs", jobs)];
  const runPages = options.runPages ?? [page("workflow_runs", [currentRun])];
  const root = await mkdtemp(path.join(tmpdir(), "school-release-replay-"));
  const currentRunPath = path.join(root, "current-run.json");
  const jobsPath = path.join(root, "job-pages.json");
  const runsPath = path.join(root, "run-pages.json");
  await Promise.all([
    writeFile(currentRunPath, JSON.stringify(currentRun)),
    writeFile(jobsPath, JSON.stringify(jobPages)),
    writeFile(runsPath, JSON.stringify(runPages)),
  ]);

  return spawnSync(
    process.execPath,
    [
      selector,
      String(currentAttempt),
      releaseId,
      currentRunPath,
      jobsPath,
      runsPath,
    ],
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
      "restore_state=true",
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
  assert.match(result.stdout, /^accepted_attempt=2$/m);
  assert.match(result.stdout, /^accepted_scope=frontend$/m);
  assert.match(result.stdout, /^accepted_frontend=true$/m);
  assert.match(result.stdout, /^accepted_backend=false$/m);
});

test("a backend deployment retained from an earlier attempt is accepted", async () => {
  const result = await selectReplay(3, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "accept-release", conclusion: "failure" }),
    job({ attempt: 2, name: "accept-release", conclusion: "success" }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^accepted_attempt=2$/m);
  assert.match(result.stdout, /^accepted_scope=backend$/m);
  assert.match(result.stdout, /^accepted_frontend=false$/m);
  assert.match(result.stdout, /^accepted_backend=true$/m);
});

test("a full release can be accepted across partial job retry attempts", async () => {
  const result = await selectReplay(3, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({
      attempt: 1,
      name: "promote-frontends (sandbox)",
      conclusion: "failure",
    }),
    job({ attempt: 1, name: "accept-release", conclusion: "skipped" }),
    job({
      attempt: 2,
      name: "promote-frontends (sandbox)",
      conclusion: "success",
    }),
    job({ attempt: 2, name: "accept-release", conclusion: "success" }),
  ]);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^accepted_attempt=2$/m);
  assert.match(result.stdout, /^accepted_scope=full$/m);
  assert.match(result.stdout, /^accepted_frontend=true$/m);
  assert.match(result.stdout, /^accepted_backend=true$/m);
});

for (const conclusion of ["success", "failure"]) {
  test(`an older rerun does not replace state after a newer ${conclusion} run`, async () => {
    const newerRun = workflowRun({
      id: runId + 1,
      createdAt: "2026-09-15T11:00:00Z",
      conclusion,
    });
    const result = await selectReplay(
      2,
      [
        job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
        job({ attempt: 1, name: "accept-release", conclusion: "success" }),
      ],
      {
        runPages: [page("workflow_runs", [newerRun, workflowRun()])],
      },
    );

    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /^already_deployed=true$/m);
    assert.match(result.stdout, /^restore_state=false$/m);
  });
}

test("failed or incomplete attempts continue through normal recovery", async () => {
  const result = await selectReplay(3, [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "accept-release", conclusion: "failure" }),
    job({
      attempt: 2,
      name: "accept-release",
      conclusion: "skipped",
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
      "restore_state=false",
      "",
    ].join("\n"),
  );
});

test("all paginated jobs participate in replay selection", async () => {
  const jobs = [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "accept-release", conclusion: "success" }),
    job({ attempt: 2, name: "resolve-scope", conclusion: null }),
  ];
  const result = await selectReplay(2, jobs, {
    jobPages: [
      page("jobs", jobs.slice(0, 1), jobs.length),
      page("jobs", jobs.slice(1), jobs.length),
    ],
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^already_deployed=true$/m);
});

test("an incomplete job inventory fails closed", async () => {
  const jobs = [
    job({ attempt: 1, name: "deploy-backend", conclusion: "success" }),
    job({ attempt: 1, name: "accept-release", conclusion: "success" }),
  ];
  const result = await selectReplay(2, jobs, {
    jobPages: [page("jobs", jobs, jobs.length + 1)],
  });

  assert.equal(result.status, 64);
  assert.match(result.stderr, /job inventory is incomplete/);
});

test("duplicate job evidence fails closed", async () => {
  const duplicateId = 99;
  const jobs = [
    job({
      id: duplicateId,
      attempt: 1,
      name: "deploy-backend",
      conclusion: "success",
    }),
    job({
      id: duplicateId,
      attempt: 1,
      name: "accept-release",
      conclusion: "success",
    }),
  ];
  const result = await selectReplay(2, jobs);

  assert.equal(result.status, 64);
  assert.match(result.stderr, /duplicate job IDs/);
});
