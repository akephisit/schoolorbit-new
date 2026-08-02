# SchoolOrbit Hybrid VPS Installer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build one resumable `migrate-vps` command that prepares a Debian/Ubuntu replacement VPS, configures GitHub and Cloudflare, deploys all SchoolOrbit services through GitHub Actions, verifies the new origin, and performs a confirmed DNS cutover.

**Architecture:** A Bash 4.4+ workstation CLI owns orchestration and non-secret checkpoints; focused Bash modules own configuration, providers, VPS bootstrap, and verification. GitHub Actions remain the build/deploy engine, `podman-compose.yml` becomes the sole production runtime topology, and Cloudflare Origin CA plus proxied DNS protect both API origins.

**Tech Stack:** Bash 4.4+, Bats, ShellCheck, shfmt, GitHub CLI/API, Cloudflare v4 API, SSH, OpenSSL, jq, Podman Compose, Nginx, GitHub Actions, Cloudflare Wrangler Action.

## Global Constraints

- Run the installer from a trusted Linux or WSL administrator workstation.
- Support the project-supported Ubuntu LTS and Debian stable releases by reading `/etc/os-release` on the target.
- Default `BASE_DOMAIN` to `schoolorbit.app`; accept a validated explicit base domain.
- Implement `migrate-vps` only. `fresh-install` is outside this plan. Do not create Neon projects, databases, R2 buckets, or Cloudflare accounts.
- Preserve the existing GitHub repository, Cloudflare account, Neon databases, R2 buckets, API hostnames, tenant data, and old VPS.
- Deploy backend-admin, backend-school, frontend-admin, and tenant frontends through GitHub Actions.
- Never edit an applied migration or write to `_sqlx_migrations`.
- Never accept secrets as command-line values or print secrets, cookies, database URLs, signed URLs, private keys, national IDs, or raw provider responses.
- Persist only non-secret checkpoints under `~/.local/state/schoolorbit-installer/` with `umask 077`.
- Generate a 5,475-day Cloudflare Origin Certificate for exactly `admin-api.BASE_DOMAIN` and `school-api.BASE_DOMAIN`.
- Keep both API DNS records Cloudflare Proxied and require explicit confirmation before cutover or rollback.
- Do not use `curl --insecure`, disable hostname verification, delete the old VPS, or automatically roll DNS back.
- Keep deployment-on-push gated off when the canonical runtime first lands on `main`; the installer enables it only after successful post-cutover verification. Manual installer dispatches bypass this rollout gate.
- Because no replacement VPS exists yet, local/static/integration-fake checks must be completed now and live migration acceptance must be reported as unrun until an actual target is supplied.

---

## Planned File Structure

### Runtime and deployment ownership

- `podman-compose.yml` — sole production owner of Nginx, backend-admin, backend-school, clamd, explicit networks, and the clamd signature volume.
- `nginx-configs/admin-api.conf.template` — base-domain-aware admin proxy.
- `nginx-configs/school-api.conf.template` — base-domain-aware school proxy including upload, SSE, and WebSocket behavior.
- `nginx-configs/school-api.maintenance.conf.template` — base-domain-aware CORS-safe maintenance proxy.
- `scripts/render_nginx_config.sh` — validate a base domain and render exactly the approved template variables.
- `backend-school/docker-compose.yml` — delete after backend deployment uses the canonical topology.

### Installer

- `scripts/schoolorbit-installer` — executable command parser and process entry point.
- `scripts/lib/schoolorbit-installer/common.sh` — safe output, redaction, retry, command checks, and confirmations.
- `scripts/lib/schoolorbit-installer/config.sh` — non-secret arguments and secret input from environment/stdin/hidden prompts.
- `scripts/lib/schoolorbit-installer/state.sh` — run IDs, JSON checkpoints, fingerprints, phase completion, and drift checks.
- `scripts/lib/schoolorbit-installer/github.sh` — repository settings and uniquely correlated workflow dispatch/wait.
- `scripts/lib/schoolorbit-installer/cloudflare.sh` — zone lookup, Origin CA, DNS snapshot, batch cutover, propagation polling, and rollback.
- `scripts/lib/schoolorbit-installer/vps.sh` — SSH bootstrap, deployment key, runtime environment, and TLS installation.
- `scripts/lib/schoolorbit-installer/verification.sh` — direct-origin, public, Compose, Nginx, and smoke verification.
- `scripts/lib/schoolorbit-installer/phases.sh` — the ordered `migrate-vps`, `resume`, and `rollback-dns` state machine.
- `scripts/lib/schoolorbit-installer/remote/bootstrap.sh` — idempotent privileged Debian/Ubuntu host bootstrap.

### Tests and CI

- `scripts/tests/installer/test_helper.bash` — isolated HOME, fixture, and fake-command helpers.
- `scripts/tests/installer/config_state.bats` — input, redaction, state, resume, and drift tests.
- `scripts/tests/installer/providers.bats` — fake GitHub and Cloudflare API tests.
- `scripts/tests/installer/vps.bats` — SSH/bootstrap/runtime/TLS stream tests.
- `scripts/tests/installer/orchestration.bats` — phase order, dry-run, cutover, failure, resume, and rollback tests.
- `scripts/tests/installer/fixtures/` — non-secret OS, provider-response, runtime-environment, and command fixtures.
- `frontend-school/tests/static/deployment-installer.test.mjs` — durable cross-stack topology/workflow/configuration guard.
- `.github/workflows/installer.yml` — ShellCheck, shfmt, Bats, actionlint, Compose, Nginx rendering, and static tests.

### Workflows and documentation

- `.github/workflows/deploy-backend-admin.yml` — canonical target deploy and direct-origin verification.
- `.github/workflows/deploy-backend-school.yml` — canonical target deploy while preserving the irreversible File Platform cutover rules.
- `.github/workflows/deploy-frontend-admin.yml` — new frontend-admin Worker deployment with a Worker secret binding.
- `.github/workflows/deploy-all-schools.yml` — base-domain variables and correlated all-tenant dispatch.
- `.github/workflows/deploy-school-tenant.yml` — base-domain variables and safe Wrangler JSON generation.
- `frontend-admin/wrangler.json` — environment-neutral Worker build definition with no account ID, production URL, or secret.
- `.env.example` — current canonical public/private R2 and runtime variable names.
- `.rules`, `docs/TESTING.md`, `docs/OPERATIONS.md`, `docs/PODMAN_SETUP.md`, and `frontend-admin/README.md` — durable ownership, testing, installation, cutover, rollback, and Worker configuration.

---

### Task 1: Canonical Runtime Topology and Proxy Templates

**Files:**
- Create: `frontend-school/tests/static/deployment-installer.test.mjs`
- Create: `nginx-configs/admin-api.conf.template`
- Create: `nginx-configs/school-api.conf.template`
- Create: `nginx-configs/school-api.maintenance.conf.template`
- Create: `scripts/render_nginx_config.sh`
- Create: `scripts/tests/installer/fixtures/runtime.env`
- Modify: `podman-compose.yml:1-155`
- Modify: `.env.example:17-94`
- Delete: `backend-school/docker-compose.yml`
- Delete: `nginx-configs/admin-api.schoolorbit.app.conf`
- Delete: `nginx-configs/school-api.schoolorbit.app.conf`
- Delete: `nginx-configs/school-api.schoolorbit.app.maintenance.conf`

**Interfaces:**
- Consumes: existing service environment names and current Nginx upload/SSE/WebSocket behavior.
- Produces: explicit runtime resources `schoolorbit-web`, `schoolorbit-file-platform-internal`, `schoolorbit-clamav-egress`, and `schoolorbit-clamav-signatures`; `render_nginx_config TEMPLATE OUTPUT BASE_DOMAIN`; rendered certificate paths `/etc/nginx/ssl/schoolorbit-origin.pem` and `/etc/nginx/ssl/schoolorbit-origin.key`.

- [ ] **Step 1: Write the failing cross-stack ownership and rendering tests**

Add tests that fail against the current duplicate Compose and hard-coded proxy files:

