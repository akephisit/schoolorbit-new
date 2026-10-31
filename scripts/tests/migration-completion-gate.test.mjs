import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { spawn } from "node:child_process";

const workflow = readFileSync(
  "scripts/lib/pipeline-remote/school-deploy-backend.sh",
  "utf8",
);
const body = workflow.match(
  /migration_completion_filter="([\s\S]*?)"\n\s*if ! podman/,
)?.[1];
assert.ok(body, "exercise the migration filter actually used by the release");
const filter = body.replaceAll('\\"', '"').replaceAll("\\$", "$");

async function accepts(report) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "docker",
      ["run", "--rm", "-i", "ghcr.io/jqlang/jq:1.7.1", "-e", filter],
      {
        stdio: ["pipe", "ignore", "pipe"],
      },
    );
    let error = "";
    child.stderr.on("data", (chunk) => (error += chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0 || code === 1 || code === 4) resolve(code === 0);
      else reject(new Error(`Migration filter failed (${code}): ${error}`));
    });
    child.stdin.end(JSON.stringify(report));
  });
}

const contextCodes = [
  "ACADEMIC_CONTEXT_CUTOVER_EVIDENCE",
  "ACADEMIC_CONTEXT_RETIRED_COLUMNS",
  "ACADEMIC_CONTEXT_EXPLICIT_CALENDAR_SCOPE",
  "ACADEMIC_CONTEXT_VISIBLE_DEFAULTS",
  "ACADEMIC_CONTEXT_LEGACY_OWNER_REMOVED",
  "ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE",
];

function validReport(version = 149) {
  const school = {
    migration_version: version,
    migration_status: "migrated",
    migration_error: null,
    academicContextCutover: {
      migrationVersion: 149,
      status: "cutoverCompleted",
      passed: true,
      checks: contextCodes.map((code) => ({ code, passed: true })),
    },
    academicCoreCutover: {
      migrationVersion: 45,
      status: "cleanupCompleted",
      passed: true,
      checks: [{ passed: true }],
    },
    gradebookResultsCutover: {
      migrationVersion: 60,
      status: "cutoverCompleted",
      passed: true,
      checks: [{ passed: true }],
    },
    deliveryTimetableCutover: {
      migrationVersion: 88,
      status: "cutoverCompleted",
      passed: true,
      checks: Array.from({ length: 30 }, () => ({ passed: true })),
    },
    staffProfileCutover: {
      migrationVersion: 114,
      status: "cutoverCompleted",
      passed: true,
      checks: Array.from({ length: 4 }, () => ({ passed: true })),
    },
  };
  return {
    latest_version: version,
    total_schools: 2,
    migrated: 2,
    pending: 0,
    failed: 0,
    outdated: 0,
    schools: [structuredClone(school), structuredClone(school)],
  };
}

test("fully migrated tenants pass without a retired personnel report", async () => {
  assert.equal(await accepts(validReport()), true);
  assert.equal(
    await accepts(validReport(150)),
    true,
    "future SQL migrations use the same completion gate",
  );
});

test("an authoritative empty first-install inventory passes after the headless completeness gate", async () => {
  const report = validReport();
  report.total_schools = 0;
  report.migrated = 0;
  report.schools = [];
  assert.equal(await accepts(report), true);
});

test("migration completion refuses incomplete, failed or inconsistent tenant coverage", async () => {
  const changes = [
    (r) => {
      r.schools.pop();
    },
    (r) => {
      r.total_schools = -1;
      r.migrated = -1;
      r.schools = [];
    },
    ...["pending", "failed", "outdated"].map((key) => (r) => {
      r[key] = 1;
    }),
    (r) => {
      r.migrated = 1;
    },
    (r) => {
      r.schools[1].migration_version = 87;
    },
    (r) => {
      r.schools[1].migration_version = 89;
    },
    (r) => {
      r.schools[1].migration_status = "failed";
    },
    (r) => {
      r.schools[1].migration_error = "synthetic_failure";
    },
  ];
  for (const change of changes) {
    const report = validReport();
    change(report);
    assert.equal(await accepts(report), false);
  }
});

