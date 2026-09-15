#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const RELEASE_ID_PATTERN = /^[0-9a-f]{40}$/;
const POSITIVE_INTEGER_PATTERN = /^[1-9][0-9]*$/;
const WORKFLOW_PATH_PATTERN =
  /^\.github\/workflows\/deploy-school-release\.yml(?:@(?:refs\/heads\/)?main)?$/;

function fail(message) {
  console.error(message);
  process.exitCode = 64;
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isPositiveInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function parseTimestamp(value, label) {
  if (typeof value !== "string") {
    throw new Error(`${label} timestamp is invalid`);
  }
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) {
    throw new Error(`${label} timestamp is invalid`);
  }
  return timestamp;
}

function collectPageItems(pages, collectionName, label) {
  if (!Array.isArray(pages) || pages.length === 0) {
    throw new Error(`${label} pages are missing`);
  }

  const expectedTotal = pages[0]?.total_count;
  if (!Number.isSafeInteger(expectedTotal) || expectedTotal < 0) {
    throw new Error(`${label} total count is invalid`);
  }

  const items = [];
  for (const page of pages) {
    if (
      !isObject(page) ||
      page.total_count !== expectedTotal ||
      !Array.isArray(page[collectionName])
    ) {
      throw new Error(`${label} page is invalid`);
    }
    items.push(...page[collectionName]);
  }
  if (items.length !== expectedTotal) {
    throw new Error(`${label} inventory is incomplete`);
  }

  const ids = new Set();
  for (const item of items) {
    if (!isObject(item) || !isPositiveInteger(item.id)) {
      throw new Error(`${label} inventory contains an invalid ID`);
    }
    if (ids.has(item.id)) {
      throw new Error(`${label} inventory contains duplicate ${label} IDs`);
    }
    ids.add(item.id);
  }
  return items;
}

function isFrontendPromotion(name) {
  return name === "promote-frontends" || name.startsWith("promote-frontends (");
}

function printResult({
  alreadyDeployed,
  acceptedAttempt = "",
  acceptedScope = "",
  acceptedFrontend = false,
  acceptedBackend = false,
  restoreState = false,
}) {
  console.log(
    [
      `already_deployed=${alreadyDeployed}`,
      `accepted_attempt=${acceptedAttempt}`,
      `accepted_scope=${acceptedScope}`,
      `accepted_frontend=${acceptedFrontend}`,
      `accepted_backend=${acceptedBackend}`,
      `restore_state=${restoreState}`,
    ].join("\n"),
  );
}

const [
  ,
  ,
  currentAttemptInput,
  releaseId,
  currentRunPath,
  jobPagesPath,
  runPagesPath,
] = process.argv;

if (!POSITIVE_INTEGER_PATTERN.test(currentAttemptInput ?? "")) {
  fail("Current workflow attempt must be a positive integer");
} else if (!RELEASE_ID_PATTERN.test(releaseId ?? "")) {
  fail("Release ID must be a 40-character lowercase Git SHA");
} else if (!currentRunPath || !jobPagesPath || !runPagesPath) {
  fail(
    "Current run, job pages, and workflow run pages JSON paths are required",
  );
} else {
  const currentAttempt = Number(currentAttemptInput);
  if (!Number.isSafeInteger(currentAttempt)) {
    fail("Current workflow attempt is outside the supported integer range");
  } else {
    try {
      const [currentRun, jobPages, runPages] = await Promise.all([
        readFile(currentRunPath, "utf8").then(JSON.parse),
        readFile(jobPagesPath, "utf8").then(JSON.parse),
        readFile(runPagesPath, "utf8").then(JSON.parse),
      ]);
      if (
        !isObject(currentRun) ||
        !isPositiveInteger(currentRun.id) ||
        currentRun.run_attempt !== currentAttempt ||
        currentRun.head_sha !== releaseId ||
        !isPositiveInteger(currentRun.workflow_id) ||
        currentRun.head_branch !== "main" ||
        (currentRun.event !== "push" &&
          currentRun.event !== "workflow_dispatch") ||
        !WORKFLOW_PATH_PATTERN.test(currentRun.path ?? "") ||
        typeof currentRun.repository?.full_name !== "string" ||
        currentRun.head_repository?.full_name !==
          currentRun.repository.full_name
      ) {
        throw new Error("Current workflow run identity is invalid");
      }
      const currentCreatedAt = parseTimestamp(
        currentRun.created_at,
        "Current workflow run",
      );

      const jobs = collectPageItems(jobPages, "jobs", "job");
      if (
        !jobs.every(
          (job) =>
            job.run_id === currentRun.id &&
            job.head_sha === releaseId &&
            Number.isInteger(job.run_attempt) &&
            job.run_attempt > 0 &&
            job.run_attempt <= currentAttempt &&
            typeof job.name === "string" &&
            (typeof job.conclusion === "string" || job.conclusion === null),
        )
      ) {
        throw new Error("Job inventory contains an invalid job");
      }

      const workflowRuns = collectPageItems(
        runPages,
        "workflow_runs",
        "workflow run",
      );
      if (
        !workflowRuns.every(
          (run) =>
            run.workflow_id === currentRun.workflow_id &&
            run.head_branch === "main" &&
            run.repository?.full_name === currentRun.repository.full_name &&
            run.head_repository?.full_name === currentRun.repository.full_name,
        ) ||
        !workflowRuns.some((run) => run.id === currentRun.id)
      ) {
        throw new Error("Workflow run inventory contains an invalid run");
      }
      const hasNewerRun = workflowRuns.some((run) => {
        if (run.id === currentRun.id) return false;
        const createdAt = parseTimestamp(run.created_at, "Workflow run");
        return (
          createdAt > currentCreatedAt ||
          (createdAt === currentCreatedAt && run.id > currentRun.id)
        );
      });

      const acceptedAttempts = [
        ...new Set(
          jobs
            .filter(
              (job) =>
                job.run_attempt < currentAttempt &&
                job.name === "accept-release" &&
                job.conclusion === "success",
            )
            .map((job) => job.run_attempt),
        ),
      ].sort((left, right) => right - left);

      const acceptedAttempt = acceptedAttempts[0];
      if (acceptedAttempt === undefined) {
        printResult({ alreadyDeployed: false });
      } else {
        const acceptedFrontend = jobs.some(
          (job) =>
            job.run_attempt <= acceptedAttempt &&
            isFrontendPromotion(job.name) &&
            job.conclusion === "success",
        );
        const acceptedBackend = jobs.some(
          (job) =>
            job.run_attempt <= acceptedAttempt &&
            job.name === "deploy-backend" &&
            job.conclusion === "success",
        );
        if (!acceptedFrontend && !acceptedBackend) {
          printResult({ alreadyDeployed: false });
        } else {
          printResult({
            alreadyDeployed: true,
            acceptedAttempt,
            acceptedScope:
              acceptedFrontend && acceptedBackend
                ? "full"
                : acceptedFrontend
                  ? "frontend"
                  : "backend",
            acceptedFrontend,
            acceptedBackend,
            restoreState: !hasNewerRun,
          });
        }
      }
    } catch (error) {
      fail(
        error instanceof Error
          ? `Could not inspect school release replay: ${error.message}`
          : "Could not inspect school release replay",
      );
    }
  }
}