```js
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(import.meta.dirname, '../../..');
const readRepo = (file) => readFile(path.join(repoRoot, file), 'utf8');

test('production services have one explicit Podman owner', async () => {
  const compose = await readRepo('podman-compose.yml');
  await assert.rejects(access(path.join(repoRoot, 'backend-school/docker-compose.yml')));
  for (const name of [
    'schoolorbit-web',
    'schoolorbit-file-platform-internal',
    'schoolorbit-clamav-egress',
    'schoolorbit-clamav-signatures'
  ]) assert.match(compose, new RegExp(`name: ${name}`));
  assert.match(compose, /^  nginx:$/m);
  assert.match(compose, /127\.0\.0\.1:8080:8080/);
  assert.match(compose, /127\.0\.0\.1:8081:8081/);
});

test('proxy templates render a validated non-default base domain', async (t) => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'schoolorbit-nginx-'));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const output = path.join(temporary, 'school.conf');
  await execFileAsync(
    path.join(repoRoot, 'scripts/render_nginx_config.sh'),
    [path.join(repoRoot, 'nginx-configs/school-api.conf.template'), output, 'example.test']
  );
  const rendered = await readFile(output, 'utf8');
  assert.match(rendered, /server_name school-api\.example\.test;/);
  assert.match(rendered, /example\\\.test/);
  assert.doesNotMatch(rendered, /\$\{BASE_DOMAIN(?:_REGEX)?\}/);
  assert.doesNotMatch(rendered, /schoolorbit\.app/);
});
```

- [ ] **Step 2: Run the focused test and verify the intended failures**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because the standalone Compose exists, explicit resource names and Nginx service are absent, and templates/renderer do not exist.

- [ ] **Step 3: Add explicit runtime ownership to `podman-compose.yml`**

Bind backend ports to loopback, add Nginx, and give every shared resource an explicit name:

```yaml
  nginx:
    image: docker.io/library/nginx:stable-alpine
    container_name: schoolorbit-nginx
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
      - ./nginx/ssl:/etc/nginx/ssl:ro
    networks:
      - schoolorbit-net

volumes:
  clamav_signatures:
    name: schoolorbit-clamav-signatures

networks:
  schoolorbit-net:
    name: schoolorbit-web
    driver: bridge
  file-platform-internal:
    name: schoolorbit-file-platform-internal
    internal: true
  clamav-egress:
    name: schoolorbit-clamav-egress
    driver: bridge
```

Change the backend bindings to `127.0.0.1:8080:8080` and `127.0.0.1:8081:8081`. Delete the second Compose owner only after the static test points exclusively at `podman-compose.yml`.

- [ ] **Step 4: Convert all three proxy definitions into strict templates**

Preserve every existing location block and replace only environment-owned values:

```nginx
"https://admin.${BASE_DOMAIN}" $http_origin;
"https://${BASE_DOMAIN}" $http_origin;
"~^https://([\w-]+\.)?${BASE_DOMAIN_REGEX}(:[0-9]+)?$" $http_origin;
server_name admin-api.${BASE_DOMAIN};
server_name school-api.${BASE_DOMAIN};
ssl_certificate /etc/nginx/ssl/schoolorbit-origin.pem;
ssl_certificate_key /etc/nginx/ssl/schoolorbit-origin.key;
```

Remove ACME challenge locations and all `/etc/letsencrypt` paths from these templates. Keep CORS headers, upload streaming, SSE buffering/timeouts, WebSocket upgrade headers, internal access-log suppression, and maintenance JSON unchanged.

- [ ] **Step 5: Implement the renderer with an allowlisted substitution set**

Create an executable script with this interface and validation:

```bash
#!/usr/bin/env bash
set -euo pipefail

template=${1:?template is required}
output=${2:?output is required}
base_domain=${3:?base domain is required}

if [[ ! $base_domain =~ ^[a-z0-9]{1}([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]{1}([a-z0-9-]*[a-z0-9])?)+$ ]]; then
    printf 'Invalid base domain\n' >&2
    exit 64
fi

BASE_DOMAIN=$base_domain
BASE_DOMAIN_REGEX=${base_domain//./\\.}
export BASE_DOMAIN BASE_DOMAIN_REGEX
temporary=$(mktemp "${output}.XXXXXX")
trap 'rm -f "$temporary"' EXIT
envsubst '${BASE_DOMAIN} ${BASE_DOMAIN_REGEX}' <"$template" >"$temporary"
if grep -Eq '\$\{BASE_DOMAIN(_REGEX)?\}' "$temporary"; then
    printf 'Unresolved proxy template variable\n' >&2
    exit 65
fi
chmod 0644 "$temporary"
mv "$temporary" "$output"
trap - EXIT
```

- [ ] **Step 6: Align the environment template and add a safe Compose fixture**

Replace legacy `R2_BUCKET_NAME` with `R2_PUBLIC_BUCKET_NAME` and `R2_PRIVATE_BUCKET_NAME`, use `schoolorbit-backend-admin`/`schoolorbit-backend-school` internal URLs, add the Origin TLS paths where runtime scripts consume them, and ensure the fixture uses values such as `test-only-not-a-secret` rather than credential-shaped strings.

- [ ] **Step 7: Run topology and rendering verification**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
bash -n scripts/render_nginx_config.sh
shellcheck scripts/render_nginx_config.sh
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml config >/dev/null
```

Expected: all commands PASS; rendered proxies contain the test domain and canonical certificate paths, and Compose resolves without exposing fixture values in output.

- [ ] **Step 8: Commit the canonical runtime foundation**

```bash
git add podman-compose.yml .env.example nginx-configs scripts/render_nginx_config.sh \
  scripts/tests/installer/fixtures/runtime.env \
  frontend-school/tests/static/deployment-installer.test.mjs backend-school/docker-compose.yml
git commit -m "refactor: establish canonical production runtime"
```

---

### Task 2: Canonical Backend Deployment Workflows

**Files:**
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`
- Modify: `.github/workflows/deploy-backend-admin.yml:1-190`
- Modify: `.github/workflows/deploy-backend-school.yml:1-end`

**Interfaces:**
- Consumes: canonical Compose, proxy templates, renderer, `/opt/stack/.env`, Origin CA files, `vars.BASE_DOMAIN`, and repository deployment secrets.
- Produces: workflow run names `Deploy Backend Admin (DEPLOYMENT_ID)` and `Deploy Backend School (DEPLOYMENT_ID)`; optional `workflow_dispatch.inputs.deployment_id`; readiness verified directly on the selected VPS.

- [ ] **Step 1: Extend the static test with workflow safety assertions**

```js
test('backend workflows deploy the canonical target without probing the old public origin', async () => {
  for (const file of [
    '.github/workflows/deploy-backend-admin.yml',
    '.github/workflows/deploy-backend-school.yml'
  ]) {
    const source = await readRepo(file);
    assert.match(source, /podman-compose\.yml/);
    assert.match(source, /scripts\/render_nginx_config\.sh/);
    assert.match(source, /deployment_id/);
    assert.match(source, /RUNTIME_DEPLOY_ENABLED/);
    assert.match(source, /--resolve/);
    assert.match(source, /cloudflare-origin-rsa-root\.pem/);
    assert.doesNotMatch(source, /backend-school\/docker-compose\.yml/);
    assert.doesNotMatch(source, /file-platform-runtime/);
  }
});
```

- [ ] **Step 2: Run the focused test and verify it fails on both legacy workflow paths**

Run:

```bash
node --test --test-name-pattern="backend workflows" \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL on missing canonical uploads, dispatch correlation, rollout gate, and direct-origin trust.

- [ ] **Step 3: Add dispatch correlation and a safe push rollout gate**

Add the same dispatch contract to both workflows:

```yaml
run-name: Deploy Backend Admin (${{ inputs.deployment_id || github.sha }})

on:
  workflow_dispatch:
    inputs:
      deployment_id:
        description: "Installer run identifier"
        required: false
        type: string

jobs:
  deploy:
    if: github.event_name == 'workflow_dispatch' || vars.RUNTIME_DEPLOY_ENABLED == 'true'
