import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repo = path.resolve(import.meta.dirname, '../..');
const script = path.join(repo, 'scripts/benchmark_backend_build.sh');
test('documentation comparison is bounded to School and cannot be mixed with historical source patches', () => {
  const workflow = readFileSync(path.join(repo, '.github/workflows/backend-build-benchmark.yml'), 'utf8');
  const body = workflow.match(/name: Validate bounded benchmark selection[\s\S]*?        run: \|\n([\s\S]*?)(?=      -)/)[1].replace(/^          /gm, '');
  for (const [backend, baseline, status] of [['backend-school', '', 0], ['backend-admin', '', 1], ['backend-school', 'a'.repeat(40), 1]]) {
    const result = spawnSync('bash', ['-e', '-c', body], {encoding:'utf8', env:{...process.env, BENCH_BACKEND:backend, BENCH_BASELINE:baseline, BENCH_SAMPLES:'2', BENCH_VARIANTS:'default,api-docs', BENCH_PROFILE_ONLY:'false'}});
    assert.equal(result.status, status, result.stderr);
  }
});
for (const mode of ['documentation', 'historical', 'failure']) test(`benchmark export preserves production targets and a clean runtime environment (${mode})`, t => {
  const f = fixture(t), bin = path.join(f.root, 'bin'), docsTarget = path.join(f.root, 'documentation target');
  mkdirSync(bin);
  mkdirSync(path.join(f.root, 'target/release'), {recursive:true});
  writeFileSync(path.join(f.root, 'Cargo.toml'), mode === 'historical' ? '[package]\n' : '[features]\napi-docs = []\n');
  const executable = `#!/bin/bash\nset -eu\ntest -z "\${RUNTIME_SECRET:-}"\ntest "$1" = export-openapi\nprintf '%s\\n' '${JSON.stringify(api)}'\n`;
  writeFileSync(path.join(f.root, 'target/release/backend-school'), mode === 'historical' ? executable : '#!/bin/bash\nexit 98\n', {mode:0o755});
  mkdirSync(path.join(docsTarget, 'debug'), {recursive:true});
  writeFileSync(path.join(docsTarget, 'debug/backend-school'), executable, {mode:0o755});
  writeFileSync(path.join(bin, 'cargo'), '#!/bin/bash\nset -eu\ntest -z "${RUSTC_WRAPPER:-}"\nprintf "%s\\n" "$@" > "$DOCS_ARGUMENTS"\nexit "${DOCS_FAILURE:-0}"\n', {mode:0o755});
  const result = spawnSync('bash', [path.join(repo, 'scripts/export_benchmark_openapi.sh')], {cwd:f.root, encoding:'utf8', env:{...process.env, PATH:bin+':'+process.env.PATH, BENCH_DOCS_TARGET_DIR:docsTarget, DOCS_ARGUMENTS:path.join(f.root, 'args'), DOCS_FAILURE:mode==='failure'?'37':'0', RUSTC_WRAPPER:'must-be-disabled', RUNTIME_SECRET:'must-be-cleared'}});
  assert.equal(result.status, mode==='failure'?37:0, result.stderr);
  if (mode !== 'failure') assert.deepEqual(JSON.parse(result.stdout), api);
  if (mode !== 'historical') {
    const args = readFileSync(path.join(f.root, 'args'), 'utf8').trim().split('\n');
    assert.equal(args[args.indexOf('--features')+1], 'api-docs');
    assert.equal(args[args.indexOf('--target-dir')+1], docsTarget);
    assert.ok(!args.includes('--release'));
  }
});
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
 *" /export_benchmark_openapi.sh "*) printf '{"openapi":"3.1.0","paths":{"/fixture":{}}}\\n';;
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
  assert.equal(calls.match(/exec schoolorbit-benchmark bash \/export_benchmark_openapi.sh/g).length, 2);
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
  assert.match(readFileSync(path.join(f.root,'arguments'),'utf8'),/-Z\ntime-passes/);
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
test('academic comparison requires the owning HTTP crate to actually recompile', t => {
  const f = fixture(t);
  const run = () => spawnSync('python3', [summary, f.folder, 'backend-school', 'default', 'academic', '2'], {encoding:'utf8'});
  assert.notEqual(run().status, 0);
  writeFileSync(path.join(f.folder, 'source-owner-package.txt'), 'school-academic-core-http\n');
  for (const sample of [1, 2]) writeFileSync(path.join(f.folder, `compile-${sample}.log`), '   Compiling school-academic-core-http v0.1.0\n   Compiling backend-school v0.1.0\n');
  assert.equal(run().status, 0);
  writeFileSync(path.join(f.folder, 'source-owner-package.txt'), 'school-academic-timetable-http\n');
  assert.notEqual(run().status, 0, 'a different HTTP owner cannot satisfy the academic probe');
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
    ...process.env, BENCH_BACKEND: 'backend-school', BENCH_BASELINE: baseline, BENCH_VARIANTS: 'default,api-batches',
    GITHUB_WORKSPACE: f.root, RUNNER_TEMP: path.join(f.root, 'runner'), GITHUB_OUTPUT: path.join(f.root, 'output'),
  } });
  if (unrelated) assert.notEqual(result.status, 0);
  else {
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(path.join(f.root, 'runner/baseline/backend-school/src/api_contract.rs'), 'utf8'), 'baseline registry\n');
    assert.equal(readFileSync(path.join(f.root, 'backend-school/src/api_contract.rs'), 'utf8'), 'candidate registry\n');
  }
});

