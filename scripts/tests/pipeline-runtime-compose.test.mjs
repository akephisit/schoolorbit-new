import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const repo = path.resolve(import.meta.dirname, "../..");
const checksum = (file) =>
  createHash("sha256").update(readFileSync(file)).digest("hex");
function fixture(t, component) {
  assert.notEqual(
    process.getuid(),
    0,
    "permission regressions require an unprivileged process",
  );
  const stack = mkdtempSync(path.join(os.tmpdir(), "runtime-compose-"));
  const writable = (directory) => {
    chmodSync(directory, 0o755);
    for (const entry of readdirSync(directory, { withFileTypes: true }))
      if (entry.isDirectory()) writable(path.join(directory, entry.name));
  };
  t.after(() => {
    writable(stack);
    rmSync(stack, { recursive: true, force: true });
  });
  const code = path.join(stack, "bundle"),
    state = path.join(stack, "deployment"),
    bin = path.join(stack, "bin");
  for (const directory of [code, state, bin]) mkdirSync(directory);
  const source = path.join(code, "podman-compose.yml"),
    active = path.join(stack, "podman-compose.yml");
  writeFileSync(source, "services:\n  fixture:\n    image: immutable-test\n", {
    mode: 0o444,
  });
  chmodSync(source, 0o444);
  writeFileSync(path.join(code, "render.sh"), "#!/bin/bash\nexit 0\n", {
    mode: 0o555,
  });
  chmodSync(path.join(code, "render.sh"), 0o555);
  chmodSync(code, 0o555);
  writeFileSync(active, "previous active runtime\n", { mode: 0o644 });
  const hash = checksum(source);
  writeFileSync(
    path.join(bin, "podman-compose"),
    `#!/bin/bash
set -eu
[[ $# == 6 && $1 == -f && $2 == "$RUNTIME_ACTIVE.next" && $3 == --dry-run && $4 == up && $5 == -d && $6 == "$RUNTIME_COMPONENT" ]] || exit 64
cmp -s "$2" "$RUNTIME_SOURCE" || exit 65
printf '%s\\n' "$*" >> "$RUNTIME_STACK/compose-calls"
if [[ -f $RUNTIME_STACK/fail-dry-run ]]; then rm "$RUNTIME_STACK/fail-dry-run"; exit 23; fi
`,
    { mode: 0o755 },
  );
  const owner =
    component === "backend-admin"
      ? "admin-deploy.sh"
      : "school-deploy-backend.sh";
  const implementation = readFileSync(
    path.join(repo, "scripts/lib/pipeline-remote", owner),
    "utf8",
  );
  const start = implementation.indexOf(
    'install -m 0755 "$renderer_source" "$renderer"',
  );
  const final = 'mv "${runtime_compose}.next" "$runtime_compose"';
  const end = implementation.indexOf(final, start);
  assert.ok(
    start >= 0 && end > start,
    "execute the complete actual runtime staging block",
  );
  const block = implementation.slice(start, end + final.length);
  const run = () =>
    spawnSync(
      "bash",
      [
        "-euc",
        `umask 077
export_school_compose_env(){ :; }
unset_school_compose_env(){ :; }
${block}`,
      ],
      {
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: bin + ":" + process.env.PATH,
          runtime_source: source,
          runtime_compose: active,
          renderer_source: path.join(code, "render.sh"),
          renderer: path.join(state, "render-nginx"),
          RUNTIME_SOURCE: source,
          RUNTIME_ACTIVE: active,
          RUNTIME_COMPONENT: component,
          RUNTIME_STACK: stack,
        },
      },
    );
  const unchanged = () => {
    assert.equal(checksum(source), hash);
    assert.equal(statSync(source).mode & 0o777, 0o444);
    assert.equal(statSync(code).mode & 0o777, 0o555);
    assert.equal(statSync(path.join(code, "render.sh")).mode & 0o777, 0o555);
  };
  return { stack, source, active, run, unchanged };
}
for (const component of ["backend-admin", "backend-school"]) {
  test(`${component} retries a failed actual runtime dry-run without changing the immutable source`, (t) => {
    const f = fixture(t, component);
    writeFileSync(path.join(f.stack, "fail-dry-run"), "");
    assert.equal(f.run().status, 23);
    assert.equal(statSync(f.active + ".next").mode & 0o777, 0o644);
    assert.equal(readFileSync(f.active, "utf8"), "previous active runtime\n");
    assert.deepEqual(readFileSync(f.active + ".next"), readFileSync(f.source));
    f.unchanged();
    const retried = f.run();
    assert.equal(retried.status, 0, retried.stderr);
    assert.equal(statSync(f.active).mode & 0o777, 0o644);
    assert.deepEqual(readFileSync(f.active), readFileSync(f.source));
    assert.equal(existsSync(f.active + ".next"), false);
    assert.equal(
      readFileSync(path.join(f.stack, "compose-calls"), "utf8")
        .trim()
        .split("\n").length,
      2,
    );
    f.unchanged();
  });
  for (const mode of [0o400, 0o444])
    test(`${component} recovers a stranded ${mode.toString(8)} runtime staging file`, (t) => {
      const f = fixture(t, component);
      writeFileSync(f.active + ".next", "stranded prior bytes\n", { mode });
      chmodSync(f.active + ".next", mode);
      assert.equal(statSync(f.active + ".next").mode & 0o777, mode);
      const result = f.run();
      assert.equal(result.status, 0, result.stderr);
      assert.equal(statSync(f.active).mode & 0o777, 0o644);
      assert.deepEqual(readFileSync(f.active), readFileSync(f.source));
      assert.equal(existsSync(f.active + ".next"), false);
      f.unchanged();
    });
}
