import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { deploymentLock } from "../deployment_tooling.mjs";

const cli = path.resolve(import.meta.dirname, "../deployment_tooling.mjs");
const entry = (version, extra = {}) => ({
  version,
  resolved: `https://registry.npmjs.org/fixture/-/fixture-${version}.tgz`,
  integrity: "sha512-disposable-fixture",
  ...extra,
});
function lock() {
  return {
    lockfileVersion: 3,
    packages: {
      "": {},
      "node_modules/wrangler": entry("4.0.0", {
        dependencies: { shared: "^1" },
        peerDependencies: { peer: "^1", optional: "^1" },
        peerDependenciesMeta: { optional: { optional: true } },
        bin: { wrangler: "bin/cli.js" },
      }),
      "node_modules/playwright": entry("1.0.0", {
        dependencies: { shared: "^2" },
      }),
      "node_modules/glob": entry("13.0.0"),
      "node_modules/shared": entry("1.0.0"),
      "node_modules/playwright/node_modules/shared": entry("2.0.0"),
      "node_modules/peer": entry("1.0.0"),
      "node_modules/app-only": { link: true, resolved: "../private-workspace" },
    },
  };
}
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "deployment-tooling-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const app = path.join(root, "frontend-school"),
    prefix = path.join(root, "tools");
  mkdirSync(app);
  const original = lock();
  writeFileSync(path.join(app, "package-lock.json"), JSON.stringify(original));
  writeFileSync(path.join(app, ".env"), "not-a-package\n");
  const run = (command, extra = {}) =>
    spawnSync(process.execPath, [cli, command, "frontend-school", prefix], {
      cwd: root,
      env: { ...process.env, GITHUB_OUTPUT: "", ...extra },
      encoding: "utf8",
    });
  return {
    root,
    app,
    prefix,
    original,
    run,
    tools: path.join(prefix, "frontend-school"),
  };
}
function installFixture(f, omitted = new Set()) {
  for (const [location, item] of Object.entries(f.original.packages)) {
    if (!location || item.link || omitted.has(location)) continue;
    mkdirSync(path.join(f.app, location), { recursive: true });
    writeFileSync(
      path.join(f.app, location, "package.json"),
      JSON.stringify({ version: item.version }),
    );
  }
  const bin = path.join(f.app, "node_modules/wrangler/bin");
  mkdirSync(bin);
  writeFileSync(path.join(bin, "cli.js"), "console.log('locked-cli');\n");
}

