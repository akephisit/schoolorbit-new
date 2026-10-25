import {execFileSync} from 'node:child_process';
import {appendFileSync, readFileSync, lstatSync, readdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {git, makePlan, classify} from './pipeline-policy.mjs';
import {readAcceptedState, trustedRun} from './pipeline-state.mjs';
import {requirePreparedTlsGate as requirePreparedGate} from './pipeline-api-tls.mjs';

export const catalogue = Object.freeze(JSON.parse(readFileSync(new URL('./owned-load-cleanup/catalogue.json', import.meta.url), 'utf8')));

export function verifyInputs(directory, expected = catalogue) {
  const root = lstatSync(directory);
  if (!root.isDirectory() || root.isSymbolicLink() || root.uid !== process.getuid() || (root.mode & 0o777) !== 0o700) throw new Error('Private inert bundle directory is required');
  const actual = readdirSync(directory).sort();
  if (JSON.stringify(actual) !== JSON.stringify(Object.keys(expected.files).sort())) throw new Error('Unexpected inert bundle members');
  let bytes = 0;
  for (const name of actual) {
    if (name.includes('/') || !/^[a-z0-9.-]+$/.test(name)) throw new Error('Invalid inert bundle member');
    const file = path.join(directory, name);
    const info = lstatSync(file);
    bytes += info.size;
    if (!info.isFile() || info.isSymbolicLink() || info.uid !== process.getuid() || (info.mode & 0o777) !== 0o600 || info.size > 2 * 1024 * 1024 || bytes > 8 * 1024 * 1024 || createHash('sha256').update(readFileSync(file)).digest('hex') !== expected.files[name]) throw new Error('Reviewed inert bundle hash or ownership mismatch');
  }
}

export function validateDispatch({request, subdomain, schoolId, apiUrl, options}) {
  if (request !== catalogue.operationId || subdomain !== catalogue.subdomain ||
      !['', catalogue.tenantId].includes(schoolId) || apiUrl !== catalogue.apiOrigin) {
    throw new Error('Cleanup requires the exact fixed operation and tenant confirmations');
  }
  if (typeof options !== 'string' || options.length > 1024) throw new Error('Invalid cleanup options');
  let parsed;
  try { parsed = JSON.parse(options); } catch { throw new Error('Invalid cleanup options'); }
  if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object' ||
      Object.keys(parsed).some(key => key !== 'action') ||
      !['execute', 'reconcile'].includes(parsed.action ?? 'execute')) throw new Error('Invalid cleanup options');
  return {operationId: catalogue.operationId, action: parsed.action ?? 'execute'};
}

export function relatedFiles(files) {
  return files.some(file => /^(?:contracts\/|\.github\/|scripts\/lib\/(?:pipeline|owned-load-cleanup)|podman-compose\.yml$|\.rules$)/.test(file) || classify([file]).deploy.length > 0);
}

export function requireQuiescentBeforeDispatch(pulls, runs, ownRun) {
  if (pulls.some(files => relatedFiles(files)) || runs.some(run => String(run.id) !== String(ownRun) &&
      run.head_branch === 'main' && run.status !== 'completed' &&
      /\/(?:pipeline|maintenance|operations)\.yml(?:@.*)?$/.test(run.path ?? ''))) {
    throw new Error('Related open changes or an active production operation require reconciliation before cleanup');
  }
}

