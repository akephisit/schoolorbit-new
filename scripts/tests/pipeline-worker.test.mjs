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

const helper = path.resolve(
  import.meta.dirname,
  "../prepare_frontend_worker.mjs",
);
function fixture(t, component, incomplete = false) {
  const root = mkdtempSync(path.join(os.tmpdir(), "pipeline-worker-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const directory = path.join(root, component);
  for (const child of [
    "build/client",
    ".svelte-kit/cloudflare-tmp",
    "node_modules/wrangler/bin",
  ]) {
    mkdirSync(path.join(directory, child), { recursive: true });
  }
  writeFileSync(
    path.join(directory, "build/index.js"),
    "import './../.svelte-kit/cloudflare-tmp/server.js';",
  );
  writeFileSync(path.join(directory, "build/client/asset.js"), "browser asset");
  writeFileSync(
    path.join(directory, ".svelte-kit/cloudflare-tmp/server.js"),
    "adapter dependency",
  );
  writeFileSync(
    path.join(directory, "wrangler.json"),
    JSON.stringify({
      name: "fixture",
      main: "build/index.js",
      build: { command: "must not rebuild" },
      assets: { directory: "build/client", binding: "ASSETS" },
    }),
  );
  // Keep the test offline. The CLI double checks the same filesystem boundary
  // as the real Wrangler dry-run, including refusal of incomplete adapter code.
  writeFileSync(
    path.join(directory, "node_modules/wrangler/bin/wrangler.js"),
    `
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
assert.equal(args[0], 'deploy'); assert.ok(args.includes('--dry-run'));
const config = JSON.parse(fs.readFileSync(args[args.indexOf('--config') + 1], 'utf8'));
assert.equal(config.build, undefined);
const first = path.isAbsolute(config.main);
const main = path.resolve(config.main);
assert.ok(fs.existsSync(path.resolve(config.assets.directory)));
if (first) {
  assert.ok(fs.existsSync(path.resolve('.svelte-kit/cloudflare-tmp/server.js')));
} else {
  assert.equal(fs.existsSync('.svelte-kit'), false);
  assert.equal(fs.existsSync('node_modules'), false);
  assert.doesNotMatch(fs.readFileSync(main, 'utf8'), /cloudflare-tmp/);
  assert.equal(fs.readFileSync('build/client/asset.js', 'utf8'), 'browser asset');
}
const outdir = args[args.indexOf('--outdir') + 1];
fs.mkdirSync(outdir, { recursive: true });
fs.writeFileSync(path.join(outdir, 'index.js'), ${JSON.stringify(incomplete ? "import './../.svelte-kit/cloudflare-tmp/server.js';" : "export default {fetch() {}};")});
fs.writeFileSync(path.join(outdir, 'README.md'), 'generated at ' + Date.now());
if (first) fs.writeFileSync(path.join(outdir, 'index.js.map'), JSON.stringify({
  version:3, sourceRoot:outdir, sources:[path.relative(outdir, path.resolve('.svelte-kit/cloudflare-tmp/server.js'))], sourcesContent:['adapter dependency'], mappings:''
}));
fs.appendFileSync(${JSON.stringify(path.join(root, "calls"))}, first ? 'bundle\\n' : 'isolated\\n');
`,
  );
  return {
    root,
    directory,
    run: () =>
      spawnSync(process.execPath, [helper, component], {
        cwd: root,
        encoding: "utf8",
      }),
  };
}
for (const component of ["frontend-admin", "frontend-school"]) {
  test(`${component} bundles once and verifies an isolated artifact without adapter sources`, (t) => {
    const f = fixture(t, component);
    const result = f.run();
    assert.equal(result.status, 0, result.stderr);
    assert.equal(
      readFileSync(path.join(f.root, "calls"), "utf8"),
      "bundle\nisolated\n",
    );
    assert.doesNotMatch(
      readFileSync(path.join(f.directory, "build/index.js"), "utf8"),
      /cloudflare-tmp/,
    );
    assert.equal(
      existsSync(path.join(f.directory, ".wrangler/pipeline-build.json")),
      false,
    );
    assert.equal(existsSync(path.join(f.directory, "build/README.md")), false);
    const map = JSON.parse(readFileSync(path.join(f.directory, "build/index.js.map"), "utf8"));
    assert.equal(map.sourceRoot, "..");
    assert.deepEqual(map.sources, [".svelte-kit/cloudflare-tmp/server.js"]);
  });
}
test("an incomplete adapter bundle refuses preparation instead of publishing a broken artifact", (t) => {
  const f = fixture(t, "frontend-admin", true);
  assert.notEqual(f.run().status, 0);
  assert.equal(readFileSync(path.join(f.root, "calls"), "utf8"), "bundle\n");
  assert.equal(
    existsSync(path.join(f.directory, ".wrangler/pipeline-build.json")),
    false,
  );
});
