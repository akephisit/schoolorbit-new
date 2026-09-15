#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const RELEASE_ID_PATTERN = /^[0-9a-f]{40}$/;
const POSITIVE_INTEGER_PATTERN = /^[1-9][0-9]*$/;

function fail(message) {
  console.error(message);
  process.exitCode = 64;
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
}) {
  console.log(
    [
      `already_deployed=${alreadyDeployed}`,
      `accepted_attempt=${acceptedAttempt}`,
      `accepted_scope=${acceptedScope}`,
      `accepted_frontend=${acceptedFrontend}`,
      `accepted_backend=${acceptedBackend}`,
    ].join("\n"),
  );
}

const [, , currentAttemptInput, releaseId, jobsPath] = process.argv;

if (!POSITIVE_INTEGER_PATTERN.test(currentAttemptInput ?? "")) {
  fail("Current workflow attempt must be a positive integer");
} else if (!RELEASE_ID_PATTERN.test(releaseId ?? "")) {
  fail("Release ID must be a 40-character lowercase Git SHA");
} else if (!jobsPath) {
  fail("Workflow jobs JSON path is required");
} else {
  const currentAttempt = Number(currentAttemptInput);
  if (!Number.isSafeInteger(currentAttempt)) {
    fail("Current workflow attempt is outside the supported integer range");
  } else {
    try {
      const jobs = JSON.parse(await readFile(jobsPath, "utf8"));
      if (!Array.isArray(jobs)) {
        throw new Error("Workflow jobs response must be an array");
      }
      if (
        !jobs.every(
          (job) =>
            job &&
            Number.isInteger(job.run_attempt) &&
            job.run_attempt > 0 &&
            typeof job.head_sha === "string" &&
            typeof job.name === "string" &&
            (typeof job.conclusion === "string" || job.conclusion === null),
        )
      ) {
        throw new Error("Workflow jobs response contains an invalid job");
      }

      const acceptedAttempts = [
        ...new Set(
          jobs
            .filter(
              (job) =>
                job.run_attempt < currentAttempt &&
                job.head_sha === releaseId &&
                job.name === "accept-release" &&
                job.conclusion === "success",
            )
            .map((job) => job.run_attempt),
        ),
      ].sort((left, right) => right - left);

      let replay;
      for (const acceptedAttempt of acceptedAttempts) {
        const acceptedFrontend = jobs.some(
          (job) =>
            job.run_attempt === acceptedAttempt &&
            job.head_sha === releaseId &&
            isFrontendPromotion(job.name) &&
            job.conclusion === "success",
        );
        const acceptedBackend = jobs.some(
          (job) =>
            job.run_attempt === acceptedAttempt &&
            job.head_sha === releaseId &&
            job.name === "deploy-backend" &&
            job.conclusion === "success",
        );
        if (!acceptedFrontend && !acceptedBackend) continue;

        replay = {
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
        };
        break;
      }

      printResult(replay ?? { alreadyDeployed: false });
    } catch (error) {
      fail(
        error instanceof Error
          ? `Could not inspect workflow jobs: ${error.message}`
          : "Could not inspect workflow jobs",
      );
    }
  }
}