function plan(emit = true) {
  const request = validateDispatch({request: process.env.CLEANUP_REQUEST, subdomain: process.env.CLEANUP_SUBDOMAIN,
    schoolId: process.env.CLEANUP_SCHOOL_ID ?? '', apiUrl: process.env.CLEANUP_API_URL, options: process.env.CLEANUP_OPTIONS});
  const repo = process.env.GITHUB_REPOSITORY;
  const sha = process.env.GITHUB_SHA;
  if (process.env.GITHUB_REF !== 'refs/heads/main' || process.env.GITHUB_EVENT_NAME !== 'workflow_dispatch' ||
      !/^[\w.-]+\/[\w.-]+$/.test(repo ?? '') || sha !== git('.', 'rev-parse', 'HEAD')) throw new Error('Cleanup requires an explicit exact-main operation');
  const api = route => JSON.parse(execFileSync('gh', ['api', route], {encoding: 'utf8', maxBuffer: 16 * 1024 * 1024}));
  if (api(`repos/${repo}/git/ref/heads/main`).object.sha !== sha) throw new Error('A newer main exists');
  const runs = api(`repos/${repo}/actions/workflows/pipeline.yml/runs?head_sha=${sha}&per_page=100`).workflow_runs;
  const run = runs.filter(item => trustedRun(item, repo, 'pipeline.yml', sha, true)).sort((a, b) => b.id - a.id)[0];
  if (!run) throw new Error('Exact-main prepared Pipeline is required');
  const pages = JSON.parse(execFileSync('gh', ['api', '--paginate', '--slurp', `repos/${repo}/actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`], {encoding: 'utf8'}));
  requirePreparedGate(pages.flatMap(page => page.jobs), sha, run.run_attempt);
  const state = readAcceptedState();
  if (!state || git('.', 'merge-base', state.sha, sha) !== state.sha) throw new Error('Trusted accepted runtime must be an ancestor of main');
  if (request.action === 'execute') {
    const publicConfig = {
      'frontend-school': {backend: process.env.PUBLIC_BACKEND_URL || '', vapid: process.env.PUBLIC_VAPID_KEY || ''},
      'frontend-admin': {backend: process.env.PUBLIC_API_URL || '', schoolBackend: process.env.BACKEND_SCHOOL_URL || ''}
    };
    if (makePlan({root: '.', head: sha, base: sha, state, scope: 'auto', publicConfig}).deploy.length) throw new Error('Queued runtime changes require the normal Pipeline first');
    const open = api(`repos/${repo}/pulls?state=open&per_page=100`);
    if (open.length >= 100) throw new Error('Open change inventory is incomplete');
    const files = open.map(pull => {
      const items = api(`repos/${repo}/pulls/${pull.number}/files?per_page=100`);
      if (items.length >= 100) throw new Error('Related change inventory is incomplete');
      return items.map(item => item.filename);
    });
    const active = ['queued', 'in_progress', 'waiting', 'pending', 'requested'].flatMap(status => {
      const items = api(`repos/${repo}/actions/runs?status=${status}&per_page=100`).workflow_runs;
      if (!Array.isArray(items) || items.length >= 100) throw new Error('Active operation inventory is incomplete');
      return items;
    });
    requireQuiescentBeforeDispatch(files, active, process.env.GITHUB_RUN_ID);
  }
  const raw = readFileSync(new URL('./owned-load-cleanup/catalogue.json', import.meta.url));
  const controller = readFileSync(new URL('./pipeline-remote/owned-load-cleanup.py', import.meta.url));
  const proof = readFileSync(new URL('./owned-load-cleanup-proof.py', import.meta.url));
  const hash = value => createHash('sha256').update(value).digest('hex');
  if (emit) appendFileSync(process.env.GITHUB_OUTPUT, `action=${request.action}\noperation_id=${request.operationId}\naccepted_app_sha=${state.sha}\nbackend_input_hash=${state.components['backend-school'].inputHash}\ncatalogue_base64=${raw.toString('base64')}\ncatalogue_sha256=${hash(raw)}\ncontroller_sha256=${hash(controller)}\nproof_sha256=${hash(proof)}\n`);
  console.log('Fixed cleanup input and trusted operation gate verified.');
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv[2] === 'verify-inputs' && process.argv.length === 4) {
    verifyInputs(process.argv[3]);
    console.log('Eight reviewed inert operation inputs verified.');
  } else if (process.argv[2] === 'recheck' && process.argv.length === 3) plan(false);
  else if (process.argv.length === 2) plan();
  else throw new Error('Unknown cleanup tool command');
}
