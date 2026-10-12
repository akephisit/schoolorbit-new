import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const repo = path.resolve(import.meta.dirname, "../..");
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "build-evidence-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}
test("build summary separates parallel phases, exposes library hits, and labels reused compiler evidence", (t) => {
  const root = fixture(t);
  writeFileSync(
    path.join(root, "build-metrics.json"),
    JSON.stringify({ compileNanoseconds: 190e9 }),
  );
  writeFileSync(
    path.join(root, "sccache-stats.json"),
    JSON.stringify({
      cache_location: "private-diagnostic-sentinel",
      stats: {
        cache_hits: { counts: { Rust: 30 } },
        cache_misses: { counts: { Rust: 0 } },
        cache_write_errors: 0,
      },
    }),
  );
  writeFileSync(
    path.join(root, "cargo-timing.html"),
    "const UNIT_DATA = " +
      JSON.stringify(
        Array.from({ length: 5000 }, (_, index) => ({
          name: "crate-" + "x".repeat(300),
          target: "bin",
          duration: index,
        })),
      ) +
      ";",
  );
  const log = path.join(root, "buildkit.log");
  writeFileSync(
    log,
    "#3 importing cache manifest from gha\n#3 DONE 0.5s\n#4 [builder 7/7] RUN bash scripts/build_release.sh\n#4 CACHED\n#5 exporting to image\n#5 DONE 2.0s\n#6 exporting cache\n#6 DONE 1.0s\n",
  );
  const result = spawnSync(
    "python3",
    [
      path.join(repo, "scripts/render_backend_build_summary.py"),
      "backend-school",
      root,
      log,
    ],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compiler layer: \*\*cached\*\*/);
  assert.match(result.stdout, /Recorded Cargo invocation: 190.00s/);
  assert.match(result.stdout, /\| hits \| 30 \|/);
  assert.match(result.stdout, /\| cache export \| 1.00s \|/);
  assert.match(result.stdout, /do not sum/);
  assert.doesNotMatch(result.stdout, /private-diagnostic-sentinel/);
  assert.ok(Buffer.byteLength(result.stdout) < 4096);
});
test("missing compiler evidence is reported unavailable instead of zero hits", (t) => {
  const root = fixture(t);
  const result = spawnSync(
    "python3",
    [
      path.join(repo, "scripts/render_backend_build_summary.py"),
      "backend-admin",
      root,
      path.join(root, "absent"),
    ],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compiler layer: \*\*unconfirmed\*\*/);
  assert.match(result.stdout, /\| hits \| unavailable \|/);
});
for (const mode of ["off", "gha"])
  test(`compiler failure cannot be hidden by optional Admin cache statistics (${mode})`, (t) => {
    const root = fixture(t),
      bin = path.join(root, "bin");
    mkdirSync(bin);
    writeFileSync(path.join(bin, "cargo"), "#!/bin/sh\nexit 42\n", {
      mode: 0o755,
    });
    writeFileSync(path.join(bin, "sccache"), "#!/bin/sh\nexit 0\n", {
      mode: 0o755,
    });
    const secrets = path.join(root, "secrets");
    mkdirSync(secrets);
    for (const name of ["sccache_gha_url", "sccache_gha_token"])
      writeFileSync(path.join(secrets, name), "disposable-fixture");
    const dockerfile = readFileSync(
      path.join(repo, "backend-admin/Dockerfile"),
      "utf8",
    );
    const run = dockerfile
      .match(
        /RUN --mount=type=secret,id=sccache_gha_url[\s\S]+?(?=\n\nFROM scratch)/,
      )[0]
      .replace(/^RUN /, "")
      .replace(/--mount=\S+\s*/g, "")
      .replaceAll("/run/secrets", secrets)
      .replaceAll("/usr/local/bin/sccache", path.join(bin, "sccache"))
      .replace(/\\\n/g, "\n");
    const result = spawnSync("sh", ["-c", run], {
      cwd: root,
      env: {
        ...process.env,
        PATH: bin + ":" + process.env.PATH,
        SCHOOL_COMPILER_CACHE: mode,
      },
      encoding: "utf8",
    });
    assert.equal(result.status, 42, result.stderr);
    assert.equal(result.stdout, "");
  });