```

Use the matching school name in the school workflow. Add canonical Compose, proxy templates, and `scripts/render_nginx_config.sh` to push path filters, but rely on the job gate so merging the foundation cannot deploy it to the old VPS.

- [ ] **Step 4: Stage and atomically install canonical deployment files**

Upload these tracked files to `/opt/stack/deployment`:

```text
podman-compose.yml
scripts/render_nginx_config.sh
nginx-configs/admin-api.conf.template
nginx-configs/school-api.conf.template
nginx-configs/school-api.maintenance.conf.template
```

On the target, validate before replacement:

```bash
set -euo pipefail
cd /opt/stack
test -f .env
install -m 0755 deployment/scripts/render_nginx_config.sh deployment/render-nginx
cp deployment/podman-compose.yml podman-compose.yml.next
podman-compose -f podman-compose.yml.next config >/dev/null
mv podman-compose.yml.next podman-compose.yml
```

- [ ] **Step 5: Render proxies and deploy backend-admin by immutable SHA**

Pull `${IMAGE_NAME}:${{ github.sha }}`, tag that exact local image as `latest`, render the admin template using `vars.BASE_DOMAIN || 'schoolorbit.app'`, start `backend-admin`, then start/reload the Compose-owned Nginx. Verify the target from inside the SSH session:

```bash
admin_host="admin-api.${BASE_DOMAIN}"
curl --fail --silent --show-error \
  --cacert /opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem \
  --resolve "${admin_host}:443:127.0.0.1" \
  "https://${admin_host}/ready" |
  jq -e '.status == "ready"' >/dev/null
```

Remove the current check against the public admin hostname because it still resolves to the old VPS before cutover.

- [ ] **Step 6: Move backend-school to canonical Compose without weakening cutover safety**

Replace only the standalone Compose calls and staged proxy paths. Preserve these existing guarantees verbatim in behavior: private-bucket exact-name checks, CORS verification, pinned AWS CLI and clamd images, CORS-safe maintenance mode, readiness before migration, `0600` migration response, all-active-tenant migration/version verification, fix-forward behavior after migration `032`, normal-proxy restoration, and rollback-tag advancement only after success.

After backend-school starts, verify the direct target:

```bash
school_host="school-api.${BASE_DOMAIN}"
curl --fail --silent --show-error \
  --cacert /opt/stack/nginx/ssl/cloudflare-origin-rsa-root.pem \
  --resolve "${school_host}:443:127.0.0.1" \
  "https://${school_host}/ready" |
  jq -e '.status == "ready" and .filePlatform == "ready"' >/dev/null
```

- [ ] **Step 7: Verify workflow syntax, static ownership, and preserved cutover markers**

Run:

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
rg -n "maintenance|migrate-all|migration-status|rollback" \
  .github/workflows/deploy-backend-school.yml
```

Expected: tests and actionlint PASS; the final search shows all four cutover phases in the school workflow.

- [ ] **Step 8: Commit backend workflow normalization**

```bash
git add .github/workflows/deploy-backend-admin.yml \
  .github/workflows/deploy-backend-school.yml \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "ci: deploy backends from canonical runtime"
```

---

### Task 3: Environment-Neutral Cloudflare Frontend Workflows

**Files:**
- Create: `.github/workflows/deploy-frontend-admin.yml`
- Modify: `.github/workflows/deploy-all-schools.yml:1-165`
- Modify: `.github/workflows/deploy-school-tenant.yml:1-104`
- Modify: `frontend-admin/wrangler.json:1-21`
- Modify: `frontend-school/tests/static/deployment-installer.test.mjs`

**Interfaces:**
- Consumes: `vars.BASE_DOMAIN`, `vars.BACKEND_ADMIN_URL`, `vars.BACKEND_SCHOOL_URL`, `vars.CLOUDFLARE_ACCOUNT_ID`, `vars.VAPID_PUBLIC_KEY`; secrets `CLOUDFLARE_API_TOKEN`, `INTERNAL_API_SECRET`, and `DEPLOY_KEY`.
- Produces: workflows `deploy-frontend-admin.yml` and `deploy-all-schools.yml` with installer-correlated run names; `INTERNAL_API_SECRET` as a Worker secret binding rather than a Wrangler variable.

- [ ] **Step 1: Add failing static checks for committed and generated Wrangler configuration**

```js
test('frontend deployments separate public variables from Worker secrets', async () => {
  const wrangler = JSON.parse(await readRepo('frontend-admin/wrangler.json'));
  assert.equal(wrangler.account_id, undefined);
  assert.equal(wrangler.vars, undefined);

  const admin = await readRepo('.github/workflows/deploy-frontend-admin.yml');
  assert.match(admin, /secrets:\s*\|\s*\n\s*INTERNAL_API_SECRET/);
  assert.match(admin, /vars\.BACKEND_ADMIN_URL/);
  assert.match(admin, /vars\.BASE_DOMAIN/);
  assert.match(admin, /wrangler\.deploy\.json/);
  assert.match(admin, /FRONTEND_DEPLOY_ENABLED/);

  for (const file of [
    '.github/workflows/deploy-all-schools.yml',
    '.github/workflows/deploy-school-tenant.yml'
  ]) {
    const source = await readRepo(file);
    assert.match(source, /vars\.BASE_DOMAIN/);
    assert.match(source, /vars\.BACKEND_SCHOOL_URL/);
    assert.doesNotMatch(source, /\.schoolorbit\.app\/\*/);
  }
});
```

- [ ] **Step 2: Run the focused frontend deployment test and verify it fails**

```bash
node --test --test-name-pattern="frontend deployments" \
  frontend-school/tests/static/deployment-installer.test.mjs
```

Expected: FAIL because the admin workflow is absent, committed admin config contains environment data, and tenant workflows hard-code the domain.

- [ ] **Step 3: Make committed frontend-admin Wrangler configuration environment-neutral**

Keep only build/runtime structure:

```json
{
  "name": "schoolorbit-frontend-admin",
  "main": "build/index.js",
  "build": { "command": "npm run build" },
  "compatibility_date": "2025-09-15",
  "compatibility_flags": ["nodejs_compat"],
  "assets": { "directory": "build/client", "binding": "ASSETS" }
}
```

- [ ] **Step 4: Add the frontend-admin build/deploy workflow**

The workflow installs with `npm ci`, runs lint/check/build, generates a non-secret deployment config containing the admin route and public variables, and installs the server-side secret through the official action:

```yaml
run-name: Deploy Frontend Admin (${{ inputs.deployment_id || github.sha }})

on:
  push:
    branches: [main]
    paths: ["frontend-admin/**", ".github/workflows/deploy-frontend-admin.yml"]
  workflow_dispatch:
    inputs:
      deployment_id:
        required: false
        type: string

jobs:
  deploy:
    if: github.event_name == 'workflow_dispatch' || vars.FRONTEND_DEPLOY_ENABLED == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-node@v6
        with:
          node-version: "22"
          cache: npm
          cache-dependency-path: frontend-admin/package-lock.json
      - run: npm ci
        working-directory: frontend-admin
      - run: npm run lint && npm run check && npm run build
        working-directory: frontend-admin
        env:
          PUBLIC_API_URL: ${{ vars.BACKEND_ADMIN_URL }}
          BACKEND_SCHOOL_URL: ${{ vars.BACKEND_SCHOOL_URL }}
      - name: Create deployment configuration
        working-directory: frontend-admin
        env:
          BASE_DOMAIN: ${{ vars.BASE_DOMAIN }}
          PUBLIC_API_URL: ${{ vars.BACKEND_ADMIN_URL }}
          BACKEND_SCHOOL_URL: ${{ vars.BACKEND_SCHOOL_URL }}
        run: |
          jq \
            --arg route "admin.${BASE_DOMAIN}/*" \
            --arg zone "$BASE_DOMAIN" \
            --arg admin_api "$PUBLIC_API_URL" \
            --arg school_api "$BACKEND_SCHOOL_URL" \
            '. + {routes:[{pattern:$route,zone_name:$zone}],
                  vars:{PUBLIC_API_URL:$admin_api,BACKEND_SCHOOL_URL:$school_api}}' \
            wrangler.json >wrangler.deploy.json
      - uses: cloudflare/wrangler-action@v3.14.1
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ vars.CLOUDFLARE_ACCOUNT_ID }}
          workingDirectory: frontend-admin
          command: deploy --config wrangler.deploy.json
          secrets: |
            INTERNAL_API_SECRET
        env:
          INTERNAL_API_SECRET: ${{ secrets.INTERNAL_API_SECRET }}
```

