import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  createReleaseBundle,
  releaseBundleFiles,
} from "../lib/pipeline-release-bundle.mjs";

const repo = path.resolve(import.meta.dirname, "../..");
const bootstrap = path.join(
  repo,
  "scripts/lib/pipeline-remote/install-release-bundle.sh",
);
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
function writable(directory) {
  if (!existsSync(directory)) return;
  chmodSync(directory, 0o700);
  for (const entry of readdirSync(directory, { withFileTypes: true }))
    if (entry.isDirectory()) writable(path.join(directory, entry.name));
}
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "release-bundle-"));
  t.after(() => {
    writable(root);
    rmSync(root, { recursive: true, force: true });
  });
  const source = path.join(root, "source"),
    stack = path.join(root, "stack");
  mkdirSync(path.join(stack, "deployment"), { recursive: true, mode: 0o700 });
  const files = [
    ...releaseBundleFiles(repo),
    "scripts/lib/pipeline-remote/install-release-bundle.sh",
  ];
  for (const name of files) {
    mkdirSync(path.dirname(path.join(source, name)), { recursive: true });
    cpSync(path.join(repo, name), path.join(source, name));
  }
  const git = (...args) =>
    execFileSync("git", ["-C", source, ...args], {
      encoding: "utf8",
      stdio: "pipe",
    }).trim();
  git("init", "-q");
  git("config", "user.name", "Fixture");
  git("config", "user.email", "fixture@example.invalid");
  git("add", ".");
  git("commit", "-qm", "release implementation");
  const sha = git("rev-parse", "HEAD"),
    bundle = createReleaseBundle(source, sha);
  function install(bytes = bundle.archive, extra = {}) {
    const digest = sha256(bytes);
    const incoming = path.join(
      stack,
      "deployment/release-bundle-incoming",
      sha,
      digest,
      "123-1",
    );
    mkdirSync(incoming, { recursive: true, mode: 0o700 });
    writeFileSync(path.join(incoming, "release-bundle.tar"), bytes);
    const result = spawnSync("bash", [bootstrap], {
      encoding: "utf8",
      env: {
        ...process.env,
        SCHOOLORBIT_STACK_ROOT: stack,
        RELEASE_SHA: sha,
        RELEASE_BUNDLE_SHA256: digest,
        BUNDLE_RUN_ID: "123",
        BUNDLE_RUN_ATTEMPT: "1",
        ...extra,
      },
    });
    return {
      ...result,
      digest,
      destination: path.join(stack, "deployment/release-bundles", sha, digest),
    };
  }
  return { root, source, stack, git, sha, bundle, install };
}
test("archive is deterministic and reads exact committed regular-file bytes", (t) => {
  const f = fixture(t);
  writeFileSync(
    path.join(f.source, "scripts/smoke_test.sh"),
    "dirty-private-sentinel",
  );
  const again = createReleaseBundle(f.source, f.sha);
  assert.deepEqual(again, f.bundle);
  assert.ok(!again.archive.includes(Buffer.from("dirty-private-sentinel")));
  assert.throws(
    () => createReleaseBundle(f.source, "a".repeat(40)),
    /checkout differs/,
  );
});
test("publication and verified replay preserve shared journals and secrets", (t) => {
  const f = fixture(t),
    journal = path.join(f.stack, "deployment/releases", f.sha);
  mkdirSync(journal, { recursive: true });
  writeFileSync(path.join(journal, "probe-token"), "private-probe-sentinel", {
    mode: 0o600,
  });
  writeFileSync(path.join(f.stack, ".env"), "private-env-sentinel", {
    mode: 0o600,
  });
  const first = f.install();
  assert.equal(first.status, 0, first.stderr);
  assert.equal(statSync(first.destination).mode & 0o777, 0o555);
  assert.equal(
    statSync(path.join(first.destination, "scripts/smoke_test.sh")).mode &
      0o777,
    0o555,
  );
  assert.equal(f.install().status, 0);
  assert.equal(
    readFileSync(path.join(journal, "probe-token"), "utf8"),
    "private-probe-sentinel",
  );
  assert.equal(
    readFileSync(path.join(f.stack, ".env"), "utf8"),
    "private-env-sentinel",
  );
  assert.doesNotMatch(
    first.stdout + first.stderr,
    /private-(probe|env)-sentinel/,
  );
});
function rewrite(bytes, mutation) {
  return execFileSync(
    "python3",
    [
      "-c",
      `import io,sys,tarfile,json\nraw=sys.stdin.buffer.read()\nsource=tarfile.open(fileobj=io.BytesIO(raw))\nentries=[(m,source.extractfile(m).read()) for m in source.getmembers()]\n${mutation}\nout=io.BytesIO()\nwith tarfile.open(fileobj=out,mode='w',format=tarfile.USTAR_FORMAT) as target:\n for member,body in entries:\n  member.size=len(body)\n  target.addfile(member,io.BytesIO(body))\nsys.stdout.buffer.write(out.getvalue())`,
    ],
    { input: bytes, maxBuffer: 8 * 1024 * 1024 },
  );
}
for (const [name, mutation] of Object.entries({
  traversal: "entries[0][0].name='../escape'",
  absolute: "entries[0][0].name='/tmp/escape'",
  duplicate: "entries[-1]=entries[0]",
  symlink:
    "entries[0][0].type=tarfile.SYMTYPE; entries[0][0].linkname='../escape'",
  hardlink:
    "entries[0][0].type=tarfile.LNKTYPE; entries[0][0].linkname=entries[1][0].name",
  special: "entries[0][0].type=tarfile.FIFOTYPE",
  mode: "entries[0][0].mode=0o777",
  content: "entries[0]=(entries[0][0],b'changed')",
  omitted: "entries.pop(0)",
  extra:
    "import copy\nm=copy.copy(entries[0][0]); m.name='scripts/extra.sh'; entries.append((m,b'extra'))",
  identity:
    "m,b=entries[-1]; d=json.loads(b); d['releaseSha']='f'*40; entries[-1]=(m,json.dumps(d).encode())",
  duplicateJson:
    'm,b=entries[-1]; entries[-1]=(m,b.replace(b\'"schemaVersion":1\',b\'"schemaVersion":1,"schemaVersion":1\'))',
  oversized: "entries[0]=(entries[0][0],b'x'*(2*1024*1024+1))",
}))
  test(`untrusted archive is refused before publication: ${name}`, (t) => {
    const f = fixture(t),
      result = f.install(rewrite(f.bundle.archive, mutation));
    assert.notEqual(result.status, 0);
    assert.equal(existsSync(result.destination), false);
  });
