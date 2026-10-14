import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
const script = path.join(repo, 'scripts/benchmark_backend_build.sh');
test('compiler profiling refuses paired-source or altered-profile selections', () => {
  const workflow = readFileSync(path.join(repo, '.github/workflows/backend-build-benchmark.yml'), 'utf8');
  const body = workflow.match(/name: Validate bounded benchmark selection[\s\S]*?        run: \|\n([\s\S]*?)(?=      -)/)[1].replace(/^          /gm, '');
  for (const [baseline, variants, code] of [['', 'default', 0], ['', 'default,cgu64', 1], ['a'.repeat(40), 'default,api-batches', 1]]) {
    const result = spawnSync('bash', ['-e', '-c', body], {encoding:'utf8',env:{...process.env,BENCH_BACKEND:'backend-school',BENCH_SAMPLES:'2',BENCH_BASELINE:baseline,BENCH_VARIANTS:variants,BENCH_PROFILE_ONLY:'true'}});
    assert.equal(result.status, code, result.stderr);
  }
  assert.match(workflow, /cache-from: \$\{\{ steps\.cache\.outputs\.from \}\}/);
  assert.match(workflow, /run: node scripts\/lib\/pipeline-build-cache\.mjs/);
  assert.doesNotMatch(workflow, /cache-to:|push: true/);
});
test('profile-only mode skips warm samples and compares credential-free exports', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'compiler-profile-'));
  t.after(() => rmSync(root, {recursive:true,force:true}));
  const bin = path.join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(path.join(bin, 'docker'), `#!/bin/bash
printf '%s\\n' "$*" >> "$PROFILE_CALLS"
case " $* " in
 *" /benchmark.sh backend-school "*) exit 99;;
 *" export-openapi "*) printf '{"openapi":"3.1.0","paths":{"/fixture":{}}}\\n';;
esac
`, {mode:0o755});
  const workflow = readFileSync(path.join(repo, '.github/workflows/backend-build-benchmark.yml'), 'utf8');
  const body = workflow.match(/name: Compare on the same runner and persistent Cargo state[\s\S]*?        run: \|\n([\s\S]*?)(?=      - uses: actions\/upload-artifact)/)[1].replace(/^          /gm, '');
  const result = spawnSync('bash', ['-e', '-c', body], {cwd:repo,encoding:'utf8',env:{...process.env,PATH:bin+':'+process.env.PATH,BENCH_BACKEND:'backend-school',BENCH_SAMPLES:'2',BENCH_VARIANTS:'default',BENCH_BASELINE:'',BENCH_PROFILE_ONLY:'true',PROFILE_CALLS:path.join(root,'calls'),RUNNER_TEMP:root,GITHUB_SHA:'a'.repeat(40),GITHUB_STEP_SUMMARY:path.join(root,'summary')}});
  assert.equal(result.status, 0, result.stderr);
  const calls = readFileSync(path.join(root,'calls'),'utf8');
  assert.doesNotMatch(calls, /bash \/benchmark\.sh/);
  assert.match(calls, /exec schoolorbit-benchmark bash \/profile\.sh backend-school/);
  assert.doesNotMatch(calls, /exec -e RUSTC_BOOTSTRAP/);
  assert.equal(calls.match(/env -i .*export-openapi/g).length, 2);
  assert.deepEqual(JSON.parse(readFileSync(path.join(root,'build-benchmark/compiler-profile/openapi.json'),'utf8')),JSON.parse(readFileSync(path.join(root,'build-benchmark/openapi-baseline.json'),'utf8')));
});
const summary = path.join(repo, 'scripts/summarize_backend_benchmark.py');
for (const code of [0, 37]) test(`compiler diagnostics scope bootstrap to the binary and preserve exit ${code}`, t => {
  const f = fixture(t), bin = path.join(f.root,'bin');
  mkdirSync(bin);
  const rustc = path.join(bin,'rustc with spaces');
  writeFileSync(rustc,'#!/bin/bash\nprintf "%s:%s\\n" "$*" "${RUSTC_BOOTSTRAP:-off}" >> "$PROFILE_RUSTC_CALLS"\n',{mode:0o755});
  writeFileSync(path.join(bin,'cargo'),`#!/bin/bash
set -euo pipefail
printf '%s\\n' "$RUSTC_WRAPPER" > "$PROFILE_WRAPPER"
printf '%s\\n' "$@" > "$PROFILE_ARGUMENTS"
"$RUSTC_WRAPPER" "$PROFILE_RUSTC" --crate-name dependency
"$RUSTC_WRAPPER" "$PROFILE_RUSTC" --crate-name backend_school
mkdir -p target/release
exit ${code}
`,{mode:0o755});
  const result=spawnSync('bash',[path.join(repo,'scripts/profile_backend_compiler.sh'),'backend-school',f.folder],{cwd:f.root,encoding:'utf8',env:{...process.env,PATH:bin+':'+process.env.PATH,RUSTC_BOOTSTRAP:'parent-only',PROFILE_RUSTC:rustc,PROFILE_RUSTC_CALLS:path.join(f.root,'calls'),PROFILE_WRAPPER:path.join(f.root,'wrapper'),PROFILE_ARGUMENTS:path.join(f.root,'arguments')}});
  assert.equal(result.status,code,result.stderr);
  assert.equal(readFileSync(path.join(f.root,'calls'),'utf8'),'--crate-name dependency:off\n--crate-name backend_school:1\n');
  assert.match(readFileSync(path.join(f.root,'arguments'),'utf8'),/-Z\nllvm-time-trace/);
  assert.match(readFileSync(path.join(f.root,'arguments'),'utf8'),/-C\nlto=off/);
  assert.throws(()=>readFileSync(readFileSync(path.join(f.root,'wrapper'),'utf8').trim()));
});
const api = { openapi: '3.1.0', paths: { '/fixture': {} } };
function fixture(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'backend-benchmark-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const folder = path.join(root, 'default/application');
  mkdirSync(folder, { recursive: true });
  writeFileSync(path.join(root, 'openapi-baseline.json'), JSON.stringify(api));
  writeFileSync(path.join(folder, 'openapi-baseline.json'), JSON.stringify(api));
  writeFileSync(path.join(folder, 'prime-nanoseconds.txt'), '999000000000');
  writeFileSync(path.join(folder, 'unchanged-nanoseconds.txt'), '1000000');
  for (const sample of [1, 2]) {
    const files = {
      [`cargo-nanoseconds-${sample}.txt`]: `${sample * 1000000000}`,
      [`link-nanoseconds-${sample}.txt`]: '250000000',
      [`link-driver-${sample}.txt`]: '-fuse-ld=lld\n',
      [`binary-bytes-${sample}.txt`]: '100',
      [`compile-${sample}.log`]: '   Compiling backend-school v0.1.0\n',
      [`openapi-${sample}.json`]: JSON.stringify(api),
      [`cargo-timing-${sample}.html`]: 'const UNIT_DATA = ' + JSON.stringify([
        { name: 'backend-school', target: 'build script', start: 0, duration: 0.1 },
        { name: 'backend-school', target: 'bin', start: 0.1, duration: 0.9 },
      ]) + ';',
    };
    for (const [name, value] of Object.entries(files)) writeFileSync(path.join(folder, name), value);
  }
  return { root, folder, run: () => spawnSync('python3', [summary, folder, 'backend-school', 'default', 'application', '2'], { encoding: 'utf8' }) };
}
test('summary excludes flag priming, counts the final link once and ranks binary above build script', t => {
  const f = fixture(t), result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const value = JSON.parse(result.stdout);
  assert.equal(value.mean_seconds, 1.5);
  assert.equal(value.prime_seconds, 999);
  assert.equal(value.samples[0].link_seconds, 0.25);
  assert.equal(value.samples[0].units_by_duration[0].target, 'bin');
});
test('Actions summary stays small even when full evidence contains many long compiler unit names', t => {
  const f = fixture(t), result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const data = JSON.parse(result.stdout);
  for (const sample of data.samples) sample.units_by_duration = Array.from({ length: 5000 }, () => ({ name: 'detailed-generic-symbol'.repeat(100), duration_seconds: 1 }));
  writeFileSync(path.join(f.folder, 'result.json'), JSON.stringify(data));
  const other = path.join(f.root, 'default/academic');
  mkdirSync(other, { recursive: true });
  writeFileSync(path.join(other, 'result.json'), JSON.stringify(data));
  const report = spawnSync('python3', [path.join(repo, 'scripts/render_backend_benchmark_summary.py'), f.root], { encoding: 'utf8' });
  assert.equal(report.status, 0, report.stderr);
  assert.ok(Buffer.byteLength(report.stdout) < 4096);
  assert.match(report.stdout, /default\/application/);
  assert.match(report.stdout, /\| default\/academic [^\n]*\n\| default\/application /);
  assert.doesNotMatch(report.stdout, /detailed-generic-symbol/);
  assert.equal(JSON.parse(readFileSync(path.join(f.folder, 'result.json'))).samples[0].units_by_duration.length, 5000);
});
for (const [description, file, content] of [
  ['duplicate link timings', 'link-nanoseconds-1.txt', '1\n2\n'],
  ['unchanged-source sample', 'compile-1.log', 'Finished release'],
  ['missing unit evidence', 'cargo-timing-1.html', '<html></html>'],
  ['changed API', 'openapi-2.json', '{"openapi":"3.1.0","paths":{"/changed":{}}}'],
  ['changed profile baseline', 'openapi-baseline.json', '{"openapi":"3.1.0","paths":{"/changed":{}}}'],
]) test(`summary refuses ${description}`, t => {
  const f = fixture(t);
  writeFileSync(path.join(f.folder, file), content);
  assert.notEqual(f.run().status, 0);
});
test('sample count requires two to five measurements before touching source or running Cargo', t => {
  const f = fixture(t);
  for (const samples of ['1', '6', '2; touch injected']) {
    const result = spawnSync('bash', [script, 'backend-school', 'default', f.folder], {
      cwd: f.root, env: { ...process.env, BENCH_SAMPLES: samples }, encoding: 'utf8',
    });
    assert.equal(result.status, 64);
  }
});
test('failed source-changing compilation restores source and propagates failure', t => {
  const f = fixture(t);
  mkdirSync(path.join(f.root, 'src'));
  mkdirSync(path.join(f.root, 'bin'));
  mkdirSync(path.join(f.root, 'target/release'), { recursive: true });
  writeFileSync(path.join(f.root, 'src/main.rs'), 'fn main() {}\n');
  writeFileSync(path.join(f.root, 'src/api_contract.rs'), '// fixture registry\n');
  writeFileSync(path.join(f.root, 'target/release/backend-school'), '#!/bin/sh\nprintf \'{"openapi":"3.1.0","paths":{"/fixture":{}}}\'\n', { mode: 0o755 });
  writeFileSync(path.join(f.root, 'bin/cargo'), '#!/bin/sh\nif grep -q "Docker invalidation" src/main.rs; then exit 42; fi\n', { mode: 0o755 });
  const result = spawnSync('bash', [script, 'backend-school', 'default', f.folder], {
    cwd: f.root, env: { ...process.env, PATH: path.join(f.root, 'bin') + ':' + process.env.PATH }, encoding: 'utf8',
  });
  assert.equal(result.status, 42, result.stderr);
  assert.equal(readFileSync(path.join(f.root, 'src/main.rs'), 'utf8'), 'fn main() {}\n');
});

