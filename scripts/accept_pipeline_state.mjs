import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { readAcceptedState, validateState } from './lib/pipeline-state.mjs';

const plan = JSON.parse(process.env.PLAN);
if (plan.sha !== process.env.GITHUB_SHA) throw new Error('Accepted release identity mismatch');
const previous = readAcceptedState();
const accepted = { schemaVersion: 2, sha: plan.sha, runId: process.env.GITHUB_RUN_ID, attempt: Number(process.env.GITHUB_RUN_ATTEMPT), components: { ...previous?.components } };
const temp = process.env.RUNNER_TEMP;
for (const part of plan.deploy) {
  const record = JSON.parse(readFileSync(path.join(temp, 'accepted-inputs', `${part}.json`), 'utf8'));
  if (record.sha !== plan.sha || record.component !== part || record.inputHash !== plan.hashes[part]) throw new Error(`Invalid accepted ${part} artifact`);
  const entry = { sha: record.sha, inputHash: record.inputHash };
  if (part.startsWith('backend-')) entry.digest = record.digest;
  else {
    entry.artifactRunId = Number(process.env.GITHUB_RUN_ID);
    entry.bundleDigest = record.bundleDigest;
    if (part === 'frontend-school') {
      const receipts = readdirSync(path.join(temp, 'accepted-workers')).filter((file) => /^worker-release-[a-z0-9-]+\.json$/.test(file));
      entry.workers = Object.fromEntries(receipts.map((file) => {
        const worker = JSON.parse(readFileSync(path.join(temp, 'accepted-workers', file), 'utf8'));
        if (worker.releaseId !== plan.sha || !/^[a-z0-9][a-z0-9-]*$/.test(worker.subdomain) || !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(worker.candidateVersionId)) throw new Error('Invalid School Worker receipt');
        return [worker.subdomain, worker.candidateVersionId];
      }));
      const targets = JSON.parse(readFileSync(path.join(temp, 'accepted-workers', 'school-targets.json'), 'utf8'));
      const expected = targets.map(target => target.subdomain).sort();
      if (receipts.length === 0 || JSON.stringify(Object.keys(entry.workers).sort()) !== JSON.stringify(expected)) throw new Error('School Worker acceptance coverage differs from discovered tenants');
    } else {
      const worker = JSON.parse(readFileSync(path.join(temp, 'accepted-workers', 'admin-worker.json'), 'utf8'));
      if (worker.sha !== plan.sha || !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(worker.versionId)) throw new Error('Invalid Admin Worker receipt');
      entry.versionId = worker.versionId;
    }
  }
  accepted.components[part] = entry;
}
validateState(accepted, { id: Number(process.env.GITHUB_RUN_ID), head_sha: plan.sha });
writeFileSync(path.join(temp, 'pipeline-state.json'), JSON.stringify(accepted));