for (const mode of ["changed", "extra-directory", "symlink"])
  test(`published bundle replay refuses ${mode}`, (t) => {
    const f = fixture(t),
      first = f.install();
    assert.equal(first.status, 0, first.stderr);
    if (mode === "changed") {
      const file = path.join(first.destination, "scripts/smoke_test.sh");
      chmodSync(file, 0o755);
      writeFileSync(file, "changed");
      chmodSync(file, 0o555);
    } else {
      chmodSync(first.destination, 0o755);
      if (mode === "extra-directory")
        mkdirSync(path.join(first.destination, "unknown"), { mode: 0o555 });
      else symlinkSync("/tmp", path.join(first.destination, "unknown"));
      chmodSync(first.destination, 0o555);
    }
    assert.notEqual(f.install().status, 0);
  });
test("transport identity mismatch and symlinked publication ancestors fail closed", (t) => {
  const f = fixture(t);
  assert.notEqual(
    f.install(f.bundle.archive, { RELEASE_BUNDLE_SHA256: "e".repeat(64) })
      .status,
    0,
  );
  const outside = path.join(f.root, "outside");
  mkdirSync(outside);
  symlinkSync(outside, path.join(f.stack, "deployment/release-bundles"));
  assert.notEqual(f.install().status, 0);
  assert.deepEqual(readdirSync(outside), []);
});
test("immutable callers require matching identity; legacy callers keep stable state", (t) => {
  const f = fixture(t),
    result = f.install();
  assert.equal(result.status, 0, result.stderr);
  const code = path.join(
    result.destination,
    "scripts/lib/pipeline-remote/maintenance.sh",
  );
  const run = (caller, extra = {}) =>
    spawnSync(
      "bash",
      [
        "-euc",
        '. "$1"; schoolorbit_release_paths "$2"; printf "%s\\n%s\\n" "$schoolorbit_code_root" "$schoolorbit_state_root"',
        "fixture",
        path.join(repo, "scripts/lib/pipeline-remote/release-paths.sh"),
        caller,
      ],
      {
        encoding: "utf8",
        env: { ...process.env, SCHOOLORBIT_STACK_ROOT: f.stack, ...extra },
      },
    );
  assert.notEqual(run(code).status, 0);
  assert.notEqual(
    run(code, { RELEASE_SHA: f.sha, RELEASE_BUNDLE_SHA256: "e".repeat(64) })
      .status,
    0,
  );
  const matching = run(code, {
    RELEASE_SHA: f.sha,
    RELEASE_BUNDLE_SHA256: result.digest,
  });
  assert.equal(matching.status, 0, matching.stderr);
  assert.deepEqual(matching.stdout.trim().split("\n"), [
    result.destination,
    path.join(f.stack, "deployment"),
  ]);
  const legacy = run(
    path.join(repo, "scripts/lib/pipeline-remote/maintenance.sh"),
  );
  assert.equal(legacy.status, 0);
  assert.deepEqual(
    legacy.stdout.trim().split("\n"),
    Array(2).fill(path.join(f.stack, "deployment")),
  );
});