for (const unrelated of [false, true]) test(`paired source comparison ${unrelated ? 'rejects unrelated backend edits' : 'archives the exact baseline API source'}`, t => {
  const f = fixture(t);
  const source = readFileSync(path.join(repo, '.github/workflows/backend-build-benchmark.yml'), 'utf8');
  const body = source.match(/      - name: Select exact baseline source[^\n]*\n[\s\S]*?        run: \|\n([\s\S]*?)(?=\n      -)/)?.[1];
  assert.ok(body, 'source selection step exists');
  const command = body.split('\n').map(line => line.slice(10)).join('\n');
  mkdirSync(path.join(f.root, 'backend-school/src'), { recursive: true });
  writeFileSync(path.join(f.root, 'backend-school/src/api_contract.rs'), 'baseline registry\n');
  writeFileSync(path.join(f.root, 'backend-school/src/main.rs'), 'unchanged application\n');
  const git = (...args) => execFileSync('git', ['-C', f.root, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  git('init', '-q'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.invalid');
  git('add', '.'); git('commit', '-qm', 'baseline');
  const baseline = git('rev-parse', 'HEAD');
  git('remote', 'add', 'origin', f.root);
  writeFileSync(path.join(f.root, 'backend-school/src/api_contract.rs'), 'candidate registry\n');
  if (unrelated) writeFileSync(path.join(f.root, 'backend-school/src/main.rs'), 'unrelated application change\n');
  git('commit', '-qam', 'candidate');
  const result = spawnSync('bash', ['-c', command], { cwd: f.root, encoding: 'utf8', env: {
    ...process.env, BENCH_BACKEND: 'backend-school', BENCH_BASELINE: baseline,
    GITHUB_WORKSPACE: f.root, RUNNER_TEMP: path.join(f.root, 'runner'), GITHUB_OUTPUT: path.join(f.root, 'output'),
  } });
  if (unrelated) assert.notEqual(result.status, 0);
  else {
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(path.join(f.root, 'runner/baseline/backend-school/src/api_contract.rs'), 'utf8'), 'baseline registry\n');
    assert.equal(readFileSync(path.join(f.root, 'backend-school/src/api_contract.rs'), 'utf8'), 'candidate registry\n');
  }
});
