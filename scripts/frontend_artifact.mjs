import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const component = process.env.COMPONENT;
const hash = process.env.INPUT_HASH;
const sha = process.env.ARTIFACT_SHA || process.env.GITHUB_SHA;
if (!['frontend-admin', 'frontend-school'].includes(component) || !/^[0-9a-f]{64}$/.test(hash || '') || !/^[0-9a-f]{40}$/.test(sha || '')) throw new Error('Invalid frontend artifact inputs');
const directory = path.resolve(component);
const archive = path.join(process.env.ARTIFACT_DIR || process.env.RUNNER_TEMP, `${component}.tar`);
const manifest = archive.replace(/\.tar$/, '.json');
const cacheManifest = path.join(directory, '.pipeline-build.json');
const digest = () => createHash('sha256').update(readFileSync(archive)).digest('hex');
if (process.argv[2] === 'pack') {
  execFileSync('tar', ['--sort=name', '--mtime=@0', '--owner=0', '--group=0', '--numeric-owner', '-cf', archive, '-C', directory, 'build']);
  const bundleDigest = digest();
  if (existsSync(cacheManifest)) {
    const cached = JSON.parse(readFileSync(cacheManifest, 'utf8'));
    if (cached.inputHash !== hash || cached.bundleDigest !== bundleDigest) throw new Error('Cached bundle does not match the selected build inputs');
  }
  const record = { schemaVersion: 1, component, sha, inputHash: hash, bundleDigest };
  writeFileSync(cacheManifest, JSON.stringify(record));
  writeFileSync(manifest, JSON.stringify(record));
} else if (process.argv[2] === 'unpack') {
  const record = JSON.parse(readFileSync(manifest, 'utf8'));
  if (record.schemaVersion !== 1 || record.component !== component || record.sha !== sha || record.inputHash !== hash || record.bundleDigest !== digest()) throw new Error('Prepared frontend artifact identity or digest is invalid');
  execFileSync('python3', ['-c', 'import tarfile,sys; t=tarfile.open(sys.argv[1]); assert all((m.name == "build" or m.name.startswith("build/")) and ".." not in m.name.split("/") and (m.isfile() or m.isdir()) for m in t.getmembers()), "Unsafe frontend archive member"', archive]);
  execFileSync('tar', ['-xf', archive, '-C', directory, '--no-same-owner']);
} else throw new Error('Expected pack or unpack');
