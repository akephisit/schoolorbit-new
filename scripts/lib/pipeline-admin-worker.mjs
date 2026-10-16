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

// Wrangler does not detach existing custom domains when the desired list is empty.
// Retire only the exact former Admin alias owned by this production Worker.
export async function retireAdminAlias({accountId, token, baseDomain, checkOnly = false, fetchImpl = fetch, wait = pause}) {
  if (!/^[0-9a-f]{32}$/i.test(accountId || '') || typeof token !== 'string' || !token.trim()) {
    throw new Error('Cloudflare Admin Worker credentials are not configured');
  }
  if (typeof baseDomain !== 'string' ||
      !/^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$/.test(baseDomain) ||
      baseDomain.length > 247) throw new Error('Invalid Admin base domain');
  const hostname = `admin.${baseDomain}`;
  const root = `https://api.cloudflare.com/client/v4/accounts/${accountId}/workers/domains`;
  const headers = {Authorization: `Bearer ${token}`};
  async function request(url, method = 'GET') {
    try {
      const response = await fetchImpl(url, {method, headers, signal: AbortSignal.timeout(15000)});
      if (method === 'DELETE' && response.status === 204) return null;
      const body = await response.json();
      if (!response.ok || body?.success !== true) throw new Error();
      return body;
    } catch {
      throw new Error(`Admin alias ${method} unavailable; maintenance must remain enabled`);
    }
  }
  async function find() {
    const body = await request(`${root}?hostname=${encodeURIComponent(hostname)}`);
    const total = body.result_info?.total_count;
    if (!Array.isArray(body.result) || body.result.length > 1 ||
        (total !== undefined && (!Number.isSafeInteger(total) || total !== body.result.length))) {
      throw new Error('Admin alias lookup returned incomplete or ambiguous ownership');
    }
    if (body.result.length === 0) return undefined;
    const domain = body.result[0];
    if (!domain || typeof domain !== 'object' || domain.hostname !== hostname || domain.service !== ADMIN_WORKER ||
        domain.environment !== 'production') {
      throw new Error('Admin alias owner does not match the production Admin Worker');
    }
    if (typeof domain.id !== 'string' || !/^[0-9a-f]{32,64}$/i.test(domain.id)) {
      throw new Error('Admin alias identifier has an unsupported format');
    }
    return domain;
  }
  const domain = await find();
  if (!domain) return false;
  if (checkOnly) return true;
  await request(`${root}/${domain.id}`, 'DELETE');
  for (let attempt = 0; attempt < 3; attempt++) {
    if (!await find()) return true;
    if (attempt < 2) await wait(500);
  }
  throw new Error('Admin alias removal is not yet confirmed; maintenance must remain enabled');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (['check-alias', 'retire-alias'].includes(process.argv[2])) {
    if (process.env.GITHUB_REF !== 'refs/heads/main' || !['push', 'workflow_dispatch'].includes(process.env.GITHUB_EVENT_NAME)) {
      throw new Error('Admin alias retirement requires the main production Pipeline');
    }
    await retireAdminAlias({accountId: process.env.CLOUDFLARE_ACCOUNT_ID,
      token: process.env.CLOUDFLARE_API_TOKEN, baseDomain: process.env.BASE_DOMAIN,
      checkOnly: process.argv[2] === 'check-alias'});
    console.log(process.argv[2] === 'check-alias' ? 'Former Admin alias ownership is safe for retirement.' :
      'Former Admin alias is detached; apex, tenant DNS and Workers are preserved.');
  } else if (process.argv[2] === 'delete') {
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
