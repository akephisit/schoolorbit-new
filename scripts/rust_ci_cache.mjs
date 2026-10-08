import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { appendFileSync, existsSync, readFileSync, writeFileSync, statSync, utimesSync, readdirSync } from 'node:fs';
import path from 'node:path';

const component = process.env.RUST_COMPONENT;
if (!['backend-admin', 'backend-school'].includes(component)) throw new Error('Invalid Rust cache owner');
const mode = process.argv[2];
const manifestPath = path.join(component, 'target', '.pipeline-sources.json');
// include_str!/include_bytes! are compiler inputs too, even when another
// component owns their source. Keep this list aligned with School consumers.
const inputPaths = component === 'backend-school'
 ? [component, 'contracts/permissions.lock.json', 'frontend-school/static/fonts']
 : [component];
const files = execFileSync('git', ['ls-files', '-z', '--', ...inputPaths], { encoding: 'utf8' }).split('\0').filter(Boolean);
const sources = Object.fromEntries(files.map(file => [file, createHash('sha256').update(readFileSync(file)).digest('hex')]));
const toolchain = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
const profile = ['CARGO_PROFILE_DEV_DEBUG', 'CARGO_PROFILE_TEST_DEBUG', 'CARGO_INCREMENTAL', 'RUSTFLAGS'].map(key => [key, process.env[key] || '']);
const environment = createHash('sha256').update(JSON.stringify({ version: 3, component, toolchain, profile, os: process.env.RUNNER_OS, arch: process.env.RUNNER_ARCH })).digest('hex').slice(0, 24);
const lock = createHash('sha256').update(readFileSync(path.join(component, 'Cargo.lock'))).digest('hex').slice(0, 24);
const prefix = `rust-ci-v3-${component}-${environment}-${lock}-`;
const recipeFiles = execFileSync('git', ['ls-files', '-z', '--', 'scripts/pipeline.mjs', 'scripts/test_backend_school.sh', 'scripts/test_backend_admin.sh', 'scripts/test_school_database_suite.sh', 'scripts/rust_ci_cache.mjs', '.github/workflows/verify.yml', '.github/actions/setup-contract-rust'], { encoding: 'utf8' }).split('\0').filter(Boolean);
const recipes = Object.fromEntries(recipeFiles.map(file => [file, createHash('sha256').update(readFileSync(file)).digest('hex')]));
const key = prefix + createHash('sha256').update(JSON.stringify({sources,recipes})).digest('hex');
if (mode === 'key') {
 appendFileSync(process.env.GITHUB_OUTPUT, `key=${key}\nprefix=${prefix}\n`);
} else if (mode === 'restore') {
 if (!existsSync(manifestPath)) {
  console.log('Rust source evidence absent; Cargo will rebuild workspace inputs.');
 } else {
  const saved = JSON.parse(readFileSync(manifestPath, 'utf8'));
  if (saved.version !== 3 || saved.environment !== environment || saved.lock !== lock || !saved.sources || !Number.isFinite(saved.savedAt)) throw new Error('Rust cache source evidence has incompatible identity');
  let reused = 0;
  for (const [file, hash] of Object.entries(sources)) {
   if (saved.sources[file] === hash) { utimesSync(file, 1, 1); reused++; }
   else {
    // A source edit must be newer than every compiled output from that cache,
    // even for backdated commits or slight clocks differences between runners.
    const dirty = Math.max(Date.now()/1000, saved.savedAt+2);
    utimesSync(file, dirty, dirty);
   }
  }
  const dirty = Math.max(Date.now()/1000, saved.savedAt+2);
  const signature = (entries, directory) => JSON.stringify(Object.entries(entries)
   .filter(([file]) => file.startsWith(directory + '/')).sort(([a],[b]) => a.localeCompare(b)));
  const restoreDirectory = directory => {
   let managed = true;
   for (const entry of readdirSync(directory, {withFileTypes:true})) {
    const child = path.join(directory, entry.name);
    if (entry.isDirectory()) {
     const hasTrackedInput = files.some(file => file.startsWith(child + '/'));
     managed = restoreDirectory(child) && hasTrackedInput && managed;
    }
    else if (!entry.isFile() || !Object.hasOwn(sources, child)) managed = false;
   }
   // Cargo observes directory mtimes for rerun-if-changed=migrations. Compare
   // membership and content so edits/deletions/backdated files still invalidate;
   // never normalize a tree containing untracked or ignored compiler inputs.
   const unchanged = managed && signature(saved.sources, directory) === signature(sources, directory);
   utimesSync(directory, unchanged ? 1 : dirty, unchanged ? 1 : dirty);
   return managed;
  };
  const migrations = path.join(component, 'migrations');
  if (existsSync(migrations)) restoreDirectory(migrations);
  console.log(`Rust unchanged source timestamps restored=${reused}; changed=${files.length-reused}. Cargo freshness still validates compiler/dependencies.`);
 }
} else if (mode === 'save') {
 // Cache keys are immutable and source-specific; main updates the prefix with a
 // fresh accepted snapshot, while PRs can only restore it.
 const savedAt = Math.max(Date.now()/1000, ...files.map(file => statSync(file).mtimeMs/1000));
 writeFileSync(manifestPath, JSON.stringify({version:3,environment,lock,sources,savedAt}));
} else throw new Error('Expected key, restore or save');
