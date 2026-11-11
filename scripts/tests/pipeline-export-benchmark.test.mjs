import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import test from "node:test";

const repo = path.resolve(import.meta.dirname, "../..");
const { parse: parseYaml } = createRequire(
  path.join(repo, "frontend-school/package.json"),
)("yaml");
test("exporter timeout leaves bounded evidence and cleanup reserves without extending legacy phases", () => {
  const job = parseYaml(
    readFileSync(
      path.join(repo, ".github/workflows/backend-cache-benchmark.yml"),
      "utf8",
    ),
  ).jobs.benchmark;
  const benchmark = job.steps.find(
    (step) => step.run === "python3 -B scripts/benchmark_backend_export.py",
  );
  const upload = job.steps.find(
    (step) => step.name === "Publish complete benchmark evidence",
  );
  const cleanup = job.steps.find(
    (step) =>
      step.name === "Remove only this run's disposable registry package",
  );
  const minutes = (value, phase) => {
    if (Number.isInteger(value)) return value;
    const expression = value.match(
      /^\$\{\{ inputs\.phase == 'exporters' && (\d+) \|\| (\d+) \}\}$/,
    );
    assert.ok(
      expression,
      "timeouts may depend only on the bounded exporter phase",
    );
    return Number(expression[phase === "exporters" ? 1 : 2]);
  };
  assert.equal(benchmark.if, "inputs.phase == 'exporters'");
  assert.equal(minutes(job["timeout-minutes"], "exporters"), 240);
  assert.equal(benchmark["timeout-minutes"], 210);
  assert.equal(minutes(upload["timeout-minutes"], "exporters"), 15);
  assert.equal(minutes(cleanup["timeout-minutes"], "exporters"), 10);
  assert.ok(
    240 -
      benchmark["timeout-minutes"] -
      minutes(upload["timeout-minutes"], "exporters") -
      minutes(cleanup["timeout-minutes"], "exporters") >=
      5,
  );
  assert.match(upload.if, /^always\(\)/);
  assert.match(cleanup.if, /^always\(\).*inputs\.phase == 'exporters'/);
  assert.ok(job.steps.indexOf(benchmark) < job.steps.indexOf(upload));
  assert.ok(job.steps.indexOf(upload) < job.steps.indexOf(cleanup));
  assert.equal(benchmark["continue-on-error"], undefined);
  assert.equal(upload["continue-on-error"], undefined);
  assert.equal(cleanup["continue-on-error"], undefined);
  for (const phase of ["compare", "producer", "consumer", "cleanup"]) {
    const jobMinutes = minutes(job["timeout-minutes"], phase);
    assert.equal(jobMinutes, 120);
    assert.ok(minutes(upload["timeout-minutes"], phase) >= jobMinutes);
    assert.ok(minutes(cleanup["timeout-minutes"], phase) >= jobMinutes);
  }
});
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "export-benchmark-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const [name, content] of Object.entries({
    "backend-school/Cargo.toml":
      '[workspace]\nmembers = [\n]\n[workspace.package]\nversion = "0.1.0"\n',
    "backend-school/Cargo.lock": "version = 4\n",
    "backend-school/src/main.rs": "fn main() {}\n",
    "backend-school/Dockerfile":
      "FROM base AS builder\nRUN --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache true\nRUN --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache true\nFROM fixture@sha256:" +
      "a".repeat(64) +
      " AS runtime\n",
    "contracts/openapi/school-api.json": '{"openapi":"3.1.0","paths":{}}',
  })) {
    mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    writeFileSync(path.join(root, name), content);
  }
  const git = (...args) =>
    execFileSync("git", ["-C", root, ...args], {
      encoding: "utf8",
      stdio: "pipe",
    }).trim();
  git("init", "-q");
  git("config", "user.name", "Fixture");
  git("config", "user.email", "fixture@example.invalid");
  git("add", ".");
  git("commit", "-qm", "fixture");
  return { root, sha: git("rev-parse", "HEAD") };
}
function python(root, code, env = {}) {
  return spawnSync(
    "python3",
    [
      "-B",
      "-c",
      `import sys,os,json,pathlib\nsys.path.insert(0,${JSON.stringify(path.join(repo, "scripts"))})\nimport benchmark_backend_export as e\nroot=pathlib.Path(${JSON.stringify(root)})\n${code}`,
    ],
    {
      cwd: root,
      encoding: "utf8",
      env: { ...process.env, ...env },
      maxBuffer: 4 * 1024 * 1024,
    },
  );
}
test("export plans retain stable dependencies and charge every candidate solve without mutable imports", (t) => {
  const f = fixture(t);
  const result = python(
    f.root,
    `
package='ghcr.io/fixture/schoolorbit-cache-benchmark-123-1'
pinned=package+'@sha256:'+'d'*64
for variant in e.VARIANTS:
 commands,cache=e.build_commands(['docker','buildx','build'],root/'contexts/source-1',package,'source-1-'+variant,variant,pinned)
 assert all(command[command.index('--cache-from')+1]=='type=registry,ref='+pinned for _,command in commands)
 assert all('SCHOOL_CARGO_BUILD_STAGE=snapshot-builder' not in command for _,command in commands)
 assert commands[-1][0]=='runtime' and '--push' in commands[-1][1]
 if variant=='dependencies-min':
  assert len(commands)==2
  assert commands[0][1][commands[0][1].index('--target')+1]=='dependencies'
  assert 'mode=min' in commands[0][1][commands[0][1].index('--cache-to')+1]
  assert '--cache-to' not in commands[1][1]
 else:
  assert len(commands)==1 and 'mode=max' in commands[0][1][commands[0][1].index('--cache-to')+1]
for invalid in [package+':mutable','ghcr.io/another/package@sha256:'+'d'*64]:
 try: e.build_commands([],root,package,'fixture','runtime-max',invalid)
 except ValueError: pass
 else: raise AssertionError('untrusted cache import accepted')
`,
  );
  assert.equal(result.status, 0, result.stderr);
});
test("missing cache digest and incomplete or changed pair evidence invalidate comparison", (t) => {
  const f = fixture(t);
  const result = python(
    f.root,
    `
e.run=lambda command: 'Digest: unavailable'
try: e.cache_identity('ghcr.io/fixture/package:tag')
except ValueError: pass
else: raise AssertionError('missing cache identity accepted')
base={'sourceContextSha256':'source','compilerSnapshotSha256':'compiler','runtime':{'binary':'same'},'api':{'canonicalSha256':'api'},'compiler':{'cargo':{'registryIdentitySha256':'profile','registryArtifactCount':1}}}
pair=[dict(base,variant=variant) for variant in e.VARIANTS]
e.compare_pair(pair)
import copy
for key in ['sourceContextSha256','compilerSnapshotSha256','runtime','api','compiler']:
 changed=copy.deepcopy(pair)
 changed[1][key]= {'runtime':{},'api':{'canonicalSha256':'wrong'},'compiler':{'cargo':{'registryIdentitySha256':'wrong','registryArtifactCount':1}}}.get(key,'wrong')
 try: e.compare_pair(changed)
 except ValueError: pass
 else: raise AssertionError('invalid '+key+' accepted')
`,
  );
  assert.equal(result.status, 0, result.stderr);
});
const docker = `#!/usr/bin/env python3
import json,os,pathlib,sys
a=sys.argv[1:]; root=pathlib.Path(os.environ['FAKE_ROOT'])
with (root/'calls.jsonl').open('a') as file: file.write(json.dumps(a)+'\\n')
if a[:2]==['buildx','build']:
 target=a[a.index('--target')+1] if '--target' in a else ''
 if '--push' in a and os.environ.get('FAIL_PHASE')=='runtime' or target=='dependencies' and os.environ.get('FAIL_PHASE')=='dependencies': sys.exit(42)
 if '--metadata-file' in a:
  p=pathlib.Path(a[a.index('--metadata-file')+1]); p.parent.mkdir(parents=True,exist_ok=True); p.write_text(json.dumps({'containerimage.digest':'sha256:'+'b'*64}))
 print('#1 [dependencies] RUN bash scripts/build_release.sh dependencies\\n#1 CACHED\\n#2 [fresh-builder] RUN bash scripts/build_release.sh\\n#2 DONE 10.0s\\n#3 exporting cache\\n#3 DONE 2.0s')
 if '--output' in a and 'dest=' in a[a.index('--output')+1]:
  dest=pathlib.Path(a[a.index('--output')+1].split('dest=',1)[1]); dest.mkdir(parents=True,exist_ok=True)
  if target=='compiler-cache-export': (dest/'entry').write_text('frozen compiler cache')
  else:
   for prefix in ['','dependency-']:
    (dest/(prefix+'build-metrics.json')).write_text(json.dumps({'cacheEnabled':True,'compileNanoseconds':1000000000}))
    stats={'cache_hits':{'counts':{'Rust':1}},'cache_misses':{'counts':{}},'cache_errors':{'counts':{}},'cache_read_errors':0,'cache_write_errors':0,'cache_timeouts':0,'cache_writes':0}
    if os.environ.get('BAD_COUNTER'): stats.pop('cache_hits')
    (dest/(prefix+'sccache-stats.json')).write_text(json.dumps({'stats':stats}))
   artifacts=[{'reason':'compiler-artifact','package_id':package,'fresh':target=='cargo-freshness-evidence','features':[],'profile':{'opt_level':'3'}} for package in ['registry+fixture','path+backend-school']]
   (dest/'cargo-artifacts.jsonl').write_text(''.join(json.dumps(item)+'\\n' for item in artifacts))
elif a[:3]==['buildx','imagetools','inspect']:
 if '--raw' in a: print(json.dumps({'layers':[{'size':1024}]}))
 else: print('Digest: sha256:'+'d'*64)
elif a[:2]==['image','inspect']: print(json.dumps([{'Id':'sha256:fixture','Config':{'User':'appuser','Cmd':['/app/backend-school'],'Env':[]}}]))
elif a[0]=='info': print(str(root))
elif a[0]=='exec': print('400000 100000' if 'cpu.max' in a[-1] else 'eth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0')
elif a[0]=='run':
 entry=a[a.index('--entrypoint')+1]
 if entry=='id': print('1000')
 elif entry=='sha256sum': print('c'*64+'  /app/backend-school')
 elif a[-1]=='/export_benchmark_openapi.sh': print('{"openapi":"3.1.0","paths":{}}')
 else: print('e'*64+'  /app/migrations/001.sql')
`;
for (const [failure, samples] of [
  ["", 2],
  ["", 3],
  ["dependencies", 2],
  ["runtime", 2],
  ["counter", 2],
])
  test(`complete producer/consumer orchestration propagates failures and cleans owned builders (${failure || "success"}, samples=${samples})`, (t) => {
    const f = fixture(t),
      bin = path.join(f.root, "bin");
    mkdirSync(bin);
    writeFileSync(path.join(bin, "docker"), docker, { mode: 0o755 });
    const result = python(
      f.root,
      `
e.__file__=str(root/'scripts/benchmark_backend_export.py')
e.os.cpu_count=lambda:4
e.resource_checkpoint=lambda *args,**kwargs:None
e.snapshot_resources.resource_checkpoint=e.resource_checkpoint
e.main()
records=json.loads((root/'output/results.json').read_text())
samples=int(os.environ['BENCH_SAMPLES'])
assert len(records)==2+4*samples
assert [row['variant'] for row in records[:2]]==['runtime-max','dependencies-min']
for scenario in ['source','workspace']:
 pairs=[[row for row in records if row['scenario']==scenario and row['sample']==sample] for sample in range(1,samples+1)]
 assert all(len(pair)==2 and {row['variant'] for row in pair}==set(e.VARIANTS) for pair in pairs)
 assert [row['variant'] for row in pairs[0]]==list(reversed([row['variant'] for row in pairs[1]]))
for row in records[2:]:
 assert '@sha256:' in row['import'] and ':buildcache' not in row['import']
 assert row['transitionSeconds']==row['producerSeconds']+row['seconds']
assert json.loads((root/'output/complete.json').read_text())['complete'] is True
`,
      {
        PATH: bin + ":" + process.env.PATH,
        FAKE_ROOT: f.root,
        BENCH_OUTPUT: path.join(f.root, "output"),
        GITHUB_REPOSITORY_OWNER: "fixture",
        GITHUB_SHA: f.sha,
        GITHUB_RUN_ID: "123",
        GITHUB_RUN_ATTEMPT: "1",
        BENCH_BUILDKIT_IMAGE: "fixture@sha256:" + "a".repeat(64),
        BENCH_SAMPLES: String(samples),
        ...(failure === "counter"
          ? { BAD_COUNTER: "1" }
          : { FAIL_PHASE: failure }),
      },
    );
    assert.equal(result.status === 0, !failure, result.stderr);
    const calls = readFileSync(path.join(f.root, "calls.jsonl"), "utf8")
      .trim()
      .split("\n")
      .map(JSON.parse);
    const created = calls
      .filter((call) => call[0] === "buildx" && call[1] === "create")
      .map((call) => call[call.indexOf("--name") + 1]);
    const removed = calls
      .filter((call) => call[0] === "buildx" && call[1] === "rm")
      .map((call) => call.at(-1));
    assert.deepEqual(new Set(removed), new Set(created));
    assert.doesNotMatch(
      JSON.stringify(calls),
      /schoolorbit-backend-school-cache|SCHOOL_CARGO_BUILD_STAGE=snapshot-builder|system.*prune/,
    );
    if (!failure) {
      const records = 2 + 4 * samples;
      assert.equal(created.length, records + 1);
      assert.equal(
        calls.filter(
          (call) =>
            call[0] === "run" && call.at(-1) === "/export_benchmark_openapi.sh",
        ).length,
        records + 1,
      );
      assert.equal(
        calls.filter((call) => call.includes("cargo-freshness-evidence"))
          .length,
        records,
      );
      assert.equal(
        calls.filter(
          (call) => call.includes("--push") && call.includes("runtime"),
        ).length,
        records,
      );
    }
  });
