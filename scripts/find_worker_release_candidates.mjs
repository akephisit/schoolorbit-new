#!/usr/bin/env node

import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const ACCOUNT_ID_PATTERN = /^[0-9a-f]{32}$/;
const RELEASE_ID_PATTERN = /^[0-9a-f]{40}$/;
const VERSION_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const WORKER_NAME_PATTERN = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/;
const DEFAULT_API_BASE_URL = "https://api.cloudflare.com/client/v4";
const PAGE_SIZE = 100;
const MAX_PAGES = 100;
const DETAIL_CONCURRENCY = 5;

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireMatch(value, pattern, label) {
  if (typeof value !== "string" || !pattern.test(value)) {
    throw new Error(`${label} is invalid`);
  }
}

function readAnnotation(record) {
  if (!Object.hasOwn(record, "annotations")) return { known: false, tag: "" };
  if (record.annotations !== null && !isRecord(record.annotations)) {
    throw new Error("Worker version annotations are invalid");
  }
  const tag = record.annotations?.["workers/tag"] ?? "";
  if (typeof tag !== "string") throw new Error("Worker version tag is invalid");
  return { known: true, tag };
}

async function readJson(url, apiToken, fetchImpl) {
  const response = await fetchImpl(url, {
    headers: { Authorization: `Bearer ${apiToken}` },
    signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok) {
    throw new Error(
      `Cloudflare API request failed with HTTP ${response.status}`,
    );
  }
  try {
    return await response.json();
  } catch {
    throw new Error("Cloudflare API returned invalid JSON");
  }
}

function validateEnvelope(payload, label) {
  if (
    !isRecord(payload) ||
    payload.success !== true ||
    !isRecord(payload.result)
  ) {
    throw new Error(`Cloudflare API returned an invalid ${label}`);
  }
}

async function loadVersionDetail({
  apiBaseUrl,
  accountId,
  workerName,
  versionId,
  apiToken,
  fetchImpl,
}) {
  const url = new URL(
    `accounts/${accountId}/workers/scripts/${encodeURIComponent(workerName)}/versions/${versionId}`,
    `${apiBaseUrl.replace(/\/$/, "")}/`,
  );
  const payload = await readJson(url, apiToken, fetchImpl);
  validateEnvelope(payload, "Worker version detail");
  if (payload.result.id !== versionId) {
    throw new Error("Cloudflare API returned the wrong Worker version detail");
  }
  const annotation = readAnnotation(payload.result);
  if (!annotation.known) {
    throw new Error("Worker version detail omitted annotations");
  }
  return annotation.tag;
}

export async function findWorkerReleaseCandidates({
  accountId,
  apiToken,
  workerName,
  releaseId,
  fetchImpl = fetch,
  apiBaseUrl = DEFAULT_API_BASE_URL,
}) {
  requireMatch(accountId, ACCOUNT_ID_PATTERN, "Cloudflare account ID");
  requireMatch(workerName, WORKER_NAME_PATTERN, "Worker name");
  if (workerName.length > 128) throw new Error("Worker name is invalid");
  requireMatch(releaseId, RELEASE_ID_PATTERN, "release ID");
  if (typeof apiToken !== "string" || apiToken.length === 0) {
    throw new Error("Cloudflare API token is unavailable");
  }

  const versions = [];
  const versionIds = new Set();
  let completed = false;

  for (let page = 1; page <= MAX_PAGES; page += 1) {
    const url = new URL(
      `accounts/${accountId}/workers/scripts/${encodeURIComponent(workerName)}/versions`,
      `${apiBaseUrl.replace(/\/$/, "")}/`,
    );
    url.searchParams.set("page", String(page));
    url.searchParams.set("per_page", String(PAGE_SIZE));

    const payload = await readJson(url, apiToken, fetchImpl);
    validateEnvelope(payload, "Worker version inventory");
    if (!Array.isArray(payload.result.items)) {
      throw new Error(
        "Cloudflare API returned an invalid Worker version inventory",
      );
    }

    for (const item of payload.result.items) {
      if (
        !isRecord(item) ||
        typeof item.id !== "string" ||
        !VERSION_ID_PATTERN.test(item.id)
      ) {
        throw new Error("Cloudflare API returned an invalid Worker version ID");
      }
      if (versionIds.has(item.id)) {
        throw new Error("Cloudflare API returned a duplicate Worker version");
      }
      readAnnotation(item);
      versionIds.add(item.id);
      versions.push(item);
    }

    const resultInfo = payload.result_info;
    if (resultInfo !== undefined) {
      if (
        !isRecord(resultInfo) ||
        (resultInfo.page !== undefined &&
          (!Number.isInteger(resultInfo.page) || resultInfo.page !== page)) ||
        (resultInfo.per_page !== undefined &&
          (!Number.isInteger(resultInfo.per_page) ||
            resultInfo.per_page <= 0)) ||
        (resultInfo.total_pages !== undefined &&
          (!Number.isInteger(resultInfo.total_pages) ||
            resultInfo.total_pages < 0 ||
            resultInfo.total_pages > MAX_PAGES ||
            (payload.result.items.length > 0 && resultInfo.total_pages < page)))
      ) {
        throw new Error(
          "Cloudflare API returned invalid Worker version pagination",
        );
      }
      if (
        payload.result.items.length === 0 &&
        resultInfo.total_pages !== undefined &&
        page < resultInfo.total_pages
      ) {
        throw new Error("Cloudflare API ended Worker version pagination early");
      }
      if (
        payload.result.items.length === 0 ||
        (resultInfo.total_pages !== undefined && page >= resultInfo.total_pages)
      ) {
        completed = true;
        break;
      }
    } else if (payload.result.items.length === 0) {
      completed = true;
      break;
    }
  }

  if (!completed)
    throw new Error("Worker version inventory exceeded the pagination limit");

  const candidates = [];
  for (let offset = 0; offset < versions.length; offset += DETAIL_CONCURRENCY) {
    const batch = versions.slice(offset, offset + DETAIL_CONCURRENCY);
    const tags = await Promise.all(
      batch.map(async (version) => {
        const annotation = readAnnotation(version);
        if (annotation.known) return annotation.tag;
        return loadVersionDetail({
          apiBaseUrl,
          accountId,
          workerName,
          versionId: version.id,
          apiToken,
          fetchImpl,
        });
      }),
    );
    for (let index = 0; index < batch.length; index += 1) {
      if (tags[index] === releaseId) candidates.push(batch[index].id);
    }
  }

  return candidates;
}

const isMain =
  process.argv[1] !== undefined &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url;

if (isMain) {
  findWorkerReleaseCandidates({
    accountId: process.env.CLOUDFLARE_ACCOUNT_ID,
    apiToken: process.env.CLOUDFLARE_API_TOKEN,
    workerName: process.argv[2],
    releaseId: process.argv[3],
  })
    .then((candidates) =>
      process.stdout.write(`${JSON.stringify(candidates)}\n`),
    )
    .catch((error) => {
      const message =
        error instanceof Error ? error.message : "unknown failure";
      process.stderr.write(`Worker version inventory failed: ${message}\n`);
      process.exitCode = 1;
    });
}
