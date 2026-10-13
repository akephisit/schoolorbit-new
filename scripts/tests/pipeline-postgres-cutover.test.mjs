import assert from 'node:assert/strict';
import test from 'node:test';
import { validateCutoverGate } from '../postgres_cutover_gate.mjs';

const sha = 'a'.repeat(40);
const valid = () => ({ repo: 'owner/repo', sha, ref: 'refs/heads/main', runtimeEnabled: 'false', frontendEnabled: 'false',
  main: { object: { sha } }, runs: { workflow_runs: [{ path: '.github/workflows/pipeline.yml', head_sha: sha,
    head_branch: 'main', head_repository: { full_name: 'owner/repo' }, event: 'push', run_attempt: 2 }] },
  jobs: [{ name: 'Pipeline gate', conclusion: 'success', head_sha: sha, run_attempt: 2 }] });

test('database cutover requires trusted current main, paused releases and the latest-attempt build gate', () => {
  assert.doesNotThrow(() => validateCutoverGate(valid()));
  for (const change of [
    (f) => { f.ref = 'refs/heads/feature'; },
    (f) => { f.main.object.sha = 'b'.repeat(40); },
    (f) => { f.runtimeEnabled = 'true'; },
    (f) => { f.frontendEnabled = 'true'; },
    (f) => { f.jobs[0].run_attempt = 1; },
    (f) => { f.jobs[0].conclusion = 'failure'; },
    (f) => { f.runs.workflow_runs[0].head_repository.full_name = 'fork/repo'; },
    (f) => { f.runs.workflow_runs[0].path = '.github/workflows/operations.yml'; },
  ]) {
    const fixture = valid(); change(fixture);
    assert.throws(() => validateCutoverGate(fixture));
  }
});
