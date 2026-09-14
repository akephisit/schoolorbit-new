import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const resolver = path.resolve(
  import.meta.dirname,
  "../resolve_school_release_scope.sh",
);

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

async function fixture() {
  const root = await mkdtemp(path.join(tmpdir(), "school-release-scope-"));
  run("git", ["init", "--initial-branch=main"], root);
  run("git", ["config", "user.name", "SchoolOrbit Test"], root);
  run("git", ["config", "user.email", "test@schoolorbit.invalid"], root);
  await writeFile(path.join(root, "README.txt"), "baseline\n");
  run("git", ["add", "."], root);
  run("git", ["commit", "-m", "baseline"], root);
  return root;
}

async function commitFile(root, relativePath, contents) {
  await mkdir(path.dirname(path.join(root, relativePath)), { recursive: true });
  await writeFile(path.join(root, relativePath), contents);
  run("git", ["add", relativePath], root);
  run("git", ["commit", "-m", relativePath], root);
  return run("git", ["rev-parse", "HEAD"], root);
}

function resolve(
  root,
  requested,
  release,
  frontendAccepted,
  backendAccepted,
  forceFull = "false",
) {
  const result = spawnSync(
    "bash",
    [
      resolver,
      requested,
      release,
      frontendAccepted,
      backendAccepted,
      forceFull,
    ],
    {
      cwd: root,
      encoding: "utf8",
    },
  );
  return result;
}

test("auto scope includes every queued commit since the last accepted push", async () => {
  const root = await fixture();
  const accepted = run("git", ["rev-parse", "HEAD"], root);
  await commitFile(root, "frontend-school/src/app.ts", "frontend\n");
  const release = await commitFile(
    root,
    "backend-school/src/main.rs",
    "backend\n",
  );

  const result = resolve(root, "auto", release, accepted, accepted);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=full$/m);
  assert.match(result.stdout, /^needs_frontend=true$/m);
  assert.match(result.stdout, /^needs_backend=true$/m);
});

test("auto scope remains narrow when the accepted baseline is trustworthy", async () => {
  const root = await fixture();
  const accepted = run("git", ["rev-parse", "HEAD"], root);
  const release = await commitFile(
    root,
    "frontend-school/src/app.ts",
    "frontend\n",
  );

  const result = resolve(root, "auto", release, accepted, accepted);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=frontend$/m);
  assert.match(result.stdout, /^needs_backend=false$/m);
});

test("Worker inventory helper changes use the frontend release path", async () => {
  const root = await fixture();
  const accepted = run("git", ["rev-parse", "HEAD"], root);
  const release = await commitFile(
    root,
    "scripts/find_worker_release_candidates.mjs",
    "export const helper = true;\n",
  );

  const result = resolve(root, "auto", release, accepted, accepted);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=frontend$/m);
  assert.match(result.stdout, /^needs_backend=false$/m);
});

test("missing or divergent accepted baselines force a full release", async () => {
  const root = await fixture();
  const release = await commitFile(
    root,
    "backend-school/src/main.rs",
    "backend\n",
  );

  const result = resolve(root, "auto", release, "", "");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=full$/m);
});

test("manual scope remains explicit", async () => {
  const root = await fixture();
  const release = run("git", ["rev-parse", "HEAD"], root);

  const result = resolve(root, "backend", release, "", "");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=backend$/m);
  assert.match(result.stdout, /^needs_frontend=false$/m);
});

test("dirty attempts force reconciliation even when a failed frontend change was reverted", async () => {
  const root = await fixture();
  const accepted = run("git", ["rev-parse", "HEAD"], root);
  await commitFile(root, "frontend-school/src/app.ts", "failed frontend\n");
  run("git", ["rm", "frontend-school/src/app.ts"], root);
  await mkdir(path.join(root, "backend-school/src"), { recursive: true });
  await writeFile(path.join(root, "backend-school/src/main.rs"), "backend\n");
  run("git", ["add", "."], root);
  run("git", ["commit", "-m", "revert frontend and change backend"], root);
  const release = run("git", ["rev-parse", "HEAD"], root);

  const result = resolve(root, "auto", release, accepted, accepted, "true");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=full$/m);
});

test("component baselines retain an older backend after a manual frontend release", async () => {
  const root = await fixture();
  const backendAccepted = run("git", ["rev-parse", "HEAD"], root);
  await commitFile(root, "backend-school/src/main.rs", "pending backend\n");
  const frontendAccepted = await commitFile(
    root,
    "frontend-school/src/app.ts",
    "accepted frontend\n",
  );

  const result = resolve(
    root,
    "auto",
    frontendAccepted,
    frontendAccepted,
    backendAccepted,
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^scope=backend$/m);
  assert.match(result.stdout, /^needs_frontend=false$/m);
});