- [ ] **Step 5: Parameterize both tenant workflows and correlate all-tenant runs**

Add `deployment_id` and a matching `run-name` to `deploy-all-schools.yml`. Gate its `get-schools` job with `github.event_name == 'workflow_dispatch' || vars.FRONTEND_DEPLOY_ENABLED == 'true'`. Replace environment-owned secret references with repository variables. Generate tenant Wrangler JSON with `jq -n --arg` so subdomains and URLs are JSON-escaped:

```bash
jq -n \
  --arg name "schoolorbit-school-${SUBDOMAIN}" \
  --arg route "${SUBDOMAIN}.${BASE_DOMAIN}/*" \
  --arg zone "$BASE_DOMAIN" \
  --arg backend "$BACKEND_SCHOOL_URL" \
  --arg vapid "$VAPID_PUBLIC_KEY" \
  --arg subdomain "$SUBDOMAIN" \
  '{name:$name,main:"build/index.js",compatibility_date:"2025-09-15",
    compatibility_flags:["nodejs_compat"],assets:{directory:"build/client",binding:"ASSETS"},
    routes:[{pattern:$route,zone_name:$zone}],
    vars:{PUBLIC_BACKEND_URL:$backend,PUBLIC_VAPID_KEY:$vapid,SUBDOMAIN:$subdomain}}' \
  >wrangler.json
```

Pass `accountId: ${{ vars.CLOUDFLARE_ACCOUNT_ID }}` to Wrangler Action. Keep `INTERNAL_API_SECRET` and `DEPLOY_KEY` in `secrets.*`; do not move them to Worker variables or build environment.

- [ ] **Step 6: Run frontend workflow and build verification**

```bash
node --test frontend-school/tests/static/deployment-installer.test.mjs
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
cd frontend-admin
npm run lint
PUBLIC_API_URL=https://admin-api.example.test \
BACKEND_SCHOOL_URL=https://school-api.example.test npm run check
PUBLIC_API_URL=https://admin-api.example.test \
BACKEND_SCHOOL_URL=https://school-api.example.test npm run build
```

Expected: every command PASS and the build contains no `INTERNAL_API_SECRET` value.

- [ ] **Step 7: Commit frontend deployment ownership**

```bash
git add .github/workflows/deploy-frontend-admin.yml \
  .github/workflows/deploy-all-schools.yml \
  .github/workflows/deploy-school-tenant.yml \
  frontend-admin/wrangler.json \
  frontend-school/tests/static/deployment-installer.test.mjs
git commit -m "ci: add environment-neutral frontend deployments"
```

---

### Task 4: Installer Core, Configuration, and Checkpoint State

**Files:**
- Create: `scripts/schoolorbit-installer`
- Create: `scripts/lib/schoolorbit-installer/common.sh`
- Create: `scripts/lib/schoolorbit-installer/config.sh`
- Create: `scripts/lib/schoolorbit-installer/state.sh`
- Create: `scripts/tests/installer/test_helper.bash`
- Create: `scripts/tests/installer/config_state.bats`
- Create: `scripts/tests/installer/fixtures/secrets.json`

**Interfaces:**
- Consumes: CLI non-secret flags, standard environment variables, a JSON object on stdin when `--secrets-stdin` is selected, or hidden prompts.
- Produces: associative arrays `SO_CONFIG` and `SO_SECRETS`; `parse_args`, `load_inputs`, `state_init`, `state_load`, `state_mark_phase`, `state_phase_done`, `state_assert_fingerprint`; sanitized `info`, `warn`, `die`, `retry`, and `confirm_exact`.

- [ ] **Step 1: Create isolated Bats helpers and failing config/state tests**

```bash
setup() {
    export TEST_ROOT
    TEST_ROOT=$(mktemp -d)
    export HOME="$TEST_ROOT/home"
    export FAKE_COMMAND_LOG="$TEST_ROOT/commands.log"
    export PHASE_LOG="$TEST_ROOT/phases.log"
    export CAPTURED_REQUEST_BODY="$TEST_ROOT/request.json"
    export FAKE_BIN="$TEST_ROOT/bin"
    mkdir -p "$HOME" "$FAKE_BIN"
    : >"$FAKE_COMMAND_LOG"
    : >"$PHASE_LOG"
    export PATH="$FAKE_BIN:$PATH"
    export SCHOOLORBIT_STATE_HOME="$HOME/.local/state/schoolorbit-installer"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/common.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/config.sh"
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/state.sh"
}

teardown() { rm -rf "$TEST_ROOT"; }

make_fake_command() {
    local name=$1 body=$2
    printf '#!/usr/bin/env bash\n%s\n' "$body" >"$FAKE_BIN/$name"
    chmod 0755 "$FAKE_BIN/$name"
}

seed_checkpoint_with_passed_phase() {
    local phase=$1
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=schoolorbit.app
    SO_CONFIG[ref]=main
    SO_CONFIG[bootstrap_user]=root
    SO_CONFIG[server_user]=schoolorbit
    SO_CONFIG[ssh_port]=22
    state_init run-123
    state_mark_phase "$phase" '{"status":"passed"}'
}
```

```bash
@test "rejects command-line secret flags" {
    run parse_args migrate-vps --target 192.0.2.20 --internal-api-secret exposed
    [ "$status" -eq 64 ]
    [[ "$output" != *exposed* ]]
}

@test "checkpoint contains no supplied secret" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    SO_SECRETS[INTERNAL_API_SECRET]=highly-sensitive-test-value
    state_init run-123
    state_mark_phase preflight '{"status":"passed"}'
    run grep -R 'highly-sensitive-test-value' "$SCHOOLORBIT_STATE_HOME"
    [ "$status" -eq 1 ]
}

@test "resume rejects a changed non-secret fingerprint" {
    SO_CONFIG[repository]=owner/repo
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    state_init run-123
    SO_CONFIG[target]=192.0.2.21
    run state_assert_fingerprint
    [ "$status" -eq 78 ]
}
```

- [ ] **Step 2: Run the Bats file and verify it fails because modules do not exist**

```bash
bats scripts/tests/installer/config_state.bats
```

Expected: FAIL while loading missing module files.

- [ ] **Step 3: Implement common safe-output and retry primitives**

Use return code `64` for invalid input, `69` for unavailable providers, `75` for exhausted transient retries, and `78` for state/configuration drift. Redact every loaded secret before emitting messages:

```bash
redact_text() {
    local value=${1-}
    local secret
    for secret in "${SO_SECRETS[@]-}"; do
        [[ -n $secret ]] && value=${value//"$secret"/'[REDACTED]'}
    done
    printf '%s' "$value"
}

retry() {
    local attempts=$1 delay=$2
    shift 2
    local current=1
    until "$@"; do
        (( current >= attempts )) && return 75
        sleep "$delay"
        delay=$((delay * 2))
        current=$((current + 1))
    done
}

confirm_exact() {
    local expected=$1 prompt=$2 answer
    read -r -p "$prompt" answer
    [[ $answer == "$expected" ]]
}
```

- [ ] **Step 4: Implement strict non-secret argument parsing**

Accept only:

```text
migrate-vps --repository --target --base-domain --ref --bootstrap-user --server-user --ssh-port --dry-run --secrets-stdin
migrate-vps --resume RUN_ID
rollback-dns --run-id RUN_ID
```

Defaults are `base_domain=schoolorbit.app`, `ref=main`, `bootstrap_user=root`, `server_user=schoolorbit`, and `ssh_port=22`. The first release accepts one IPv4 target; reject unknown options, secret-looking option names, invalid IPv4 targets, invalid `OWNER/REPOSITORY`, and domains outside the renderer grammar.

- [ ] **Step 5: Implement secret acquisition without command-line values**

Use these local-only input names:

