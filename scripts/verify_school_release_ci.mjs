#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { appendFile, readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

const requireTools = createRequire(
  new URL("../frontend-school/package.json", import.meta.url),
);
const { parse: parseYaml } = requireTools("yaml");
const SHA = /^[0-9a-f]{40}$/;
export const workflows = [
  "api-contract.yml",
  "permission-contract.yml",
  "installer.yml",
];

// Current workflow paths use only *, ** and ?. Reject new unsupported syntax
// instead of silently deciding that a required workflow is irrelevant.
export function matchesPaths(files, patterns) {
  if (!Array.isArray(patterns) || patterns.length === 0)
    throw new Error("CI path filters are missing");
  return files.some((file) => {
    let matches = false;
    for (const value of patterns) {
      const negative = value.startsWith("!");
      const pattern = negative ? value.slice(1) : value;
      if (/[\[\]+]/.test(pattern))
        throw new Error("Unsupported CI path filter syntax");
      let source = "";
      for (let i = 0; i < pattern.length; i++) {
        if (pattern[i] === "*" && pattern[i + 1] === "*") {
          i++;
          if (pattern[i + 1] === "/") {
            source += "(?:.*/)?";
            i++;
          } else source += ".*";
        } else if (pattern[i] === "*") source += "[^/]*";
        else if (pattern[i] === "?") source += "[^/]";
        else source += pattern[i].replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      }
      if (new RegExp(`^${source}$`).test(file)) matches = !negative;
    }
    return matches;
  });
}

export async function requiredPushChecks(files, read = readFile) {
  const checks = [];
  for (const file of workflows) {
    const definition = parseYaml(
      await read(
        new URL(`../.github/workflows/${file}`, import.meta.url),
        "utf8",
      ),
    );
    if (!definition.on?.push?.branches?.includes("main"))
      throw new Error(`${file}: main push CI is missing`);
    if (
      file !== "installer.yml" ||
      matchesPaths(files, definition.on.push.paths)
    ) {
      if (!matchesPaths(files, definition.on.push.paths))
        throw new Error(`${file}: release inputs are not covered by CI`);
      checks.push({
        file,
        jobs: Object.entries(definition.jobs).map(
          ([id, job]) => job.name ?? id,
        ),
      });
    }
  }
  return checks;
}

export function latestTrustedRun(runs, { repository, sha, workflowId, file }) {
  const selected = runs
    .filter(
      (run) =>
        run.head_sha === sha &&
        run.head_branch === "main" &&
        run.event === "push" &&
        run.workflow_id === workflowId &&
        run.repository?.full_name === repository &&
        run.head_repository?.full_name === repository &&
        (run.path === `.github/workflows/${file}` ||
          run.path === `.github/workflows/${file}@refs/heads/main`),
    )
    .sort((a, b) => b.id - a.id)[0];
  if (
    selected &&
    (!Number.isSafeInteger(selected.id) ||
      selected.id <= 0 ||
      !Number.isSafeInteger(selected.run_attempt) ||
      selected.run_attempt <= 0)
  ) {
    throw new Error(`${file}: invalid CI run identity`);
  }
  return selected;
}

export function requireSuccessfulJobs(jobs, run, required) {
  for (const name of required) {
    const matches = jobs.filter((job) => job.name === name);
    if (
      matches.length !== 1 ||
      matches[0].run_id !== run.id ||
      matches[0].head_sha !== run.head_sha ||
      matches[0].run_attempt !== run.run_attempt ||
      matches[0].status !== "completed" ||
      matches[0].conclusion !== "success"
    ) {
      throw new Error(
        `CI run ${run.id}: required job ${name} did not succeed on this SHA/attempt`,
      );
    }
  }
}

