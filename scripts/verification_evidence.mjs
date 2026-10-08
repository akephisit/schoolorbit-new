import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { suites } from './lib/pipeline-policy.mjs';

const suite = process.env.VERIFICATION_SUITE;
if (!suites.includes(suite)) throw new Error('Invalid verification evidence suite');
const evidence = {
  schemaVersion: 1, suite, runId: process.env.GITHUB_RUN_ID, attempt: Number(process.env.GITHUB_RUN_ATTEMPT),
  tree: execFileSync('git', ['rev-parse', 'HEAD^{tree}'], { encoding: 'utf8' }).trim(),
  node: process.versions.node,
  runnerImage: process.env.ImageVersion || '',
  runnerOS: process.env.RUNNER_OS,
  profile: { incremental: process.env.CARGO_INCREMENTAL, devDebug: process.env.CARGO_PROFILE_DEV_DEBUG, testDebug: process.env.CARGO_PROFILE_TEST_DEBUG },
  rust: suite.startsWith('backend-') ? execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim() : null,
  docker: execFileSync('docker', ['version', '--format', '{{.Server.Version}}'], { encoding: 'utf8' }).trim()
};
writeFileSync(path.join(process.env.RUNNER_TEMP, `verification-${suite}.json`), JSON.stringify(evidence));