for (const mode of ['graph', 'profile', 'external', 'unrelated']) test(`HTTP ownership comparison ${mode}`, t => {
  const f = fixture(t);
  const source = readFileSync(path.join(repo, '.github/workflows/backend-build-benchmark.yml'), 'utf8');
  const body = source.match(/      - name: Select exact baseline source[^\n]*\n[\s\S]*?        run: \|\n([\s\S]*?)(?=\n      -)/)[1].replace(/^          /gm, '');
  mkdirSync(path.join(f.root, 'backend-school/src'), { recursive: true });
  writeFileSync(path.join(f.root, 'backend-school/src/app.rs'), 'baseline router\n');
  writeFileSync(path.join(f.root, 'backend-school/Cargo.toml'), '[workspace]\nmembers = []\n');
  writeFileSync(path.join(f.root, 'backend-school/Cargo.lock'), 'version = 4\n');
  mkdirSync(path.join(f.root, 'scripts'), {recursive:true});
  writeFileSync(path.join(f.root, 'scripts/validate_backend_benchmark_source.py'), readFileSync(path.join(repo, 'scripts/validate_backend_benchmark_source.py')));
  const git = (...args) => execFileSync('git', ['-C', f.root, ...args], {encoding:'utf8',stdio:['ignore','pipe','pipe']}).trim();
  git('init','-q'); git('config','user.name','Fixture'); git('config','user.email','fixture@example.invalid');
  git('add','.'); git('commit','-qm','baseline');
  const baseline = git('rev-parse','HEAD');
  git('remote','add','origin',f.root);
  writeFileSync(path.join(f.root, 'backend-school/src/app.rs'), 'candidate router\n');
  if (mode === 'profile') writeFileSync(path.join(f.root, 'backend-school/Cargo.toml'), '[workspace]\nmembers = []\n[profile.release]\nopt-level = 2\n');
  if (mode === 'external') writeFileSync(path.join(f.root, 'backend-school/Cargo.lock'), 'version = 4\n[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "' + 'c'.repeat(64) + '"\n');
  if (mode === 'unrelated') writeFileSync(path.join(f.root, 'backend-school/Dockerfile'), 'changed build flags\n');
  if (mode === 'graph') {
    writeFileSync(path.join(f.root, 'backend-school/Cargo.toml'), '[workspace]\nmembers = ["crates/school-academic-core-http"]\n');
    mkdirSync(path.join(f.root, 'backend-school/crates/school-academic-core-http/src'), {recursive:true});
    writeFileSync(path.join(f.root, 'backend-school/crates/school-academic-core-http/Cargo.toml'), '[package]\nname = "school-academic-core-http"\nversion = "0.1.0"\nedition = "2021"\n');
    writeFileSync(path.join(f.root, 'backend-school/crates/school-academic-core-http/src/lib.rs'), 'pub fn router_owner() {}\n');
  }
  git('add','.'); git('commit','-qm','candidate');
  // The mounted candidate comes from Git, even if the checkout is later modified.
  writeFileSync(path.join(f.root, 'backend-school/src/app.rs'), 'uncommitted router\n');
  const result = spawnSync('bash',['-c',body],{cwd:f.root,encoding:'utf8',env:{...process.env,BENCH_BACKEND:'backend-school',BENCH_BASELINE:baseline,BENCH_VARIANTS:'default,http-routes',GITHUB_WORKSPACE:f.root,RUNNER_TEMP:path.join(f.root,'runner'),GITHUB_OUTPUT:path.join(f.root,'output')}});
  if (mode !== 'graph') assert.equal(result.status,64,result.stderr);
  else {
    assert.equal(result.status,0,result.stderr);
    assert.equal(readFileSync(path.join(f.root,'runner/baseline/backend-school/src/app.rs'),'utf8'),'baseline router\n');
    assert.equal(readFileSync(path.join(f.root,'runner/candidate/backend-school/src/app.rs'),'utf8'),'candidate router\n');
    assert.ok(readFileSync(path.join(f.root, 'runner/candidate/backend-school/Cargo.toml'), 'utf8').includes('school-academic-core-http'));
    assert.ok(readFileSync(path.join(f.root, 'runner/candidate/backend-school/crates/school-academic-core-http/src/lib.rs'), 'utf8').includes('router_owner'));
  }
});


test('academic benchmark source selection supports split, monolithic and historical owners', t => {
  const f = fixture(t);
  const helper = path.join(repo, 'scripts/backend_benchmark_source.sh');
  const sources = ['src/modules/academic/core/handlers.rs', 'crates/school-academic-http/src/core/handlers.rs', 'crates/school-academic-core-http/src/core/handlers.rs'];
  for (const source of sources) {
    mkdirSync(path.dirname(path.join(f.root, source)), { recursive: true });
    writeFileSync(path.join(f.root, source), '// source fixture\n');
    const result = spawnSync('bash', ['-c', '. "$1"; source=$(benchmark_owner_source backend-school academic); printf "%s\n" "$source"; benchmark_owner_package "$source"', 'fixture', helper], { cwd: f.root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const expected = source.startsWith('crates/') ? source.split('/')[1] : 'backend-school';
    assert.deepEqual(result.stdout.trim().split('\n'), [source, expected]);
  }
});
