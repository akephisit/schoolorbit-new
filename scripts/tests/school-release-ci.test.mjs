import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import {
  latestTrustedRun,
  matchesPaths,
  requiredPushChecks,
  requireSuccessfulJobs,
  waitForReleaseCi,
} from "../verify_school_release_ci.mjs";

const repository = "example/schoolorbit";
const sha = "a".repeat(40);
const check = {
  file: "api-contract.yml",
  workflowId: 10,
  jobs: ["backend", "frontend"],
};

test("every effective release trigger is covered by mandatory CI filters", async () => {
  const tools = createRequire(
    new URL("../../frontend-school/package.json", import.meta.url),
  );
  const release = tools("yaml").parse(
    await readFile(
      new URL(
        "../../.github/workflows/deploy-school-release.yml",
        import.meta.url,
      ),
      "utf8",
    ),
  );
  for (const pattern of release.on.push.paths.filter(
    (p) => !p.startsWith("!"),
  )) {
    const sample = pattern
      .replaceAll("**", "coverage/probe")
      .replaceAll("*", "probe");
    if (matchesPaths([sample], release.on.push.paths)) {
      const checks = await requiredPushChecks([sample]);
      assert.ok(
        checks.some((c) => c.file === "api-contract.yml"),
        sample,
      );
      assert.ok(
        checks.some((c) => c.file === "permission-contract.yml"),
        sample,
      );
    }
  }
});
function run(overrides = {}) {
  return {
    id: 100,
    workflow_id: 10,
    path: ".github/workflows/api-contract.yml",
    run_attempt: 1,
    head_sha: sha,
    head_branch: "main",
    event: "push",
    repository: { full_name: repository },
    head_repository: { full_name: repository },
    status: "completed",
    conclusion: "success",
    ...overrides,
  };
}
function jobs(r = run()) {
  return check.jobs.map((name) => ({
    name,
    run_id: r.id,
    head_sha: r.head_sha,
    run_attempt: r.run_attempt,
    status: "completed",
    conclusion: "success",
  }));
}
function fixture({
  runs = [run()],
  jobList,
  current,
  intercept,
  ...options
} = {}) {
  let time = 0;
  let cycle = 0;
  const calls = [];
  return {
    calls,
    config: {
      repository,
      sha,
      checks: [check],
      now: () => time,
      sleep: async (ms) => {
        time += ms;
        cycle++;
      },
      log: () => {},
      intervalMs: 10,
      timeoutMs: 40,
      discoveryMs: 20,
      request: async (endpoint) => {
        calls.push(endpoint);
        const result = intercept?.(endpoint);
        if (result !== undefined) return result;
        if (endpoint.endsWith("/workflows/api-contract.yml"))
          return { id: 10, path: ".github/workflows/api-contract.yml" };
        const list = typeof runs === "function" ? runs(cycle) : runs;
        if (endpoint.includes("/workflows/10/runs?"))
          return { total_count: list.length, workflow_runs: list };
        if (endpoint.includes("/jobs?"))
          return {
            total_count: (jobList ?? jobs(list[0])).length,
            jobs: jobList ?? jobs(list[0]),
          };
        if (/\/runs\/\d+$/.test(endpoint)) return current?.(cycle) ?? list[0];
        throw new Error(`Unexpected endpoint ${endpoint}`);
      },
      ...options,
    },
  };
}

test("path matching handles deletions, ordered exclusions and directory wildcards", () => {
  assert.equal(
    matchesPaths(
      ["frontend-school/src/routes/+page.svelte"],
      ["frontend-school/**"],
    ),
    true,
  );
  assert.equal(matchesPaths(["removed/runtime.yml"], ["**/*.yml"]), true);
  assert.equal(matchesPaths(["root.yml"], ["**/*.yml"]), true);
  assert.equal(
    matchesPaths(
      ["backend-school/README.md"],
      ["backend-school/**", "!**/*.md"],
    ),
    false,
  );
  assert.equal(
    matchesPaths(["docs/TESTING.md"], ["**", "!**/*.md", "docs/TESTING.md"]),
    true,
  );
  assert.throws(() => matchesPaths(["x"], ["[xy]"]), /Unsupported/);
});

for (const file of [
  "frontend-school/src/routes/example/+page.svelte",
  "backend-school/src/middleware/session.rs",
  "backend-school/crates/new-owner/src/lib.rs",
  "backend-school/scripts/build_release.sh",
]) {
  test(`runtime release input receives both mandatory workflows: ${file}`, async () => {
    assert.deepEqual(
      (await requiredPushChecks([file])).map((c) => c.file),
      ["api-contract.yml", "permission-contract.yml"],
    );
  });
}
for (const file of [
  "podman-compose.yml",
  "nginx-configs/school-api.conf.template",
  ".github/workflows/deploy-school-release.yml",
  "scripts/verify_school_release_ci.mjs",
]) {
  test(`infrastructure release also requires Installer: ${file}`, async () => {
    assert.deepEqual(
      (await requiredPushChecks([file])).map((c) => c.file),
      ["api-contract.yml", "permission-contract.yml", "installer.yml"],
    );
  });
}
test("missing mandatory trigger coverage is rejected rather than treated as passing", async () => {
  await assert.rejects(
    requiredPushChecks(
      ["frontend-school/src/new.ts"],
      async () =>
        "on:\n  push:\n    branches: [main]\n    paths: [unrelated/**]\njobs:\n  test: {}\n",
    ),
    /not covered/,
  );
});