```bash
SO_REQUIRED_SECRETS=(
    SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN
    SCHOOLORBIT_CLOUDFLARE_DEPLOY_TOKEN
    SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN
    DATABASE_URL JWT_SECRET INTERNAL_API_SECRET ENCRYPTION_KEY BLIND_INDEX_KEY DEPLOY_KEY
    NEON_API_KEY NEON_DB_PASSWORD
    R2_ACCESS_KEY_ID R2_SECRET_ACCESS_KEY
    VAPID_PRIVATE_KEY
    SCHOOLORBIT_RUNTIME_GITHUB_TOKEN
    SMOKE_SUBDOMAIN SMOKE_USERNAME SMOKE_PASSWORD
)

SO_REQUIRED_RUNTIME_VALUES=(
    NEON_PROJECT_ID NEON_HOST
    R2_ACCOUNT_ID R2_PUBLIC_BUCKET_NAME R2_PRIVATE_BUCKET_NAME R2_PUBLIC_URL
    VAPID_PUBLIC_KEY
)
```

Read existing environment values first. With `--secrets-stdin`, read one JSON object from stdin and use `jq -er --arg name "$name" '.[$name] | strings | select(length > 0)'`; otherwise use `read -r -s` for secrets and normal `read -r` for runtime values. Reject newlines, known example markers, shared public/private bucket names, and values shorter than the application-specific minimums. Store secrets only in `SO_SECRETS`; store runtime values under `SO_CONFIG[runtime:NAME]`.

- [ ] **Step 6: Implement atomic non-secret JSON checkpoints**

Build the fingerprint from repository, target, base domain, ref, bootstrap user, server user, SSH port, and the sorted non-secret runtime values. Write through a mode-`0600` temporary file and rename:

```bash
state_mark_phase() {
    local phase=$1 details=$2 temporary
    temporary=$(mktemp "${SO_STATE_FILE}.XXXXXX")
    jq --arg phase "$phase" --argjson details "$details" \
      '.phases[$phase] = $details' "$SO_STATE_FILE" >"$temporary"
    chmod 0600 "$temporary"
    mv "$temporary" "$SO_STATE_FILE"
}

state_phase_done() {
    jq -e --arg phase "$1" '.phases[$phase].status == "passed"' \
      "$SO_STATE_FILE" >/dev/null
}
```

The state allowlist is run ID, timestamps, repository, target, base domain, ref, users, SSH port, configuration fingerprint, phase status, workflow run IDs/URLs, Cloudflare zone/record/certificate IDs, certificate expiry, DNS snapshots, and sanitized verification codes.

- [ ] **Step 7: Add the executable entry point and verify help/error behavior**

The entry point resolves its repository root, sources modules by absolute path, keeps `set -x` disabled, and calls `schoolorbit_main "$@"`. Until orchestration is added, `migrate-vps --dry-run` validates config and prints a sanitized plan; mutation requests exit `69` with `Installer phases are not loaded`.

Run:

```bash
bats scripts/tests/installer/config_state.bats
shellcheck scripts/schoolorbit-installer scripts/lib/schoolorbit-installer/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/lib/schoolorbit-installer/*.sh
```

Expected: PASS.

- [ ] **Step 8: Commit the installer core**

```bash
git add scripts/schoolorbit-installer scripts/lib/schoolorbit-installer \
  scripts/tests/installer
git commit -m "feat: add secure installer core and checkpoints"
```

---

### Task 5: GitHub and Cloudflare Provider Modules

**Files:**
- Create: `scripts/lib/schoolorbit-installer/github.sh`
- Create: `scripts/lib/schoolorbit-installer/cloudflare.sh`
- Create: `scripts/tests/installer/providers.bats`
- Create: `scripts/tests/installer/fixtures/cloudflare-zone.json`
- Create: `scripts/tests/installer/fixtures/cloudflare-dns.json`
- Create: `scripts/tests/installer/fixtures/cloudflare-certificate.json`
- Create: `scripts/tests/installer/fixtures/github-runs.json`

**Interfaces:**
- Consumes: `SO_CONFIG`, `SO_SECRETS`, common retry/redaction, and state functions.
- Produces: `github_set_variable NAME VALUE`, `github_set_secret NAME VALUE`, `github_configure_repository`, `github_dispatch_and_wait WORKFLOW DEPLOYMENT_ID`, `cf_preflight`, `cf_issue_origin_certificate CSR_FILE`, `cf_snapshot_dns`, `cf_assert_no_dns_drift`, `cf_apply_dns_batch MODE`, `cf_wait_for_record_content TARGET_IP`, and `cf_wait_for_proxy_resolution`.

- [ ] **Step 1: Write failing provider tests with fake `gh` and `curl` commands**

```bash
@test "GitHub secrets are delivered through stdin" {
    SO_CONFIG[repository]=owner/repo
    SO_SECRETS[INTERNAL_API_SECRET]=stdin-only-value
    github_set_secret INTERNAL_API_SECRET "${SO_SECRETS[INTERNAL_API_SECRET]}"
    run grep -F -- '--body stdin-only-value' "$FAKE_COMMAND_LOG"
    [ "$status" -eq 1 ]
    grep -F 'secret set INTERNAL_API_SECRET --repo owner/repo' "$FAKE_COMMAND_LOG"
}

@test "Cloudflare cutover sends one two-record batch" {
    cf_apply_dns_batch cutover
    run jq -e '
      .patches | length == 2 and
      all(.[]; .content == "192.0.2.20" and .proxied == true)
    ' "$CAPTURED_REQUEST_BODY"
    [ "$status" -eq 0 ]
}

@test "DNS drift blocks cutover" {
    SO_DNS_SNAPSHOT_ETAG=original
    SO_DNS_CURRENT_ETAG=changed
    run cf_assert_no_dns_drift
    [ "$status" -eq 78 ]
}
```

- [ ] **Step 2: Run provider tests and verify missing-module failures**

```bash
bats scripts/tests/installer/providers.bats
```

Expected: FAIL because `github.sh` and `cloudflare.sh` are absent.

- [ ] **Step 3: Implement GitHub repository configuration through stdin**

Require `gh auth status --hostname github.com` and read/write Actions permission for the selected repository before any mutation. Do not extract or persist the administrator's GitHub token.

Map non-secrets to variables:

```text
BASE_DOMAIN
BACKEND_ADMIN_URL
BACKEND_SCHOOL_URL
CLOUDFLARE_ACCOUNT_ID
VAPID_PUBLIC_KEY
RUNTIME_DEPLOY_ENABLED=false
FRONTEND_DEPLOY_ENABLED=false
```

Map deployment credentials to secrets:

```text
SERVER_IP
SERVER_USER
SSH_PRIVATE_KEY
CLOUDFLARE_API_TOKEN
INTERNAL_API_SECRET
DEPLOY_KEY
SMOKE_USERNAME
SMOKE_PASSWORD
```

Use `gh variable set NAME --body VALUE --repo REPOSITORY` only for non-secrets. Use `printf '%s' "$value" | gh secret set NAME --repo REPOSITORY` for every secret. Never pass a secret to `--body`.

- [ ] **Step 4: Implement uniquely correlated workflow dispatch and wait**

Dispatch with the current ref and `deployment_id`, then poll by exact `displayTitle`:

```bash
gh workflow run "$workflow" --repo "${SO_CONFIG[repository]}" \
  --ref "${SO_CONFIG[ref]}" -f "deployment_id=$deployment_id"

gh run list --repo "${SO_CONFIG[repository]}" --workflow "$workflow" \
  --event workflow_dispatch --json databaseId,displayTitle,status,conclusion,url \
  --jq ".[] | select(.displayTitle == \"$expected_title\")"
```

Require exactly one matching run, store its numeric ID and URL, use `gh run watch ID --exit-status`, and return the failed run URL without copying raw logs into state.

- [ ] **Step 5: Implement the Cloudflare API boundary and permission preflight**

Use `Authorization: Bearer` from `SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN`, capture bodies in private temporary files, require `.success == true`, and emit only error codes/messages after redaction. Discover exactly one zone matching `BASE_DOMAIN`, require the zone SSL mode to already be `strict`, then read exactly one A record for each API hostname. Refuse AAAA, CNAME, and multiple-record ambiguity in the first release.

- [ ] **Step 6: Implement CSR signing through Origin CA**

Post the CSR and exact SANs to `/client/v4/certificates` with an actual jq-generated request body:

