import {appendFileSync, mkdirSync, readFileSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {setTimeout as pause} from 'node:timers/promises';

export const ADMIN_WORKER = 'schoolorbit-frontend-admin';

// Only a recognized missing-service response permits first-time creation.
export async function adminWorkerExists({accountId, token, fetchImpl = fetch}) {
  if (!/^[0-9a-f]{32}$/i.test(accountId || '') || typeof token !== 'string' || !token.trim()) {
    throw new Error('Cloudflare Admin Worker credentials are not configured');
  }
  let response;
  let body;
  try {
    response = await fetchImpl(`https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/services/${ADMIN_WORKER}`, {
      headers: {Authorization: `Bearer ${token}`}, signal: AbortSignal.timeout(15000)
    });
    body = await response.json();
  } catch {
    throw new Error('Cloudflare Admin Worker lookup unavailable');
  }
  if (response.ok && body?.success === true && body.result?.default_environment?.script) return true;
  if (response.status === 404 && body?.success === false && Array.isArray(body.errors) &&
      body.errors.length === 1 && [10007, 10090].includes(body.errors[0].code)) return false;
  throw new Error(`Cloudflare Admin Worker lookup refused (HTTP ${response.status})`);
}

export function writeAdminBootstrap(directory, config) {
  if (config.name !== ADMIN_WORKER || !/^\d{4}-\d{2}-\d{2}$/.test(config.compatibility_date || '')) {
    throw new Error('Invalid Admin Worker bootstrap owner or compatibility date');
  }
  const root = path.join(directory, '.wrangler');
  mkdirSync(root, {recursive: true});
  writeFileSync(path.join(root, 'admin-bootstrap.mjs'), `export default {
  fetch() {
    return Response.json({error: 'maintenance'}, {
      status: 503, headers: {'Cache-Control': 'no-store', 'Retry-After': '60'}
    });
  }
};
`);
  const file = path.join(root, 'admin-bootstrap.json');
  // No application assets, API variables, secrets, routes or public preview hosts.
  writeFileSync(file, JSON.stringify({
    name: ADMIN_WORKER,
    main: 'admin-bootstrap.mjs',
    compatibility_date: config.compatibility_date,
    workers_dev: false,
    preview_urls: false,
    routes: []
  }));
  return file;
}

export async function prepareAdminWorker({directory, config, accountId, token, fetchImpl}) {
  if (config.name !== ADMIN_WORKER) throw new Error('Only the Admin Worker may be bootstrapped');
  const bootstrap = !await adminWorkerExists({accountId, token, fetchImpl});
  if (bootstrap) writeAdminBootstrap(directory, config);
  return bootstrap;
}

export async function removeAdminWorker({accountId, token, fetchImpl = fetch, wait = pause}) {
  if (!await adminWorkerExists({accountId, token, fetchImpl})) return false;
  let response;
  let body;
  try {
    response = await fetchImpl(`https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/scripts/${ADMIN_WORKER}`, {
      method: 'DELETE', headers: {Authorization: `Bearer ${token}`}, signal: AbortSignal.timeout(15000)
    });
    body = await response.json();
  } catch {
    throw new Error('Admin Worker deletion unavailable; maintenance must remain enabled');
  }
  if (!response.ok || body?.success !== true) throw new Error(`Admin Worker deletion refused (HTTP ${response.status})`);
  for (let attempt = 0; attempt < 3; attempt++) {
    if (!await adminWorkerExists({accountId, token, fetchImpl})) return true;
    if (attempt < 2) await wait(500);
  }
  throw new Error('Admin Worker deletion is not yet confirmed; maintenance must remain enabled');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv[2] === 'delete') {
    if (process.env.GITHUB_REF !== 'refs/heads/main' || process.env.GITHUB_EVENT_NAME !== 'workflow_dispatch' || process.env.ADMIN_WORKER_RECREATE !== 'true') {
      throw new Error('Admin Worker deletion requires the explicit main Operations action');
    }
    await removeAdminWorker({accountId: process.env.CLOUDFLARE_ACCOUNT_ID, token: process.env.CLOUDFLARE_API_TOKEN});
    console.log('Only the Admin Worker was removed or already absent; the prepared Pipeline will recreate it.');
  } else {
    if (process.argv[2]) throw new Error('Unknown Admin Worker operation');
    const config = JSON.parse(readFileSync('wrangler.deploy.json', 'utf8'));
    const bootstrap = await prepareAdminWorker({directory: process.cwd(), config,
      accountId: process.env.CLOUDFLARE_ACCOUNT_ID, token: process.env.CLOUDFLARE_API_TOKEN});
    appendFileSync(process.env.GITHUB_OUTPUT, `bootstrap=${bootstrap}\n`);
    console.log(bootstrap ? 'Admin Worker absent; create its isolated maintenance bootstrap before staging.' : 'Existing Admin Worker preserved; stage the release version.');
  }
}