test("latest run wins even when a previous run succeeded", () => {
  assert.equal(
    latestTrustedRun([run(), run({ id: 101, conclusion: "failure" })], {
      repository,
      sha,
      ...check,
    }).conclusion,
    "failure",
  );
});
for (const override of [
  { head_sha: "b".repeat(40) },
  { head_branch: "feature" },
  { event: "pull_request" },
  { workflow_id: 99 },
  { path: ".github/workflows/other.yml" },
  { head_repository: { full_name: "fork/schoolorbit" } },
  { repository: { full_name: "other/repo" } },
]) {
  test(`untrusted or unrelated CI cannot authorize deployment: ${JSON.stringify(override)}`, () => {
    assert.equal(
      latestTrustedRun([run(override)], { repository, sha, ...check }),
      undefined,
    );
  });
}
test("zero or invalid attempts cannot authorize deployment", () => {
  assert.throws(
    () =>
      latestTrustedRun([run({ run_attempt: 0 })], {
        repository,
        sha,
        ...check,
      }),
    /identity/,
  );
});
for (const override of [
  { conclusion: "failure" },
  { conclusion: "skipped" },
  { run_attempt: 2 },
  { head_sha: "b".repeat(40) },
  { run_id: 101 },
]) {
  test(`required jobs must match success, SHA and attempt: ${JSON.stringify(override)}`, () => {
    const list = jobs();
    list[0] = { ...list[0], ...override };
    assert.throws(
      () => requireSuccessfulJobs(list, run(), check.jobs),
      /did not succeed/,
    );
  });
}
test("missing and duplicate required jobs are rejected", () => {
  assert.throws(
    () => requireSuccessfulJobs(jobs().slice(1), run(), check.jobs),
    /did not succeed/,
  );
  assert.throws(
    () => requireSuccessfulJobs([...jobs(), jobs()[0]], run(), check.jobs),
    /did not succeed/,
  );
});
test("successful CI validates all jobs on the exact attempt", async () => {
  const f = fixture();
  await waitForReleaseCi(f.config);
  assert.ok(f.calls.some((p) => p.includes("/attempts/1/jobs")));
});
test("pending CI waits and then accepts success", async () => {
  const f = fixture({
    runs: (cycle) => [
      run(cycle ? {} : { status: "in_progress", conclusion: null }),
    ],
  });
  await waitForReleaseCi(f.config);
  assert.equal(f.calls.filter((p) => p.includes("/jobs?")).length, 1);
});
for (const conclusion of [
  "failure",
  "cancelled",
  "timed_out",
  "skipped",
  "neutral",
]) {
  test(`CI ${conclusion} stops release immediately`, async () => {
    const f = fixture({ runs: [run({ conclusion })] });
    await assert.rejects(waitForReleaseCi(f.config), new RegExp(conclusion));
    assert.equal(
      f.calls.some((p) => p.includes("/jobs?")),
      false,
    );
  });
}
test("a missing required run stops at the bounded discovery timeout", async () => {
  await assert.rejects(
    waitForReleaseCi(fixture({ runs: [] }).config),
    /missing/,
  );
});
test("a running CI stops at the overall timeout", async () => {
  await assert.rejects(
    waitForReleaseCi(
      fixture({ runs: [run({ status: "in_progress", conclusion: null })] })
        .config,
    ),
    /timed out/,
  );
});
test("CI API failure cannot be interpreted as a pass", async () => {
  await assert.rejects(
    waitForReleaseCi(
      fixture({
        request: async () => {
          throw new Error("HTTP 403");
        },
      }).config,
    ),
    /403/,
  );
});
test("a new rerun invalidates the previous successful attempt", async () => {
  const f = fixture({
    runs: (cycle) => [run({ run_attempt: cycle ? 2 : 1 })],
    current: (cycle) =>
      run(
        cycle
          ? { run_attempt: 2 }
          : { run_attempt: 2, status: "in_progress", conclusion: null },
      ),
  });
  await waitForReleaseCi(f.config);
  assert.ok(f.calls.some((p) => p.includes("/attempts/2/jobs")));
});
test("incomplete API inventories fail closed", async () => {
  const f = fixture({
    intercept: (p) =>
      p.includes("/workflows/10/runs?")
        ? { total_count: 1, workflow_runs: [] }
        : undefined,
  });
  await assert.rejects(waitForReleaseCi(f.config), /Incomplete/);
});
