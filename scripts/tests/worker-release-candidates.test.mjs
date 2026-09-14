import assert from "node:assert/strict";
import test from "node:test";

import { findWorkerReleaseCandidates } from "../find_worker_release_candidates.mjs";

const releaseId = "a".repeat(40);
const candidateId = "11111111-1111-4111-8111-111111111111";

function response(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function version(id, tag) {
  return {
    id,
    annotations: tag === undefined ? {} : { "workers/tag": tag },
  };
}

test("finds a release candidate beyond the first inventory page", async () => {
  const calls = [];
  const fetchImpl = async (input, options) => {
    const url = new URL(input);
    calls.push({ url, options });
    const page = Number(url.searchParams.get("page"));
    if (page === 1) {
      return response({
        success: true,
        result: {
          items: Array.from({ length: 10 }, (_, index) =>
            version(
              `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
            ),
          ),
        },
        result_info: { page: 1, total_pages: 2 },
      });
    }
    return response({
      success: true,
      result: { items: [version(candidateId, releaseId)] },
      result_info: { page: 2, total_pages: 2 },
    });
  };

  const candidates = await findWorkerReleaseCandidates({
    accountId: "b".repeat(32),
    apiToken: "test-token",
    workerName: "schoolorbit-school-sandbox",
    releaseId,
    fetchImpl,
  });

  assert.deepEqual(candidates, [candidateId]);
  assert.equal(calls.length, 2);
  assert.equal(calls[0].url.searchParams.get("deployable"), null);
  assert.equal(calls[0].options.headers.Authorization, "Bearer test-token");
});

test("loads exact version details when list metadata omits annotations", async () => {
  const fetchImpl = async (input) => {
    const url = new URL(input);
    if (url.pathname.endsWith(`/versions/${candidateId}`)) {
      return response({
        success: true,
        result: version(candidateId, releaseId),
      });
    }
    return response({
      success: true,
      result: { items: [{ id: candidateId }] },
      result_info: { page: 1, total_pages: 1 },
    });
  };

  const candidates = await findWorkerReleaseCandidates({
    accountId: "b".repeat(32),
    apiToken: "test-token",
    workerName: "schoolorbit-school-sandbox",
    releaseId,
    fetchImpl,
  });

  assert.deepEqual(candidates, [candidateId]);
});

test("paginates to an empty page when result info omits total pages", async () => {
  const fetchImpl = async (input) => {
    const page = Number(new URL(input).searchParams.get("page"));
    return response({
      success: true,
      result: { items: page === 1 ? [version(candidateId)] : [] },
      result_info: { page, per_page: 100 },
    });
  };

  const candidates = await findWorkerReleaseCandidates({
    accountId: "b".repeat(32),
    apiToken: "test-token",
    workerName: "schoolorbit-school-sandbox",
    releaseId,
    fetchImpl,
  });

  assert.deepEqual(candidates, []);
});

test("fails closed when pagination repeats a version", async () => {
  const fetchImpl = async (input) => {
    const page = Number(new URL(input).searchParams.get("page"));
    return response({
      success: true,
      result: { items: [version(candidateId)] },
      result_info: { page, total_pages: 2 },
    });
  };

  await assert.rejects(
    findWorkerReleaseCandidates({
      accountId: "b".repeat(32),
      apiToken: "test-token",
      workerName: "schoolorbit-school-sandbox",
      releaseId,
      fetchImpl,
    }),
    /duplicate Worker version/,
  );
});

test("fails closed when an empty page contradicts the reported total", async () => {
  await assert.rejects(
    findWorkerReleaseCandidates({
      accountId: "b".repeat(32),
      apiToken: "test-token",
      workerName: "schoolorbit-school-sandbox",
      releaseId,
      fetchImpl: async () =>
        response({
          success: true,
          result: { items: [] },
          result_info: { page: 1, total_pages: 2 },
        }),
    }),
    /ended Worker version pagination early/,
  );
});

test("fails closed on an invalid Cloudflare response", async () => {
  await assert.rejects(
    findWorkerReleaseCandidates({
      accountId: "b".repeat(32),
      apiToken: "test-token",
      workerName: "schoolorbit-school-sandbox",
      releaseId,
      fetchImpl: async () => response({ success: false, result: null }),
    }),
    /invalid Worker version inventory/,
  );
});