```bash
jq -n \
  --arg admin_host "admin-api.${SO_CONFIG[base_domain]}" \
  --arg school_host "school-api.${SO_CONFIG[base_domain]}" \
  --rawfile csr "$csr_file" \
  '{hostnames:[$admin_host,$school_host],request_type:"origin-rsa",
    requested_validity:5475,csr:$csr}' >"$request_file"
```

Return certificate PEM only through `CF_CERTIFICATE`, while storing only certificate ID and expiry in state. Validate that both requested SANs are present before installation.

- [ ] **Step 7: Implement snapshot, drift comparison, batch cutover, and rollback**

Snapshot each record's ID, type, name, content, TTL, proxied flag, and modified timestamp. Immediately before mutation, re-read and compare every field. Use one `POST /zones/ZONE_ID/dns_records/batch` body with two `patches`; cutover sets the target IP and `proxied:true`, rollback restores snapshotted content/TTL/proxy values. Poll the Cloudflare API until both record contents match, then poll public DNS only for successful proxied resolution; proxied public DNS is not expected to reveal the target IP.

- [ ] **Step 8: Run provider, shell, and fixture safety tests**

```bash
bats scripts/tests/installer/providers.bats
shellcheck scripts/lib/schoolorbit-installer/github.sh \
  scripts/lib/schoolorbit-installer/cloudflare.sh
shfmt -d -i 4 -ci scripts/lib/schoolorbit-installer/github.sh \
  scripts/lib/schoolorbit-installer/cloudflare.sh
! rg -n "Bearer |ghp_|BEGIN .*PRIVATE KEY|postgres(?:ql)?://" \
  scripts/tests/installer/fixtures
```

Expected: PASS and the fixture secret scan returns no matches.

- [ ] **Step 9: Commit provider automation**

```bash
git add scripts/lib/schoolorbit-installer/github.sh \
  scripts/lib/schoolorbit-installer/cloudflare.sh \
  scripts/tests/installer/providers.bats \
  scripts/tests/installer/fixtures
git commit -m "feat: automate installer provider operations"
```

---

### Task 6: Idempotent VPS Bootstrap and TLS Installation

**Files:**
- Create: `scripts/lib/schoolorbit-installer/vps.sh`
- Create: `scripts/lib/schoolorbit-installer/remote/bootstrap.sh`
- Create: `scripts/tests/installer/vps.bats`
- Create: `scripts/tests/installer/fixtures/os-release-debian`
- Create: `scripts/tests/installer/fixtures/os-release-ubuntu`
- Create: `scripts/tests/installer/fixtures/os-release-unsupported`

**Interfaces:**
- Consumes: validated target/users/SSH port, runtime values, `cf_issue_origin_certificate`, and the current SSH agent/default identity.
- Produces: `remote_os_supported OS_RELEASE_FILE`, `render_runtime_env`, `vps_preflight`, `vps_bootstrap`, `vps_create_deployment_key`, `vps_install_runtime_env`, `vps_issue_and_install_tls`, and a target ready for GitHub workflow SSH.

- [ ] **Step 1: Write failing VPS tests for OS support, idempotency, and secret streaming**

```bash
@test "supports Debian and Ubuntu and rejects another distribution" {
    run remote_os_supported "$BATS_TEST_DIRNAME/fixtures/os-release-debian"
    [ "$status" -eq 0 ]
    run remote_os_supported "$BATS_TEST_DIRNAME/fixtures/os-release-ubuntu"
    [ "$status" -eq 0 ]
    run remote_os_supported "$BATS_TEST_DIRNAME/fixtures/os-release-unsupported"
    [ "$status" -eq 69 ]
}

@test "runtime environment excludes bootstrap credentials" {
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN]=must-not-reach-vps
    SO_SECRETS[SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN]=runtime-only-value
    render_runtime_env >"$TEST_ROOT/runtime.env"
    ! grep -Fq must-not-reach-vps "$TEST_ROOT/runtime.env"
    grep -Fxq 'CLOUDFLARE_API_TOKEN=runtime-only-value' "$TEST_ROOT/runtime.env"
}

@test "bootstrap can run twice without duplicate users or keys" {
    vps_bootstrap
    vps_bootstrap
    [ "$(grep -c 'useradd schoolorbit' "$FAKE_COMMAND_LOG")" -le 1 ]
}
```

- [ ] **Step 2: Run VPS tests and verify missing-module failures**

```bash
bats scripts/tests/installer/vps.bats
```

Expected: FAIL because the VPS module and remote bootstrap are absent.

- [ ] **Step 3: Implement read-only target preflight**

Connect using the bootstrap account, verify the host key through normal SSH policy, parse `/etc/os-release`, require `ID=debian` or `ID=ubuntu`, confirm `sudo -n true` for non-root bootstrap users, and verify the detected SSH server port remains allowed before firewall changes.

- [ ] **Step 4: Implement an idempotent privileged bootstrap script**

The remote script must:

```bash
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y \
  podman podman-compose uidmap slirp4netns fuse-overlayfs \
  curl jq openssl gettext-base ca-certificates ufw
id schoolorbit >/dev/null 2>&1 || useradd --create-home --shell /bin/bash schoolorbit
loginctl enable-linger schoolorbit
install -d -m 0750 -o schoolorbit -g schoolorbit \
  /opt/stack /opt/stack/nginx/conf.d /opt/stack/nginx/ssl /opt/stack/deployment
printf 'net.ipv4.ip_unprivileged_port_start=80\n' \
  >/etc/sysctl.d/90-schoolorbit-rootless-ports.conf
sysctl --system >/dev/null
ufw allow "${SSH_PORT}/tcp"
ufw allow 80/tcp
ufw allow 443/tcp
ufw deny 8080/tcp
ufw deny 8081/tcp
ufw --force enable
```

Guard package/user/firewall changes so a second run reports verified state rather than duplicating rules. Test a fresh SSH session before closing the bootstrap session.

- [ ] **Step 5: Generate and install a dedicated GitHub Actions SSH key**

Create a private temporary directory under `${XDG_RUNTIME_DIR:-/dev/shm}` when writable, otherwise `mktemp -d` with mode `0700`. Generate an Ed25519 key without a passphrase, append the public key exactly once to the runtime user's mode-`0600` `authorized_keys`, load the private key into `SO_SECRETS[SSH_PRIVATE_KEY]`, and delete transient files through an EXIT trap after GitHub configuration.

- [ ] **Step 6: Render and atomically stream `/opt/stack/.env`**

Allow only the canonical names consumed by `podman-compose.yml`. Map the least-privilege tokens explicitly:

```text
CLOUDFLARE_API_TOKEN <- SCHOOLORBIT_CLOUDFLARE_RUNTIME_TOKEN
GITHUB_TOKEN <- SCHOOLORBIT_RUNTIME_GITHUB_TOKEN
CLOUDFLARE_ACCOUNT_ID <- discovered zone account ID
CLOUDFLARE_ZONE_ID <- discovered zone ID
BASE_DOMAIN <- SO_CONFIG[base_domain]
API_URL <- https://school-api.BASE_DOMAIN
GITHUB_REPO <- SO_CONFIG[repository]
GITHUB_REPOSITORY <- SO_CONFIG[repository]
BACKEND_ADMIN_URL <- http://schoolorbit-backend-admin:8080
BACKEND_SCHOOL_URL <- http://schoolorbit-backend-school:8081
VAPID_SUBJECT <- mailto:admin@BASE_DOMAIN
```

Stream with SSH stdin to a target temporary file, validate required names without printing values, set mode `0600`, then rename to `/opt/stack/.env`. Do not transfer the Cloudflare bootstrap/deploy tokens, administrator GitHub auth token, smoke password, or deployment private key into runtime `.env`.

- [ ] **Step 7: Generate, request, and install Origin CA material**

Generate a local RSA-2048 key and CSR with both API SANs, call `cf_issue_origin_certificate`, and download the public RSA root from:

```text
https://developers.cloudflare.com/ssl/static/origin_ca_rsa_root.pem
```

Require SHA-256:

```text
91a8a5567efa6bf941162aa806b3ba476aaddf7867640e53053b35fb225a5dae
```

Verify the signed certificate against the root, compare certificate and private-key public keys, then stream three files to the VPS as `schoolorbit-origin.pem` (`0644`), `schoolorbit-origin.key` (`0600`), and `cloudflare-origin-rsa-root.pem` (`0644`). Remove local key/CSR/certificate files on every exit path.

