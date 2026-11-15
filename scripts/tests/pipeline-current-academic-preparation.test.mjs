import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { releaseBundleFiles } from "../lib/pipeline-release-bundle.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = (name) =>
  readFileSync(new URL(`../../${name}`, import.meta.url), "utf8");

test("fixed preparation controller has portable pure failure coverage without provider or crypto tooling", () => {
  for (const args of [
    ["scripts/tests/current-academic-preparation.test.py", "PreparationTests"],
    ["scripts/tests/academic-restore-fidelity.test.py"],
  ]) {
    const result = spawnSync("python3", args, {
      cwd: root,
      env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
      encoding: "utf8",
      timeout: 60_000,
      maxBuffer: 500_000,
    });
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  }
});

test("release image contains the exact existing canonical copy CLI and checksum-pinned age tools", () => {
  const docker = read("backend-school/Dockerfile");
  const build = read("backend-school/scripts/build_release.sh");
  assert.match(
    docker,
    /ADD --checksum=sha256:7df45a6cc87d4da11cc03a539a7470c15b1041ab2b396af088fe9990f7c79d50 https:\/\/github\.com\/FiloSottile\/age\/releases\/download\/v1\.2\.1\/age-v1\.2\.1-linux-amd64\.tar\.gz/,
  );
  assert.match(
    docker,
    /COPY .*target\/release\/migrate_tenant_schema \/app\/migrate_tenant_schema/,
  );
  assert.match(docker, /COPY --from=academic-age-tool .*age-keygen/);
  assert.match(
    build,
    /cargo build --release --locked --bin migrate_tenant_schema/,
  );
  assert.match(
    build,
    /cargo rustc --release --locked --bin backend-school --timings --message-format=json-render-diagnostics/,
  );
});

test("preparation pins Debian-compatible PostgreSQL tools and archive-owned database restore", () => {
  const controller = read(
    "scripts/lib/pipeline-remote/current-academic-preparation.py",
  );
  assert.ok(
    controller.includes(
      "PG_IMAGE = 'docker.io/library/postgres:18.6-trixie@sha256:e3d24f6906d0760bf4da0dae872415391ededefad23fd471c47dc53b0ddf3c42'",
    ),
  );
  assert.ok(
    controller.includes("yield connection, args + [PG_IMAGE], environment"),
  );
  assert.ok(controller.includes(",target=/var/lib/postgresql', PG_IMAGE"));
  assert.ok(
    controller.includes(
      "'--read-only', PG_IMAGE, 'pg_restore', '--file=/dev/null'",
    ),
  );
  assert.ok(
    controller.includes(
      "'pg_restore', '--create', '--clean', '--if-exists', '--exit-on-error', '-U', 'postgres', '-d', 'postgres'",
    ),
  );
  assert.ok(!controller.includes("self.copy_sql('CREATE DATABASE '"));
});

test("immutable bundle and existing headless cutover own preparation after writer removal and before live migration", () => {
  assert.ok(
    releaseBundleFiles(root).includes(
      "scripts/lib/pipeline-remote/academic_restore_fidelity.py",
    ),
  );
  assert.ok(
    releaseBundleFiles(root).includes(
      "scripts/lib/pipeline-remote/current-academic-preparation.py",
    ),
  );
  const rollout = read("scripts/lib/pipeline-remote/school-storage-rollout.sh");
  const stop = rollout.indexOf('podman rm "$backend"');
  const prepare = rollout.indexOf('current-academic-preparation.py" prepare');
  const live = rollout.indexOf(
    'configured "$scratch/migration" schoolorbit-web migrate-tenants',
  );
  const complete = rollout.indexOf('current-academic-preparation.py" complete');
  assert.ok(stop >= 0 && stop < prepare && prepare < live && live < complete);
  for (const code of [
    "ACADEMIC_CURRENT_PREPARATION_REQUIRED",
    "ACADEMIC_CURRENT_PREPARATION_MISMATCH",
  ])
    assert.ok(rollout.includes(code));
});