export async function waitForReleaseCi({
  repository,
  sha,
  checks,
  request,
  now = Date.now,
  sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  log = console.log,
  timeoutMs = 30 * 60_000,
  discoveryMs = 120_000,
  intervalMs = 15_000,
}) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository) || !SHA.test(sha))
    throw new Error("Invalid release repository or SHA");
  if (!checks.length) throw new Error("Release CI checks are missing");
  const started = now();
  const metadata = await Promise.all(
    checks.map(async (check) => {
      const workflow = await request(
        `/repos/${repository}/actions/workflows/${check.file}`,
      );
      if (
        !Number.isSafeInteger(workflow.id) ||
        workflow.path !== `.github/workflows/${check.file}`
      )
        throw new Error("Invalid CI workflow identity");
      return { ...check, workflowId: workflow.id };
    }),
  );
  async function collection(endpoint, key) {
    const items = [];
    for (let page = 1; page <= 10; page++) {
      const result = await request(
        `${endpoint}${endpoint.includes("?") ? "&" : "?"}per_page=100&page=${page}`,
      );
      if (
        !Array.isArray(result[key]) ||
        !Number.isSafeInteger(result.total_count) ||
        result.total_count < 0
      )
        throw new Error("Invalid CI inventory");
      items.push(...result[key]);
      if (items.length === result.total_count) return items;
      if (items.length > result.total_count || result[key].length === 0)
        throw new Error("Incomplete CI inventory");
    }
    throw new Error("CI inventory exceeds the bounded pagination limit");
  }
  while (now() - started < timeoutMs) {
    let passed = true;
    for (const check of metadata) {
      const runs = await collection(
        `/repos/${repository}/actions/workflows/${check.workflowId}/runs?head_sha=${sha}&branch=main&event=push`,
        "workflow_runs",
      );
      const run = latestTrustedRun(runs, { repository, sha, ...check });
      if (!run) {
        if (now() - started >= discoveryMs)
          throw new Error(
            `${check.file}: required CI run is missing for ${sha}`,
          );
        log(`${check.file}: waiting for required CI run`);
        passed = false;
        continue;
      }
      log(
        `${check.file}: run ${run.id}, attempt ${run.run_attempt}, ${run.status}/${run.conclusion ?? "pending"}`,
      );
      if (run.status !== "completed") {
        passed = false;
        continue;
      }
      if (run.conclusion !== "success")
        throw new Error(`${check.file}: CI ${run.conclusion} for ${sha}`);
      const jobs = await collection(
        `/repos/${repository}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs`,
        "jobs",
      );
      requireSuccessfulJobs(jobs, run, check.jobs);
      const current = await request(
        `/repos/${repository}/actions/runs/${run.id}`,
      );
      if (
        current.run_attempt !== run.run_attempt ||
        current.status !== "completed" ||
        current.conclusion !== "success"
      ) {
        passed = false; // A rerun started while its previous attempt was being inspected.
      }
    }
    if (passed) return;
    await sleep(intervalMs);
  }
  throw new Error(`Release CI timed out for ${sha}`);
}

async function main() {
  const {
    GITHUB_REPOSITORY: repository,
    GITHUB_SHA: sha,
    PUSH_BASE_SHA: base,
    GH_TOKEN: token,
  } = process.env;
  if (!SHA.test(sha ?? "") || !SHA.test(base ?? "") || !token)
    throw new Error("Release CI environment is incomplete");
  const args =
    base === "0".repeat(40)
      ? ["ls-tree", "-r", "--name-only", "-z", sha]
      : ["diff", "--no-renames", "--name-only", "-z", base, sha];
  const files = execFileSync("git", args, { encoding: "utf8" })
    .split("\0")
    .filter(Boolean);
  const checks = await requiredPushChecks(files);
  const request = async (endpoint) => {
    const response = await fetch(`https://api.github.com${endpoint}`, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
      },
      signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok)
      throw new Error(`GitHub CI API failed: HTTP ${response.status}`);
    return response.json();
  };
  try {
    await waitForReleaseCi({ repository, sha, checks, request });
    if (process.env.GITHUB_STEP_SUMMARY)
      await appendFile(
        process.env.GITHUB_STEP_SUMMARY,
        `\nRelease CI passed for \`${sha}\`: ${checks.map((c) => c.file).join(", ")}.\n`,
      );
  } catch (error) {
    if (process.env.GITHUB_STEP_SUMMARY)
      await appendFile(
        process.env.GITHUB_STEP_SUMMARY,
        `\nRelease blocked before production changes: ${error.message}\n`,
      );
    throw error;
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