- [ ] **Step 8: Run VPS module verification**

```bash
bats scripts/tests/installer/vps.bats
shellcheck scripts/lib/schoolorbit-installer/vps.sh \
  scripts/lib/schoolorbit-installer/remote/bootstrap.sh
shfmt -d -i 4 -ci scripts/lib/schoolorbit-installer/vps.sh \
  scripts/lib/schoolorbit-installer/remote/bootstrap.sh
```

Expected: PASS; captured stdout/state contains none of the fixture secret values.

- [ ] **Step 9: Commit VPS bootstrap and TLS support**

```bash
git add scripts/lib/schoolorbit-installer/vps.sh \
  scripts/lib/schoolorbit-installer/remote/bootstrap.sh \
  scripts/tests/installer/vps.bats \
  scripts/tests/installer/fixtures/os-release-*
git commit -m "feat: bootstrap installer target VPS securely"
```

---

### Task 7: Direct-Origin and Post-Cutover Verification

**Files:**
- Create: `scripts/lib/schoolorbit-installer/verification.sh`
- Modify: `scripts/smoke_test.sh:1-319`
- Create: `scripts/tests/installer/orchestration.bats`

**Interfaces:**
- Consumes: target IP, base domain, Cloudflare Origin root, `SMOKE_SUBDOMAIN`, `SMOKE_USERNAME`, `SMOKE_PASSWORD`, and optional `FILE_SMOKE_PNG`.
- Produces: `verify_direct_origin`, `verify_public_services`, and an extended smoke script supporting `SMOKE_RESOLVE_IP` without exposing signed URLs.

- [ ] **Step 1: Write failing verification tests**

```bash
@test "direct verification pins both API hostnames to the target" {
    SO_CONFIG[target]=192.0.2.20
    SO_CONFIG[base_domain]=example.test
    verify_direct_origin
    grep -F -- '--resolve admin-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    grep -F -- '--resolve school-api.example.test:443:192.0.2.20' "$FAKE_COMMAND_LOG"
    ! grep -Fq -- '--insecure' "$FAKE_COMMAND_LOG"
}

@test "public verification fails when either service identity is wrong" {
    export FAKE_ADMIN_IDENTITY=wrong
    run verify_public_services
    [ "$status" -ne 0 ]
}
```

- [ ] **Step 2: Run the focused tests and verify missing verification functions**

```bash
bats scripts/tests/installer/orchestration.bats --filter 'verification'
```

Expected: FAIL because `verification.sh` does not exist.

- [ ] **Step 3: Implement direct-origin readiness and identity checks**

Use `curl --resolve` and the verified Cloudflare root for each hostname. Require admin `/ready` status and service identity, then require school `/ready` fields `status=ready`, `controlPlane=connected`, and `filePlatform=ready`. Run `podman-compose config`, `podman exec schoolorbit-nginx nginx -t`, and bounded container-health checks over SSH.

- [ ] **Step 4: Add `SMOKE_RESOLVE_IP` to the smoke script**

Build per-host curl option arrays without affecting R2 signed URLs:

```bash
admin_api_host=${SMOKE_ADMIN_API_URL#https://}
admin_api_host=${admin_api_host%%/*}
school_api_host=${SMOKE_API_URL#https://}
school_api_host=${school_api_host%%/*}
declare -a admin_api_curl_options=()
declare -a school_api_curl_options=()
if [[ -n ${SMOKE_RESOLVE_IP:-} ]]; then
    admin_api_curl_options+=(--resolve "${admin_api_host}:443:${SMOKE_RESOLVE_IP}")
    school_api_curl_options+=(--resolve "${school_api_host}:443:${SMOKE_RESOLVE_IP}")
fi
```

Parse URL hostnames before building these arrays so no path reaches `--resolve`. Pass the appropriate array to every API request; leave tenant Worker and external grant requests on normal DNS.

- [ ] **Step 5: Add authenticated SSE/CORS smoke coverage**

After login, open `/api/notifications/stream` with a bounded five-second request, accept curl timeout `28` after headers arrive, and require HTTP `200`, `Content-Type: text/event-stream`, matching `Access-Control-Allow-Origin`, and `Access-Control-Allow-Credentials: true`. Do not print the stream body.

- [ ] **Step 6: Add optional self-cleaning private File Platform smoke coverage**

When `FILE_SMOKE_PNG` is set, upload `profile_image`, parse only `data.id`, request the typed download grant into a mode-`0600` temporary file, parse `data.url` inside a short Python process, fetch it with the tenant Origin and no cookies/referrer, compare downloaded bytes, and delete the file ID. Never echo the grant envelope or URL. Register cleanup so the delete request runs after a later assertion failure.

- [ ] **Step 7: Implement public verification and run the focused suite**

Public verification rechecks both API identities without `--resolve`, runs authenticated `scripts/smoke_test.sh`, verifies frontend-admin and the selected tenant return HTML, and fails if authenticated checks were skipped.

Run:

```bash
bats scripts/tests/installer/orchestration.bats
shellcheck scripts/smoke_test.sh scripts/lib/schoolorbit-installer/verification.sh
shfmt -d -i 4 -ci scripts/smoke_test.sh \
  scripts/lib/schoolorbit-installer/verification.sh
```

Expected: PASS with fake endpoints; no signed URL appears in output or fixture logs.

- [ ] **Step 8: Commit verification coverage**

```bash
git add scripts/smoke_test.sh \
  scripts/lib/schoolorbit-installer/verification.sh \
  scripts/tests/installer/orchestration.bats
git commit -m "test: verify installer origins and cutover"
```

---

### Task 8: Resumable Migration Orchestration and Confirmed Rollback

**Files:**
- Create: `scripts/lib/schoolorbit-installer/phases.sh`
- Modify: `scripts/schoolorbit-installer`
- Modify: `scripts/tests/installer/orchestration.bats`

**Interfaces:**
- Consumes: all module interfaces from Tasks 4-7 and the four deploy workflows.
- Produces: complete commands `migrate-vps`, `migrate-vps --resume RUN_ID`, `migrate-vps --dry-run`, and `rollback-dns --run-id RUN_ID`.

- [ ] **Step 1: Add failing end-to-end fake orchestration tests**

```bash
@test "migration runs verified phases in the approved order" {
    run schoolorbit_main migrate-vps --repository owner/repo \
      --target 192.0.2.20 --base-domain example.test
    [ "$status" -eq 0 ]
    expected='preflight input snapshot bootstrap tls deploy origin-verify cutover-gate dns-cutover public-verify handoff'
    [ "$(tr '\n' ' ' <"$PHASE_LOG" | sed 's/ $//')" = "$expected" ]
}

@test "dry run performs no mutation" {
    run schoolorbit_main migrate-vps --repository owner/repo \
      --target 192.0.2.20 --dry-run
    [ "$status" -eq 0 ]
    ! grep -Eq 'secret set|variable set|dns_records/batch|apt-get|workflow run' "$FAKE_COMMAND_LOG"
}

@test "post-cutover failure offers rollback but does not execute it" {
    export FAKE_PUBLIC_VERIFY_FAILURE=1
    run schoolorbit_main migrate-vps --repository owner/repo --target 192.0.2.20
    [ "$status" -ne 0 ]
    [[ "$output" == *'rollback-dns --run-id'* ]]
    ! grep -Fq 'rollback-batch-applied' "$FAKE_COMMAND_LOG"
}

@test "resume skips only reverified passed phases" {
    seed_checkpoint_with_passed_phase preflight
    run schoolorbit_main migrate-vps --resume run-123
    [ "$status" -eq 0 ]
    [ "$(grep -c '^preflight$' "$PHASE_LOG")" -eq 0 ]
    grep -Fxq snapshot "$PHASE_LOG"
}
```

- [ ] **Step 2: Run orchestration tests and verify missing state-machine failures**

```bash
bats scripts/tests/installer/orchestration.bats
```

Expected: FAIL because the final phase state machine is absent.

- [ ] **Step 3: Implement the ordered phase runner**

Define the exact phase table:

```bash
SO_PHASES=(
    preflight input snapshot bootstrap tls deploy
    origin-verify cutover-gate dns-cutover public-verify handoff
)
```