function optionalBranch(original) {
  original.packages["node_modules/optional"] = entry("1.0.0", {
    dependencies: { "optional-child": "^1" },
  });
  original.packages["node_modules/optional-child"] = entry("1.0.0", {
    dependencies: { optional: "^1" },
  });
}
test("deployment closure preserves locked nested versions and peers while excluding application workspace links", () => {
  const original = lock(),
    result = deploymentLock(original, "frontend-school");
  assert.equal(
    result.lock.packages["node_modules/shared"],
    original.packages["node_modules/shared"],
  );
  assert.equal(
    result.lock.packages["node_modules/playwright/node_modules/shared"].version,
    "2.0.0",
  );
  assert.ok(result.lock.packages["node_modules/peer"]);
  assert.equal(result.lock.packages["node_modules/app-only"], undefined);
  assert.deepEqual(result.manifest.dependencies, {
    wrangler: "4.0.0",
    playwright: "1.0.0",
    glob: "13.0.0",
  });
});
test("required missing, unpinned and nonportable dependencies refuse tooling preparation", () => {
  for (const change of [
    (value) => {
      delete value.packages["node_modules/shared"];
    },
    (value) => {
      delete value.packages["node_modules/peer"];
    },
    (value) => {
      value.packages["node_modules/wrangler"].link = true;
    },
    (value) => {
      delete value.packages["node_modules/wrangler"].integrity;
    },
    (value) => {
      value.packages["node_modules/wrangler"].dependencies["../outside"] = "1";
    },
  ]) {
    const value = lock();
    change(value);
    assert.throws(() => deploymentLock(value, "frontend-school"));
  }
});
test("optional peers and their descendants inherit deployment reachability instead of app-wide required flags", () => {
  const original = lock();
  optionalBranch(original);
  const result = deploymentLock(original, "frontend-school");
  for (const name of ["optional", "optional-child"]) {
    const location = `node_modules/${name}`;
    assert.equal(result.lock.packages[location].optional, true);
    assert.equal(original.packages[location].optional, undefined);
    for (const key of ["version", "resolved", "integrity"])
      assert.equal(
        result.lock.packages[location][key],
        original.packages[location][key],
      );
  }
  assert.equal(result.lock.packages["node_modules/shared"].optional, undefined);
});
test("a later required path upgrades an optional branch and its cyclic descendants", () => {
  const original = lock();
  optionalBranch(original);
  original.packages["node_modules/playwright"].dependencies.optional = "^1";
  original.packages["node_modules/optional"].optional = true;
  original.packages["node_modules/optional-child"].optional = true;
  const result = deploymentLock(original, "frontend-school");
  for (const name of ["optional", "optional-child"])
    assert.equal(result.lock.packages[`node_modules/${name}`].optional, false);
});
test("unrelated frontend dependencies do not invalidate deployment reuse, but locked tooling does", (t) => {
  const f = fixture(t),
    first = f.run("prepare");
  assert.equal(first.status, 0, first.stderr);
  f.original.packages["node_modules/unrelated"] = entry("99.0.0");
  writeFileSync(
    path.join(f.app, "package-lock.json"),
    JSON.stringify(f.original),
  );
  assert.equal(
    f.run("prepare").stdout.split("\n")[0],
    first.stdout.split("\n")[0],
  );
  f.original.packages["node_modules/wrangler"].version = "4.1.0";
  writeFileSync(
    path.join(f.app, "package-lock.json"),
    JSON.stringify(f.original),
  );
  assert.notEqual(
    f.run("prepare").stdout.split("\n")[0],
    first.stdout.split("\n")[0],
  );
});
test("seeded cache retains exact versions and runnable bin links, and refuses a different platform identity", (t) => {
  const f = fixture(t);
  installFixture(f);
  assert.equal(f.run("prepare").status, 0);
  assert.equal(f.run("seed").stdout, "seeded=true\n");
  assert.equal(f.run("mark").status, 0);
  assert.equal(f.run("validate").stdout, "valid=true\n");
  assert.equal(
    f.run("validate", { ImageOS: "different-image" }).stdout,
    "valid=false\n",
  );
  assert.equal(existsSync(path.join(f.tools, ".env")), false);
  assert.equal(existsSync(path.join(f.tools, "node_modules/app-only")), false);
  const result = spawnSync(
    process.execPath,
    [path.join(f.tools, "node_modules/.bin/wrangler")],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "locked-cli");
  assert.notEqual(
    f.run("link").status,
    0,
    "cannot replace another dependency owner",
  );
  writeFileSync(
    path.join(f.tools, "node_modules/glob/package.json"),
    '{"version":"wrong"}',
  );
  assert.equal(f.run("validate").stdout, "valid=false\n");
});
test("minimal installs permit absent optional branches but reject wrong or malformed present packages and missing required packages", (t) => {
  const f = fixture(t);
  optionalBranch(f.original);
  writeFileSync(
    path.join(f.app, "package-lock.json"),
    JSON.stringify(f.original),
  );
  installFixture(
    f,
    new Set(["node_modules/optional", "node_modules/optional-child"]),
  );
  assert.equal(f.run("prepare").status, 0);
  assert.equal(f.run("seed").stdout, "seeded=true\n");
  assert.equal(f.run("mark").status, 0);
  assert.equal(f.run("validate").stdout, "valid=true\n");
  const optional = path.join(f.tools, "node_modules/optional");
  mkdirSync(optional);
  writeFileSync(path.join(optional, "package.json"), '{"version":"wrong"}');
  assert.notEqual(f.run("mark").status, 0);
  assert.equal(f.run("validate").stdout, "valid=false\n");
  writeFileSync(path.join(optional, "package.json"), "{invalid-json");
  assert.notEqual(f.run("mark").status, 0);
  rmSync(optional, { recursive: true });
  rmSync(path.join(f.tools, "node_modules/shared"), { recursive: true });
  assert.notEqual(f.run("mark").status, 0);
});
test("partial caches fall back to install instead of claiming a hit or copying app files", (t) => {
  const f = fixture(t);
  assert.equal(f.run("prepare").status, 0);
  assert.equal(f.run("validate").stdout, "valid=false\n");
  assert.equal(f.run("seed").stdout, "seeded=false\n");
  assert.notEqual(f.run("mark").status, 0);
  assert.equal(
    readFileSync(path.join(f.tools, "package.json"), "utf8").includes(
      "app-only",
    ),
    false,
  );
});