test("retiring personnel hooks preserves the other domain audit gates", async () => {
  for (const key of [
    "academicCoreCutover",
    "gradebookResultsCutover",
    "deliveryTimetableCutover",
    "staffProfileCutover",
  ]) {
    for (const change of [
      (a) => {
        a.passed = false;
      },
      (a) => {
        a.status = "pending";
      },
      (a) => {
        a.checks[0].passed = false;
      },
      (a) => {
        a.migrationVersion = 0;
      },
    ]) {
      const report = validReport();
      change(report.schools[1][key]);
      assert.equal(await accepts(report), false);
    }
  }
});

test("delivery cutover requires both complete reconciliation audits before release", async () => {
  for (const change of [
    (r) => {
      delete r.schools[1].deliveryTimetableCutover;
    },
    (r) => {
      r.schools[1].deliveryTimetableCutover = null;
    },
    (r) => {
      r.schools[1].deliveryTimetableCutover.migrationVersion = 87;
    },
    (r) => {
      r.schools[1].deliveryTimetableCutover.checks = [];
    },
    (r) => {
      r.schools[1].deliveryTimetableCutover.checks.pop();
    },
    (r) => {
      r.schools[1].deliveryTimetableCutover.checks.push({ passed: true });
    },
    (r) => {
      delete r.schools[1].deliveryTimetableCutover.checks[0].passed;
    },
  ]) {
    const report = validReport();
    change(report);
    assert.equal(await accepts(report), false);
  }
});

test("staff profile cutover refuses missing or incomplete preservation proof", async () => {
  for (const change of [
    (r) => {
      delete r.schools[1].staffProfileCutover;
    },
    (r) => {
      r.schools[1].staffProfileCutover.checks = [];
    },
    (r) => {
      r.schools[1].staffProfileCutover.checks.pop();
    },
  ]) {
    const report = validReport();
    change(report);
    assert.equal(await accepts(report), false);
  }
});

test("academic context cutover refuses absent, stale, incomplete and failed evidence", async () => {
  const mutations = [
    (report) => {
      delete report.schools[0].academicContextCutover;
    },
    (report) => {
      report.schools[0].academicContextCutover.migrationVersion = 147;
    },
    (report) => {
      report.schools[0].academicContextCutover.migrationVersion = 148;
    },
    (report) => {
      report.schools[0].academicContextCutover.status = "pendingMigration";
    },
    (report) => {
      report.schools[0].academicContextCutover.passed = false;
    },
    (report) => {
      report.schools[0].academicContextCutover.checks = [];
    },
    (report) => {
      report.schools[0].academicContextCutover.checks = Object.fromEntries(
        report.schools[0].academicContextCutover.checks.map((check, index) => [
          index,
          check,
        ]),
      );
    },
    (report) => {
      report.schools[0].academicContextCutover.checks[0].passed = false;
    },
    (report) => {
      report.schools[0].academicContextCutover.checks.pop();
    },
    (report) => {
      report.schools[0].academicContextCutover.checks.push({
        code: contextCodes[5],
        passed: true,
      });
    },
    (report) => {
      report.schools[0].academicContextCutover.checks[5] = {
        code: contextCodes[0],
        passed: true,
      };
    },
    (report) => {
      report.schools[0].academicContextCutover.checks[5] = {
        code: "ACADEMIC_CONTEXT_UNKNOWN",
        passed: true,
      };
    },
    (report) => {
      delete report.schools[0].academicContextCutover.checks[5].code;
    },
    (report) => {
      report.schools[0].academicContextCutover.checks[5].passed = false;
    },
  ];
  for (const mutate of mutations) {
    const report = validReport();
    mutate(report);
    assert.equal(await accepts(report), false);
  }
});

test("context readiness cannot accept schema148 even when an empty inventory has no tenant checks", async () => {
  assert.equal(await accepts(validReport(148)), false);
  const report = validReport(148);
  report.total_schools = 0;
  report.migrated = 0;
  report.schools = [];
  assert.equal(await accepts(report), false);
});

test("latest migration is a numeric integer rather than a jq type-order comparison", async () => {
  for (const version of ["149", 149.5]) {
    const report = validReport(version);
    assert.equal(await accepts(report), false);
  }
});

test("six exact context codes permit arbitrary provider order but require every evidence result", async () => {
  const report = validReport();
  report.schools[0].academicContextCutover.checks.reverse();
  assert.equal(await accepts(report), true);
  for (const code of contextCodes) {
    const invalid = validReport();
    invalid.schools[0].academicContextCutover.checks.find(
      (check) => check.code === code,
    ).passed = false;
    assert.equal(await accepts(invalid), false);
  }
});