Each phase function performs `plan → apply → verify`, writes a checkpoint only after verification, and on resume revalidates the external resource before skipping. `--dry-run` executes preflight/input/snapshot in read-only mode and prints sanitized planned mutations without calling provider writes, SSH bootstrap, workflow dispatch, or DNS batch endpoints.

- [ ] **Step 4: Implement deployment phase ordering and rollout gates**

Configure GitHub variables/secrets after VPS bootstrap and deployment-key creation. Dispatch and wait in this exact order:

```text
deploy-backend-admin.yml
deploy-backend-school.yml
deploy-frontend-admin.yml
deploy-all-schools.yml
```

Use `DEPLOYMENT_ID=$RUN_ID` for every workflow. Keep `RUNTIME_DEPLOY_ENABLED=false` and `FRONTEND_DEPLOY_ENABLED=false` during migration; manual dispatches still run. Enable both only in `handoff` after public verification passes.

- [ ] **Step 5: Implement exact cutover and rollback confirmations**

Immediately before cutover, print the two-record diff and require:

```text
CUTOVER 192.0.2.20
```

where the IP must equal `SO_CONFIG[target]`. `rollback-dns --run-id RUN_ID` reloads the snapshot, re-collects the bootstrap Cloudflare token, rechecks current records, displays the reverse diff, and requires:

```text
ROLLBACK ORIGINAL_IP
```

Neither confirmation can be supplied through a command-line flag or environment variable.

- [ ] **Step 6: Implement failure classification and handoff output**

Before cutover, return the failing phase, sanitized reason, and exact resume command. After cutover, return the failed verification codes plus the rollback command without applying it. Handoff prints run ID, checkpoint path, workflow URLs, certificate ID/expiry, old/new origin IPs, enabled deployment gates, and a warning to retain the old VPS. It must not print secrets or raw DNS/API bodies.

- [ ] **Step 7: Run the complete fake orchestration matrix**

```bash
bats scripts/tests/installer/config_state.bats
bats scripts/tests/installer/providers.bats
bats scripts/tests/installer/vps.bats
bats scripts/tests/installer/orchestration.bats
shellcheck scripts/schoolorbit-installer scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer \
  scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
```

Expected: PASS for success, dry-run, retry, auth failure, pre-cutover failure, post-cutover failure, drift, resume, cutover refusal, and confirmed rollback cases.

- [ ] **Step 8: Commit the complete command**

```bash
git add scripts/schoolorbit-installer \
  scripts/lib/schoolorbit-installer/phases.sh \
  scripts/tests/installer/orchestration.bats
git commit -m "feat: orchestrate resumable VPS migration"
```

---

### Task 9: Installer CI, Durable Documentation, and Final Verification

**Files:**
- Create: `.github/workflows/installer.yml`
- Modify: `.rules:1-317`
- Modify: `docs/TESTING.md:1-269`
- Modify: `docs/OPERATIONS.md:5-227`
- Modify: `docs/PODMAN_SETUP.md:1-325`
- Modify: `frontend-admin/README.md`

**Interfaces:**
- Consumes: the finished installer, canonical runtime, workflows, tests, and approved design.
- Produces: CI enforcement and one durable operator procedure; no new backlog entry and no claim that live VPS acceptance passed before a VPS exists.

- [ ] **Step 1: Add the installer CI workflow**

Run on installer/runtime/workflow/doc changes. Use Ubuntu 24.04 and these exact checks:

```yaml
- run: sudo apt-get update && sudo apt-get install -y bats shellcheck shfmt podman-compose
- run: shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
- run: shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
- run: bats scripts/tests/installer
- run: node --test frontend-school/tests/static/deployment-installer.test.mjs
- run: env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) podman-compose -f podman-compose.yml config >/dev/null
- run: docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
```

- [ ] **Step 2: Update the authoritative development and testing rules**

Add to `.rules` that `podman-compose.yml` is the sole production Compose owner, deployment workflows must verify the selected origin rather than a still-public old hostname, and installer changes run ShellCheck, shfmt, Bats, actionlint, Compose validation, proxy rendering, and the deployment static guard. Add the same executable commands to `docs/TESTING.md`.

- [ ] **Step 3: Replace legacy operational topology and TLS procedures**

Update `docs/OPERATIONS.md` to remove the isolated backend-school Compose statement and document canonical service-specific recreation, deployment gates, installer checkpoint/resume, direct-origin verification, confirmed cutover, confirmed rollback, and monitoring of the checkpointed Origin CA expiry because Cloudflare does not send expiry notifications. Update `docs/PODMAN_SETUP.md` to make the installer the recommended replacement-VPS path, describe the manual path separately, use explicit network names, and replace Certbot instructions for installer-managed API origins with Origin CA plus Cloudflare Full (strict).

- [ ] **Step 4: Document frontend-admin deployment inputs**

Update `frontend-admin/README.md` so public API URLs/account ID are repository variables and `INTERNAL_API_SECRET` is a Worker secret binding. State that a committed Wrangler file never owns production credentials or URLs.

- [ ] **Step 5: Run the full local verification matrix**

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
node --test frontend-school/tests/static/deployment-installer.test.mjs
bats scripts/tests/installer
shellcheck scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/smoke_test.sh scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/render_nginx_config.sh \
  scripts/smoke_test.sh scripts/lib/schoolorbit-installer/*.sh \
  scripts/lib/schoolorbit-installer/remote/*.sh
docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:1.7.7
env $(grep -v '^#' scripts/tests/installer/fixtures/runtime.env | xargs) \
  podman-compose -f podman-compose.yml config >/dev/null
cd frontend-admin
npm run lint
PUBLIC_API_URL=https://admin-api.example.test \
BACKEND_SCHOOL_URL=https://school-api.example.test npm run check
PUBLIC_API_URL=https://admin-api.example.test \
BACKEND_SCHOOL_URL=https://school-api.example.test npm run build
cd ../frontend-school
npm run lint
PUBLIC_BACKEND_URL=https://school-api.example.test \
PUBLIC_VAPID_KEY=test-only npm run check
npm run test:static
cd ..
git diff --check
git status --short
```

Expected: all locally runnable checks PASS. Record missing Docker/Podman/Bats dependencies as unrun rather than replacing their checks.

- [ ] **Step 6: Perform a secret and destructive-operation audit**

```bash
! rg -n "ghp_|Bearer [A-Za-z0-9]|BEGIN .*PRIVATE KEY|postgres(?:ql)?://[^.].*@|--insecure|set -x" \
  scripts .github/workflows nginx-configs podman-compose.yml
! rg -n "rm -rf|podman volume rm|drop-bucket|DROP DATABASE|DELETE /certificates" \
  scripts/lib/schoolorbit-installer scripts/schoolorbit-installer
```

Expected: no committed secret-shaped values, TLS bypass, shell tracing, broad deletion, provider deletion, database deletion, volume deletion, or certificate revocation in the installer.

- [ ] **Step 7: Record the live acceptance boundary exactly**

Without a rented VPS, report these as unrun: real Ubuntu/Debian bootstrap, GitHub workflow dispatch to the target, direct Origin CA verification, Cloudflare batch cutover, authenticated file/SSE smoke, and rollback drill. Once a target exists, run:

```bash
./scripts/schoolorbit-installer migrate-vps \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run

./scripts/schoolorbit-installer migrate-vps \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app
```

Supply secrets through the environment, hidden prompts, or `--secrets-stdin`; never add them to these commands.

- [ ] **Step 8: Commit CI and durable documentation**

```bash
git add .github/workflows/installer.yml .rules docs/TESTING.md \
  docs/OPERATIONS.md docs/PODMAN_SETUP.md frontend-admin/README.md
git commit -m "docs: add hybrid VPS installer operations"
```

- [ ] **Step 9: Review the complete branch before execution handoff**

```bash
git log --oneline --decorate origin/main..HEAD
git diff --stat origin/main...HEAD
git diff --check origin/main...HEAD
git status --short --branch
```

Expected: nine coherent task commits plus the approved spec/plan commits, no unrelated files, no unstaged changes, and no claim that live acceptance ran without a VPS.