test("release only forwards exact bounded controller and canonical refusal schemas", () => {
  const source = read("scripts/lib/pipeline-remote/school-storage-rollout.sh");
  const helper = source.slice(
    source.indexOf("report_preparation_refusal() {"),
    source.indexOf("private_file() {"),
  );
  const execute = (body) => {
    const directory = mkdtempSync(path.join(os.tmpdir(), "academic-refusal-"));
    const input = path.join(directory, "input.json");
    try {
      writeFileSync(
        input,
        typeof body === "string" ? body : JSON.stringify(body),
        { mode: 0o600 },
      );
      return spawnSync(
        "bash",
        ["-c", `${helper}\nreport_preparation_refusal "$1"`, "fixture", input],
        { encoding: "utf8", timeout: 10_000 },
      );
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  };
  const diagnostic = {
    schemaVersion: 1,
    operation: "migrate_tenant_schema",
    status: "refused",
    version: 157,
    sqlstate: "23514",
    code: "ACADEMIC_CURRENT_PREPARATION_MISMATCH",
  };
  assert.deepEqual(
    JSON.parse(
      execute({ schemaVersion: 1, error: "cutover_date_changed" }).stdout,
    ),
    { schemaVersion: 1, error: "cutover_date_changed" },
  );
  assert.deepEqual(
    JSON.parse(
      execute({
        schemaVersion: 1,
        error: "copy_migration_refused",
        migrationRefusal: diagnostic,
      }).stdout,
    ),
    {
      schemaVersion: 1,
      error: "copy_migration_refused",
      migrationRefusal: {
        version: 157,
        sqlstate: "23514",
        code: diagnostic.code,
      },
    },
  );
  const fidelity = {
    stage: "source_profile",
    facet: "memberships",
    mismatchCount: 1,
    propertyMask: 1 << 14,
  };
  const report = {
    schemaVersion: 1,
    error: "restored_source_mismatch",
    diagnostic: fidelity,
  };
  assert.deepEqual(JSON.parse(execute(report).stdout), report);
  // Match executable helper tables, including every stage and every facet ceiling.
  const tables = spawnSync(
    "python3",
    [
      "-c",
      `
import importlib.util,json,sys
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('f','scripts/lib/pipeline-remote/academic_restore_fidelity.py')
f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
print(json.dumps([f.DIAGNOSTIC_STAGES,{key:2**len(value) for key,value in f.DIAGNOSTIC_PROPERTIES.items()}]))
`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(tables.status, 0, tables.stderr);
  const [stages, limits] = JSON.parse(tables.stdout);
  for (const stage of stages)
    assert.deepEqual(
      JSON.parse(
        execute({ ...report, diagnostic: { ...fidelity, stage } }).stdout,
      ),
      { ...report, diagnostic: { ...fidelity, stage } },
    );
  for (const [facet, limit] of Object.entries(limits)) {
    const detail = {
      ...fidelity,
      facet,
      propertyMask: limit - 1,
      mismatchCount: facet === "unknown" ? 0 : 20000,
    };
    assert.deepEqual(
      JSON.parse(execute({ ...report, diagnostic: detail }).stdout),
      { ...report, diagnostic: detail },
    );
    assert.equal(
      execute({ ...report, diagnostic: { ...detail, propertyMask: limit } })
        .stdout,
      "",
    );
  }
  const invalid = [
    { ...report, error: "child_failed" },
    { ...report, migrationRefusal: diagnostic },
    {
      ...report,
      diagnostic: { ...fidelity, stage: "postgresql://private-sentinel" },
    },
    {
      ...report,
      diagnostic: { ...fidelity, facet: "SELECT private-sentinel" },
    },
    { ...report, diagnostic: { ...fidelity, raw: "private-sentinel" } },
    { ...report, diagnostic: null },
    ...[true, null, "1", 1.5, -1, 20001].map((mismatchCount) => ({
      ...report,
      diagnostic: { ...fidelity, mismatchCount },
    })),
    ...[true, null, "1", 1.5, -1, 2 ** 32].map((propertyMask) => ({
      ...report,
      diagnostic: { ...fidelity, propertyMask },
    })),
    JSON.stringify(report) + "\n" + JSON.stringify(report),
    JSON.stringify(report).replace('"mismatchCount":1', '"mismatchCount":NaN'),
    JSON.stringify(report).replace(
      '"mismatchCount":1',
      '"mismatchCount":Infinity',
    ),
    JSON.stringify(report).replace(
      '"mismatchCount":1',
      '"mismatchCount":1,"mismatchCount":2',
    ),
  ];
  for (const value of invalid) {
    const result = execute(value);
    assert.equal(result.stdout, "");
    assert.equal(result.stderr, "");
  }
  // Exercise the actual controller __main__ exception projection before rollout.
  const native = spawnSync(
    "python3",
    [
      "-c",
      `
import ast,importlib.util,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
path=Path('scripts/lib/pipeline-remote/current-academic-preparation.py')
spec=importlib.util.spec_from_file_location('controller',path)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Failure:
    def __init__(self):
        self.temp=tempfile.TemporaryDirectory();self.point=Path(self.temp.name)
    def prepare(self):
        self.diagnostic_stage='oracle'
        raise m.fidelity.FidelityError('oracle',('tree',))
m.Controller=Failure;m.__name__='__main__';sys.argv=['controller','prepare']
entry=ast.parse(path.read_text()).body[-1]
exec(compile(ast.Module(body=[entry],type_ignores=[]),str(path),'exec'),m.__dict__)
`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(native.status, 1);
  assert.equal(native.stdout, "");
  assert.deepEqual(JSON.parse(execute(native.stderr).stdout), {
    schemaVersion: 1,
    error: "restored_source_mismatch",
    diagnostic: {
      stage: "oracle",
      facet: "oracle",
      mismatchCount: 1,
      propertyMask: 128,
    },
  });
  const streamFailure = {
    stage: "restore_archive",
    producerExit: 0,
    consumerExit: 4,
    producerCode: "unknown",
    consumerCode: "missing_role",
    producerStderrTruncated: false,
    consumerStderrTruncated: true,
  };
  const streamReport = {
    schemaVersion: 1,
    error: "stream_failed",
    streamFailure,
  };
  assert.deepEqual(JSON.parse(execute(streamReport).stdout), streamReport);
  assert.deepEqual(
    JSON.parse(execute({ schemaVersion: 1, error: "stream_failed" }).stdout),
    { schemaVersion: 1, error: "stream_failed" },
  );
  const streamTables = spawnSync(
    "python3",
    [
      "-c",
      `
import importlib.util,json,sys
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('m','scripts/lib/pipeline-remote/current-academic-preparation.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
print(json.dumps([m.STREAM_STAGES,m.STREAM_CODES]))
`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(streamTables.status, 0, streamTables.stderr);
  const [streamStages, streamCodes] = JSON.parse(streamTables.stdout);
  for (const stage of streamStages) {
    const value = {
      ...streamReport,
      streamFailure: { ...streamFailure, stage },
    };
    assert.deepEqual(JSON.parse(execute(value).stdout), value);
  }
  for (const code of streamCodes) {
    const value = {
      ...streamReport,
      streamFailure: {
        ...streamFailure,
        producerExit: -255,
        consumerExit: 255,
        producerCode: code,
        consumerCode: code,
      },
    };
    assert.deepEqual(JSON.parse(execute(value).stdout), value);
  }
  const invalidStreams = [
    { ...streamReport, error: "child_failed" },
    { ...streamReport, diagnostic: fidelity },
    { ...streamReport, migrationRefusal: diagnostic },
    { ...report, streamFailure },
    { ...streamReport, streamFailure: null },
    { ...streamReport, streamFailure: { ...streamFailure, consumerExit: 0 } },
    {
      ...streamReport,
      streamFailure: { ...streamFailure, raw: "private-sentinel" },
    },
    ...["stage", "producerCode", "consumerCode"].flatMap((key) =>
      [null, true, "postgresql://private-sentinel", "unknown\u0000"].map(
        (value) => ({
          ...streamReport,
          streamFailure: { ...streamFailure, [key]: value },
        }),
      ),
    ),
    ...["producerExit", "consumerExit"].flatMap((key) =>
      [null, true, "1", 1.5, -256, 256].map((value) => ({
        ...streamReport,
        streamFailure: { ...streamFailure, [key]: value },
      })),
    ),
    ...["producerStderrTruncated", "consumerStderrTruncated"].flatMap((key) =>
      [null, 0, 1, "false"].map((value) => ({
        ...streamReport,
        streamFailure: { ...streamFailure, [key]: value },
      })),
    ),
    JSON.stringify(streamReport) + "\n" + JSON.stringify(streamReport),
    JSON.stringify(streamReport).replace(
      '"consumerExit":4',
      '"consumerExit":NaN',
    ),
    JSON.stringify(streamReport).replace(
      '"consumerExit":4',
      '"consumerExit":Infinity',
    ),
    JSON.stringify(streamReport).replace(
      '"consumerExit":4',
      '"consumerExit":4,"consumerExit":5',
    ),
    " ".repeat(1048577) + JSON.stringify(streamReport),
  ];
  for (const value of invalidStreams) {
    const result = execute(value);
    assert.equal(result.stdout, "");
    assert.equal(result.stderr, "");
  }
  const streamNative = spawnSync(
    "python3",
    [
      "-c",
      `
import ast,importlib.util,sys,tempfile
from pathlib import Path
sys.dont_write_bytecode=True
path=Path('scripts/lib/pipeline-remote/current-academic-preparation.py')
spec=importlib.util.spec_from_file_location('controller',path)
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
original=m.Controller
class Failure:
    def __init__(self):
        self.temp=tempfile.TemporaryDirectory();self.point=Path(self.temp.name);self.exits=0
    def prepare(self):
        producer=[sys.executable,'-c','import sys;sys.stdout.buffer.write(b"data")']
        message='pg_restore: error: could not execute query: ERROR: role '+chr(34)+'private-sentinel'+chr(34)+' does not exist'+chr(10)
        consumer=[sys.executable,'-c','import sys;sys.stdin.buffer.read();sys.stderr.write('+repr(message)+');raise SystemExit(4)']
        original.stream(self,producer,consumer,stage='restore_archive')
m.Controller=Failure;m.__name__='__main__';sys.argv=['controller','prepare']
entry=ast.parse(path.read_text()).body[-1]
exec(compile(ast.Module(body=[entry],type_ignores=[]),str(path),'exec'),m.__dict__)
`,
    ],
    { cwd: root, encoding: "utf8" },
  );
  assert.equal(streamNative.status, 1);
  assert.equal(streamNative.stdout, "");
  assert.ok(!streamNative.stderr.includes("private-sentinel"));
  assert.deepEqual(JSON.parse(execute(streamNative.stderr).stdout), {
    schemaVersion: 1,
    error: "stream_failed",
    streamFailure: { ...streamFailure, consumerStderrTruncated: false },
  });
  for (const value of [
    "raw PostgreSQL error with private row sentinel",
    { schemaVersion: 1, error: "postgresql://private-sentinel" },
    { schemaVersion: 1, error: "child_failed", rows: "private-sentinel" },
    {
      schemaVersion: 1,
      error: "copy_migration_refused",
      migrationRefusal: { ...diagnostic, version: 151 },
    },
    {
      schemaVersion: 1,
      error: "copy_migration_refused",
      migrationRefusal: { ...diagnostic, payload: "private-sentinel" },
    },
    {
      schemaVersion: 1,
      error: "copy_migration_refused",
      migrationRefusal: { ...diagnostic, code: "unknown-private-sentinel" },
    },
  ]) {
    const result = execute(value);
    assert.equal(result.stdout, "");
    assert.equal(result.stderr, "");
  }
});
