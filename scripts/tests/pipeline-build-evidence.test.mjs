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
test("lazy cached-layer downloads retain reuse status and materialization duration", (t) => {
  const root = fixture(t);
  writeFileSync(path.join(root, "build-metrics.json"), JSON.stringify({compileNanoseconds: 190e9}));
  const log = path.join(root, "buildkit.log");
  writeFileSync(log, [
    "#22 [builder 5/7] RUN bash scripts/build_release.sh dependencies",
    "#22 CACHED",
    "#22 sha256:fixture 614.89MB / 614.89MB 139.0s done",
    "#22 extracting sha256:fixture 10.3s done",
    "#22 DONE 149.3s",
    "#24 [builder 7/7] RUN bash scripts/build_release.sh",
    "#24 CACHED",
    "#24 DONE 28.2s",
  ].join("\n"));
  const result = spawnSync("python3", [path.join(repo, "scripts/render_backend_build_summary.py"), "backend-school", root, log], {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compiler layer: \*\*cached\*\*/);
  assert.match(result.stdout, /Recorded Cargo invocation: 190.00s/);
  assert.match(result.stdout, /\| dependency layer \| 149.30s \|/);
  assert.match(result.stdout, /\| compiler layer \| 28.20s \|/);
  assert.match(result.stderr, /current_layer=cached/);
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

test("registry restore without CACHED differs from transferred inputs followed by compiler execution", (t) => {
  const root = fixture(t);
  const log = path.join(root, "buildkit.log");
  writeFileSync(log, [
    "#23 [builder 7/7] RUN bash scripts/build_release.sh",
    "#23 sha256:fixture 614.89MB / 614.89MB 18.3s done",
    "#23 extracting sha256:fixture 9.8s done",
    "#23 DONE 28.2s",
  ].join("\n"));
  const args = [path.join(repo, "scripts/render_backend_build_summary.py"), "backend-school", root, log];
  let result = spawnSync("python3", args, {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compiler layer: \*\*cached\*\*/);
  assert.match(result.stdout, /\| compiler layer \| 28.20s \|/);
  writeFileSync(log, readFileSync(log, "utf8").replace("#23 DONE", "#23 28.3 school_compiler_cache phase=application mode=local enabled=true\n#23 DONE"));
  result = spawnSync("python3", args, {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /compiler layer: \*\*executed\*\*/);
});

test("real GHA export headings and dependency timings stay separate from the application", (t) => {
  const root = fixture(t);
  writeFileSync(path.join(root, "dependency-build-metrics.json"), JSON.stringify({compileNanoseconds: 349e9}));
  writeFileSync(path.join(root, "dependency-sccache-stats.json"), JSON.stringify({stats: {
    cache_hits: {counts: {Rust: 100}}, cache_misses: {counts: {}}
  }}));
  const log = path.join(root, "buildkit.log");
  writeFileSync(log, [
    "#19 [builder 5/7] RUN bash scripts/build_release.sh dependencies",
    "#19 CACHED",
    "#23 [builder 7/7] RUN bash scripts/build_release.sh",
    "#23 DONE 317.2s",
    "#30 exporting to GitHub Actions Cache",
    "#30 preparing build cache for export 66.5s done",
    "#30 DONE 120.3s",
  ].join("\n"));
  const result = spawnSync("python3", [path.join(repo, "scripts/render_backend_build_summary.py"), "backend-school", root, log], {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Recorded dependency cook: 349.00s/);
  assert.match(result.stdout, /Dependency Rust library cache: hits=100, misses=0/);
  assert.match(result.stdout, /\| dependency layer \| 0.00s \|/);
  assert.match(result.stdout, /\| compiler layer \| 317.20s \|/);
  assert.match(result.stdout, /\| cache export \| 120.30s \|/);
  assert.match(result.stderr, /current_layer=executed/);
  assert.match(result.stderr, /dependency_layer=0.00s compiler_layer=317.20s.*cache_export=120.30s/);
});

test("an observed empty Rust cache counter is zero while absent counters remain unavailable", (t) => {
  const root = fixture(t);
  writeFileSync(path.join(root, "sccache-stats.json"), JSON.stringify({stats: {
    cache_hits: {counts: {}}, cache_misses: {}, cache_write_errors: 0
  }}));
  const result = spawnSync("python3", [path.join(repo, "scripts/render_backend_build_summary.py"), "backend-school", root, "absent"], {encoding: "utf8"});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /\| hits \| 0 \|/);
  assert.match(result.stdout, /\| misses \| unavailable \|/);
});

for (const phase of ["application", "dependencies"])
  for (const mode of ["off", "local", "gha", "missing-secrets"])
    test(`School ${phase} compiler cache preserves commands and failure status (${mode})`, (t) => {
      const root = fixture(t), bin = path.join(root, "bin"), secrets = path.join(root, "secrets");
      mkdirSync(bin);
      mkdirSync(secrets);
      if (mode === "gha")
        for (const name of ["sccache_gha_url", "sccache_gha_token"])
          writeFileSync(path.join(secrets, name), "private-cache-fixture-sentinel");
      writeFileSync(path.join(bin, "cargo"), '#!/bin/bash\nprintf "%s\\n" "$@" > "$CARGO_ARGS"\nprintf "%s\\n" "${RUSTC_WRAPPER:-off}" "${ACTIONS_CACHE_SERVICE_V2:-off}" "${SCCACHE_GHA_VERSION:-off}" > "$CACHE_ENV"\nexit "$CARGO_EXIT"\n', {mode: 0o755});
      writeFileSync(path.join(bin, "sccache"), '#!/bin/bash\nexit 9\n', {mode: 0o755});
      const script = readFileSync(path.join(repo, "backend-school/scripts/build_release.sh"), "utf8")
        .replaceAll("/run/secrets", secrets).replaceAll("/usr/local/bin/sccache", path.join(bin, "sccache"))
        .replaceAll("/usr/local/bin/rustc-cache", path.join(bin, "rustc-cache"));
      for (const status of [42, 0]) {
        const runRoot = path.join(root, String(status));
        mkdirSync(runRoot);
        const result = spawnSync("bash", ["-c", script, "build_release.sh", phase], {
          cwd: runRoot,
          env: {...process.env, PATH: bin + ":" + process.env.PATH, RUSTC_WRAPPER: "must-be-cleared", SCHOOL_COMPILER_CACHE: mode === "missing-secrets" ? "gha" : mode, CARGO_EXIT: String(status), CARGO_ARGS: path.join(root, "args"), CACHE_ENV: path.join(root, "cache-env")},
          encoding: "utf8"
        });
        assert.equal(result.status, status, result.stderr);
        const args = readFileSync(path.join(root, "args"), "utf8");
        assert.equal(args, phase === "dependencies"
          ? "chef\ncook\n--release\n--recipe-path\nrecipe.json\n"
          : `rustc\n--release\n--locked\n--bin\nbackend-school\n--timings\n--\n-C\nlto=off\n-C\nlinker=${runRoot}/scripts/release_linker.sh\n`);
        const cacheEnv = readFileSync(path.join(root, "cache-env"), "utf8").split("\n");
        assert.equal(cacheEnv[0], ["gha", "local"].includes(mode) ? path.join(bin, "rustc-cache") : "off");
        if (mode === "gha") assert.deepEqual(cacheEnv.slice(1, 3), ["true", "schoolorbit-backend-school"]);
        assert.doesNotMatch(result.stdout + result.stderr, /private-cache-fixture-sentinel/);
        const metricsDir = path.join(runRoot, "target", phase === "dependencies" ? "dependency-timings" : "cargo-timings");
        if (status === 0) {
          const metrics = JSON.parse(readFileSync(path.join(metricsDir, "build-metrics.json"), "utf8"));
          assert.equal(metrics.cacheEnabled, ["gha", "local"].includes(mode));
          assert.ok(metrics.compileNanoseconds >= 0);
          assert.deepEqual(JSON.parse(readFileSync(path.join(metricsDir, "sccache-stats.json"), "utf8")), {});
        } else assert.throws(() => readFileSync(path.join(metricsDir, "build-metrics.json")));
      }
    });

test('API-readable evidence distinguishes executable/link work and current BuildKit phases', t => {
  const root = fixture(t);
  writeFileSync(path.join(root, 'build-metrics.json'), JSON.stringify({compileNanoseconds: 170e9}));
  writeFileSync(path.join(root, 'cargo-timing.html'), 'const UNIT_DATA = ' + JSON.stringify([
    {name: 'school-academic-http', target: 'lib', duration: 3},
    {name: 'backend-school', target: 'bin "backend-school"', duration: 160}
  ]) + ';');
  writeFileSync(path.join(root, 'link-nanoseconds.txt'), '1200000000\n');
  const log = path.join(root, 'buildkit.log');
  writeFileSync(log, '#3 [builder 7/7] RUN bash scripts/build_release.sh\n#3 DONE 171.0s\n#4 exporting to image\n#4 DONE 40.0s\n#5 exporting cache\n#5 DONE 50.0s\n');
  const result = spawnSync('python3', [path.join(repo, 'scripts/render_backend_build_summary.py'), 'backend-school', root, log], {encoding:'utf8'});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /executable unit \(includes link\): 160.00s/);
  assert.match(result.stdout, /final link \(within executable unit\): 1.20s/);
  assert.match(result.stderr, /current_layer=executed.*executable=160.00s link=1.20s/);
  assert.match(result.stderr, /image_export\/push=40.00s cache_export=50.00s/);
});

test('release linker preserves arguments and failure status without reporting a successful link', t => {
  const root = fixture(t), bin = path.join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(path.join(bin, 'cc'), '#!/bin/bash\nprintf "%s\\n" "$@" > "$LINK_ARGUMENTS"\nexit "$LINK_EXIT"\n', {mode:0o755});
  for (const code of [0, 42]) {
    const metrics = path.join(root, `link-${code}.txt`);
    const result = spawnSync('bash', [path.join(repo, 'backend-school/scripts/release_linker.sh'), '-fuse-ld=lld', 'input with spaces.o', '-o', 'app'], {
      env:{...process.env,PATH:bin+':'+process.env.PATH,LINK_ARGUMENTS:path.join(root,'args'),LINK_EXIT:String(code),SCHOOLORBIT_LINK_METRICS:metrics}, encoding:'utf8'
    });
    assert.equal(result.status, code, result.stderr);
    assert.equal(readFileSync(path.join(root, 'args'), 'utf8'), '-fuse-ld=lld\ninput with spaces.o\n-o\napp\n');
    if (code === 0) assert.match(readFileSync(metrics, 'utf8'), /^[0-9]+\n$/);
    else assert.throws(() => readFileSync(metrics));
  }
});

for (const available of [true, false])
  test(`workflow reads the exact builder record and stderr progress; telemetry availability=${available}`, (t) => {
    const root = fixture(t),
      bin = path.join(root, "bin");
    mkdirSync(bin);
    writeFileSync(
      path.join(bin, "docker"),
      `#!/bin/sh
if [ "$*" != "buildx --builder fixture-builder history logs --progress=plain fixture-record" ]; then exit 2; fi
${available ? "printf '#4 [builder 7/7] RUN bash scripts/build_release.sh\\n#4 DONE 1.2s\\n' >&2" : "printf 'fixture record unavailable\\n' >&2; exit 1"}
`,
      { mode: 0o755 },
    );
    const workflow = readFileSync(
      path.join(repo, ".github/workflows/prepare.yml"),
      "utf8",
    );
    const body = workflow
      .match(
        /name: Summarize build phases and compiler reuse[\s\S]*?        run: \|\n([\s\S]*?)(?=      - name: Bind selected image)/,
      )[1]
      .replace(/^          /gm, "");
    const output = path.join(root, "summary.md");
    const result = spawnSync("bash", ["-c", body], {
      cwd: repo,
      env: {
        ...process.env,
        PATH: bin + ":" + process.env.PATH,
        COMPONENT: "backend-school",
        RUNNER_TEMP: root,
        GITHUB_STEP_SUMMARY: output,
        BUILD_METADATA: JSON.stringify({
          "buildx.build.ref": "fixture-builder/fixture-node/fixture-record",
        }),
      },
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const summary = readFileSync(output, "utf8");
    assert.match(
      summary,
      available
        ? /compiler layer: \*\*executed\*\*/
        : /compiler layer: \*\*unconfirmed\*\*/,
    );
    if (available) {
      assert.match(summary, /\| compiler layer \| 1.20s \|/);
      assert.doesNotMatch(result.stdout, /::warning/);
    } else
      assert.match(
        result.stdout,
        /::warning::BuildKit phase logs are unavailable/,
      );
    assert.doesNotMatch(summary, /fixture record unavailable/);
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
