# Operations

This guide describes production-facing procedures and invariants. Development conventions live in [`.rules`](../.rules), and executable verification commands live in [Testing](./TESTING.md).

## Runtime Topology

- `backend-admin` listens on container port `8080`, owns the admin database, school records, tenant database provisioning metadata, and deployment coordination.
- `backend-school` listens on container port `8081`, calls backend-admin over the internal network, resolves the tenant from the request, and connects to that tenant's PostgreSQL database.
- `frontend-admin` is the administrative web application.
- `frontend-school` is built and deployed per tenant/subdomain and calls the school API.

Local topology is defined in [`docker-compose.yml`](../docker-compose.yml). [`podman-compose.yml`](../podman-compose.yml) is the sole production Compose owner for both backends, Nginx, clamd, their explicitly named networks, and the scanner volume. The production host publishes backend ports only on `127.0.0.1`; containers use service DNS names internally and must not use `localhost` to reach another container.

For first-time production server bootstrap, follow [Podman server setup](./PODMAN_SETUP.md).

## Required Environment and Secrets

All secrets and environment-specific URLs come from the runtime environment or deployment secret store. Example files are templates only.

Core backend-school secrets:

- `JWT_SECRET`
- `INTERNAL_API_SECRET`
- `ENCRYPTION_KEY`
- `BLIND_INDEX_KEY`
- `DEPLOY_KEY`

Backend-admin also needs its admin `DATABASE_URL` and the provider credentials required by the operations it performs. Tenant provisioning uses the configured Neon values; deployment/DNS operations use the configured GitHub and Cloudflare values.

Internal service calls identify the caller with `X-Internal-Caller`. A caller-specific `INTERNAL_API_SECRET_<CALLER>` may override the shared `INTERNAL_API_SECRET`; the shared value is the controlled fallback during rotation. Keep both sides synchronized while rotating and remove the old value only after all callers are confirmed.

Never commit, print, or paste production secrets into logs, issues, screenshots, test fixtures, or generated artifacts.

## Health and Readiness

Both backends expose:

- `/health` for process liveness without dependency checks;
- `/ready` for dependency readiness and deployment gating.

Compose healthchecks and backend deployment workflows use `/ready`. Backend-school readiness verifies its backend-admin control-plane connection; it must not wake every tenant database. A healthy process with a failing readiness probe should not receive traffic until the dependency failure is resolved.

## Deployment Workflows

Current workflows:

- [deploy-backend-admin.yml](../.github/workflows/deploy-backend-admin.yml)
- [deploy-backend-school.yml](../.github/workflows/deploy-backend-school.yml)
- [deploy-school-tenant.yml](../.github/workflows/deploy-school-tenant.yml)
- [deploy-all-schools.yml](../.github/workflows/deploy-all-schools.yml)
- [permission-contract.yml](../.github/workflows/permission-contract.yml)
- [api-contract.yml](../.github/workflows/api-contract.yml)
- [smoke-test.yml](../.github/workflows/smoke-test.yml)
- [e2e-sandbox.yml](../.github/workflows/e2e-sandbox.yml)
- [installer.yml](../.github/workflows/installer.yml)

Backend workflows stage the tracked canonical Compose file, validate it, atomically replace
`/opt/stack/podman-compose.yml`, and recreate only the selected service plus its declared
dependency. An admin deployment does not restart backend-school or clamd. A school deployment
starts clamd when required and recreates backend-school without restarting backend-admin.
Both workflows verify the selected target origin with the intended hostname and pinned
Cloudflare Origin CA root; they do not use the still-public hostname as proof of the new origin.

`RUNTIME_DEPLOY_ENABLED` gates push-triggered backend deployments and
`FRONTEND_DEPLOY_ENABLED` gates push-triggered frontend deployments. Manual workflow dispatch
remains available while either gate is `false`. The replacement-VPS installer keeps both gates
disabled during migration and enables them only in the final handoff after public verification.

Backend deploys wait for `/ready` before declaring success. Tenant workflows deploy the
frontend first, then run `npm run sync:menu-routes` as an explicit step with server-only
`DEPLOY_KEY` and `SUBDOMAIN`. Missing configuration, an incomplete scan, or a rejected
request fails the deployment workflow instead of being hidden inside the frontend build.

After deployment, verify readiness first, then run the smoke test and the relevant browser workflow with runtime credentials.

## Reverse Proxy and Realtime

The current proxy sources are
[`nginx-configs/school-api.conf.template`](../nginx-configs/school-api.conf.template)
for the school API and
[`nginx-configs/admin-api.conf.template`](../nginx-configs/admin-api.conf.template)
for the admin API. The backend deployment workflows render them with the validated base domain,
install the result, validate Nginx, and then reload it.

Preserve:

- WebSocket upgrade headers and long-lived connection timeouts;
- direct, uncached realtime paths;
- the expected tenant/frontend CORS origins and credentials;
- forwarded origin/host information required for tenant resolution;
- access-log redaction so tokens, cookies, raw query strings, and PII are not recorded.

Validate WebSocket heartbeat, reconnect, and authenticated server-owned identity through the same proxy path clients use.

## Replacement VPS Migration and DNS Rollback

Use [`scripts/schoolorbit-installer`](../scripts/schoolorbit-installer) from an administrator
machine running Bash 4.4 or newer. The target may be Debian or Ubuntu. Supply credentials only
through environment variables, hidden prompts, or `--secrets-stdin`; the installer never accepts
secret values as command-line arguments. Run the read-only provider and target preflight first:

```bash
./scripts/schoolorbit-installer migrate-vps \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` for the real migration. The installer creates a mode-`0600` checkpoint under
`~/.local/state/schoolorbit-installer/`, prints its run ID, and records only non-secret state.
`SCHOOLORBIT_SERVER_PASSWORD` is also required: use a unique value of at least 16 characters for
the `schoolorbit` Linux/Cockpit account. It is installer input, not an application runtime value,
and must remain in `.env.local`, JSON stdin, a hidden prompt, or the operator's secret manager.
After correcting a failure before or during migration, resume the same run without repeating a
verified phase:

```bash
./scripts/schoolorbit-installer migrate-vps --resume RUN_ID
```

The migration bootstraps the target, installs runtime configuration and Origin CA material,
dispatches the two backend workflows followed by the two frontend workflows, and pins frontend
deployment discovery, readiness, and menu synchronization to the selected origin until DNS is
changed. It verifies both APIs directly with `curl --resolve` and pinned Origin CA trust, then
prints the DNS diff and
requires the exact phrase `CUTOVER <target-ip>` before one two-record Cloudflare batch. Public
verification covers API identity, both frontends, authenticated SSE, and the File Platform. Only
after those checks pass does the migration configure the Cockpit management Tunnel, verify its
connector came from the selected target, publish `server.schoolorbit.app`, and verify the public
Cockpit endpoint before the deployment gates are enabled. A failed post-cutover verification reports recovery commands;
it never performs an automatic rollback.

Rollback restores the complete checkpointed record content, TTL, and proxy state. Confirm that
the current records still represent this run, then execute:

```bash
./scripts/schoolorbit-installer rollback-dns --run-id RUN_ID
```

The command prints the reverse diff and requires the exact phrase `ROLLBACK <original-ip>` before
applying one reverse API-DNS batch. If that migration published management DNS, the same rollback
also restores or removes its management CNAME after revalidating both current states. The
replacement VPS, GitHub configuration, and both Cloudflare Tunnels are retained for diagnosis or a
later retry. Keep the old VPS available until the rollback window has been closed explicitly.

The TLS checkpoint stores the Cloudflare Origin CA certificate ID and `certificate_expiry`, but
never the private key. Monitor that expiry independently and schedule replacement in advance;
Cloudflare does not send Origin CA expiry notifications. Keep Cloudflare SSL/TLS mode at
`Full (strict)` for installer-managed API origins.

## Cockpit Management over Cloudflare Tunnel

The supported management path is:

```text
browser -> Cloudflare edge -> Cloudflare Tunnel -> 127.0.0.1:9090 -> Cockpit
```

Cockpit and cloudflared run as host systemd services; they are not Compose services and do not use
the application Nginx container. The host firewall must not allow inbound `9090/tcp`. Cockpit keeps
`root` in `/etc/cockpit/disallowed-users`; log in at `https://server.schoolorbit.app` as
`schoolorbit` so Cockpit Podman sees the same rootless containers as production. A root Cockpit
session would use a different Podman namespace and is intentionally unsupported.

This deployment intentionally has no Cloudflare Access, OTP, or account-member gate. It is a
public login and the login page is therefore publicly reachable. Use a unique strong password, retain SSH key access for recovery,
monitor authentication activity, and treat Cockpit/cloudflared security updates as production
patches. Cloudflare terminates public TLS; Cockpit accepts unencrypted HTTP only on its loopback
listener. The Tunnel token is stored at `/etc/cloudflared/schoolorbit-cockpit.token` as root mode
`0600` and is consumed with `--token-file`; it must never appear in a command argument or checkpoint.

For an already migrated VPS, first ensure the operator environment contains
`SCHOOLORBIT_CLOUDFLARE_BOOTSTRAP_TOKEN` with the account/zone permissions required to manage
Cloudflare Tunnels and DNS plus `SCHOOLORBIT_SERVER_PASSWORD`. Run a read-only check:

```bash
./scripts/schoolorbit-installer configure-cockpit \
  --repository akephisit/schoolorbit-new \
  --target "$TARGET_IP" \
  --base-domain schoolorbit.app \
  --dry-run
```

Remove `--dry-run` to apply. A distinct Tunnel named from the installer run is created or safely
adopted on resume; the management CNAME is published only after Cockpit, the loopback listener, the
fresh SSH verification, and the connector origin IP pass. Resume the same operation with:

```bash
./scripts/schoolorbit-installer configure-cockpit --resume RUN_ID
```

If management DNS was published but validation failed, use the reported run ID:

```bash
./scripts/schoolorbit-installer rollback-cockpit --run-id RUN_ID
```

The command revalidates drift, prints the reverse management diff, and requires the exact phrase
`ROLLBACK COCKPIT server.schoolorbit.app`. It restores an existing snapshotted CNAME or deletes only
the record created by that run. It never deletes either Tunnel or changes application API DNS.

After setup, verify on the target through a privileged SSH session:

```bash
systemctl is-active cockpit.socket schoolorbit-cloudflared.service
ss -ltnH '( sport = :9090 )'
curl -fsS http://127.0.0.1:9090/ping
stat -c '%a %U:%G' /etc/cloudflared/schoolorbit-cockpit.token
```

The only listener must be `127.0.0.1:9090`, the ping service must be `cockpit`, and the token file
must be `600 root:root`. Confirm the Cloudflare connector origin matches the target IP, the CNAME is
proxied to the checkpointed Tunnel UUID, direct access to `<target-ip>:9090` fails, and the
`schoolorbit` Cockpit Podman page lists `schoolorbit-backend-admin`, `schoolorbit-backend-school`,
`schoolorbit-clamd`, and `schoolorbit-nginx`. Retain the prior Tunnel/VPS through the rollback window.

## Tenant Migration and Cutover

Active tenant migrations begin at `backend-school/migrations/001_baseline.sql`. Do not modify an applied migration or hide a checksum mismatch.

Before a cutover, use [`scripts/check_migration_rebaseline_ready.sh`](../scripts/check_migration_rebaseline_ready.sh) for read-only validation.

For a brand-new clean target, [`scripts/prepare_clean_tenant_db.sh`](../scripts/prepare_clean_tenant_db.sh):

- requires an explicit target URL and confirmation;
- refuses legacy migration history and unsafe non-empty targets;
- applies the clean baseline under guarded conditions.

For an existing-tenant move, [`scripts/cutover_tenant_data.sh`](../scripts/cutover_tenant_data.sh):

- requires separately identified source and clean target;
- keeps the target migration history at the active baseline;
- excludes source `_sqlx_migrations`;
- copies tenant data, then requires permission sync and row-count validation.

Treat these scripts as destructive-capable operational tools. Read their validation and confirmation requirements before execution, back up the source, test on non-production data, and schedule a controlled write freeze/cutover. Never improvise by editing SQLx checksum records.

## Permission and Menu Synchronization

Permission definitions originate in `contracts/permissions.json` and are materialized into generated registries plus tenant DB data. Deploy the contract artifacts and any new sequential permission migration together.

After permission changes:

1. verify the generated permission contract;
2. apply the new tenant migration;
3. verify expected permission/grant rows;
4. invalidate affected permission caches and confirm `permission_changed` refresh behavior;
5. rebuild tenant frontends when route metadata changed.

Frontend route metadata is synchronized explicitly after each successful tenant frontend
deployment. Provide matching backend and workflow `DEPLOY_KEY` values plus `SUBDOMAIN`.
Synchronization is transactional: frontend route identity is marked `managed_by =
'frontend'`, cleanup is limited to stale frontend-owned rows, and school-owned names,
icons, placement, active state, ordering, and custom menu records remain unchanged. Treat a
failed synchronization step as a failed deployment and fix the scan or backend error before
rerunning it.

## Encryption and Key Rotation

National IDs use application-side AES-256-GCM through `backend-school/src/utils/field_encryption.rs`. Search uses keyed HMAC-SHA256 blind indexes in `*_national_id_hash` columns.

`ENCRYPTION_KEY` and `BLIND_INDEX_KEY` must remain stable after data is written. Rotation requires a dedicated, reviewed job that decrypts with the old key, re-encrypts with the new key, rebuilds blind indexes, verifies counts/samples safely, and provides rollback. Do not switch either key independently without migrating existing data.

Do not use legacy PostgreSQL `pgcrypto`, `ALTER ROLE`, or database session settings for application field encryption. Do not log plaintext values or keys during migration.

## File Storage

Backend-school owns a provider-neutral File Platform. Business modules and frontends store a logical file ID; they never store an R2 key, bucket, provider URL, or signed URL. The platform selects storage from the registered purpose:

- `R2_PUBLIC_BUCKET_NAME` contains only public purposes such as school branding. `R2_PUBLIC_URL` is the delivery base for this bucket.
- `R2_PRIVATE_BUCKET_NAME` contains profiles, achievements, admissions, question-bank images, and documents. It must have no public custom domain or `r2.dev` access.
- `R2_PRIVATE_BUCKET_NAME` allows browser delivery only through short-lived signed `GET`/`HEAD` requests from `https://*.schoolorbit.app`. The backend-school deployment applies and verifies this CORS policy without making the bucket public.
- The two bucket names must be present and different. `R2_BUCKET_NAME` and `CDN_URL` are not compatibility fallbacks.

Object keys are immutable and server-generated:

```text
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/original.{ext}
tenants/{tenant_id}/{domain}/{purpose}/{file_id}/v{version}/derivatives/{variant}.{ext}
```

The `domain` and `purpose` segments come only from the purpose registry. This looks like folders in R2, but the database remains the source of metadata and lifecycle state. A future document system should reference file IDs and add its authorization relationship; it must not invent another key layout.

Required runtime configuration:

- R2: `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_REGION`, `R2_PUBLIC_BUCKET_NAME`, `R2_PRIVATE_BUCKET_NAME`, and `R2_PUBLIC_URL`.
- Scanner: `CLAMD_ENDPOINT`, `CLAMD_CONNECT_TIMEOUT_MS`, `CLAMD_WRITE_TIMEOUT_MS`, `CLAMD_READ_TIMEOUT_MS`, `CLAMD_MAX_CHUNK_BYTES`, `CLAMD_MAX_RESPONSE_BYTES`, and `CLAMD_MAX_CONCURRENT_SCANS`.
- Lifecycle: `FILE_PRIVATE_GRANT_TTL_SECONDS`, `FILE_RECONCILE_LEASE_SECONDS`, `FILE_RECONCILE_BATCH_SIZE`, `FILE_RECONCILE_MAX_ATTEMPTS`, `FILE_RECONCILE_RETRY_BASE_SECONDS`, and `FILE_RECONCILE_RETRY_MAX_SECONDS`.

Configuration fails closed at startup for missing, placeholder, shared-bucket, or out-of-range values. `/health` remains process liveness. `/ready` requires backend-admin, both R2 buckets, and a clean clamd probe; deployment must gate traffic on `/ready`.

### Bucket and scanner rollout

1. Preserve the existing public bucket and set it as `R2_PUBLIC_BUCKET_NAME`.
2. Check the configured public and private bucket names directly with `HeadBucket`, without requiring account-wide bucket-list access or printing credentials. Create `R2_PRIVATE_BUCKET_NAME` only when that exact private name is absent.
3. Do not attach a public domain, public bucket policy, or `r2.dev` access to the private bucket. Verify `HeadBucket` succeeds for both buckets. Apply the private-bucket CORS policy for `https://*.schoolorbit.app` with `GET` and `HEAD`, then read the policy back before deployment continues.
4. Start the pinned `docker.io/clamav/clamav-debian` runtime. Persist `/var/lib/clamav`, expose no host port, and wait for its healthcheck before backend-school.
5. For a normal release, deploy backend-school only and wait for `/ready`. For
   the File Platform contract cutover, the backend-school workflow first places
   school-api in maintenance mode, starts the cutover image, waits for
   `/ready`, migrates every active tenant, verifies every tenant reached the
   latest migration, and only then restores the normal proxy.

The backend-school workflow performs exact-name checks before creation through the pinned AWS CLI image, validates and promotes the canonical production Compose definition, starts `schoolorbit-clamd` when required, and recreates `schoolorbit-backend-school`. It does not restart backend-admin or create a second production topology.

To diagnose private browser delivery, request a fresh typed grant through the authenticated file-download endpoint and keep `data.url` in memory. Fetch that URL separately with the tenant `Origin`, credentials omitted, and referrer disabled. Confirm the R2 response includes a matching `Access-Control-Allow-Origin`; never print, persist, or paste the grant URL because its query string is a temporary bearer credential.

### File Platform contract cutover

Migration `032_file_platform_contract_cutover.sql` is the clean boundary from
the path-based compatibility schema to the final provider-neutral schema. Its
transactional preflight refuses to drop legacy columns when a logical file has
no version, a ready file has no matching current version, or a legacy profile
or achievement path lacks its file-ID replacement.

The backend-school deployment workflow performs this cutover while the school
API returns a CORS-safe `503` maintenance response. It starts the new image,
waits for `/ready`, calls the internal all-tenant migration endpoint, and
restores normal traffic only when every active tenant reports the same latest
migration version with no failures. The raw migration response is kept in a
mode-`0600` temporary file and must never be printed because it can contain
tenant-specific failure details.

If readiness, migration, or proxy restoration fails, leave maintenance mode
and the cutover image in place, inspect only safe aggregate/error information,
and fix forward. After any tenant applies migration `032`, never run a
backend-school image older than commit `1bdeb0c5`; those binaries still query
columns that no longer exist.

### Durable lifecycle and recovery

Uploads scan and inspect bytes before reserving public delivery. Originals and derivatives use immutable versions. Delete revokes delivery in metadata first, then removes objects. Provider or metadata failures leave durable operations for the background reconciler; retries use leases, bounded exponential backoff, and a terminal attempt limit.

When reconciliation is unhealthy:

1. keep the backend running only if `/ready` is healthy;
2. inspect safe reconciliation counters and error codes, never raw keys or signed URLs;
3. restore scanner or bucket access before retrying terminal work;
4. compare file/version/derivative rows with object counts using file IDs and tenant/purpose aggregates;
5. do not manually delete metadata rows or reuse object keys.

The `rollback` image tag is advanced only after readiness and every active
tenant migration succeeds. For releases after the contract cutover, it may be
used only when its source commit is `1bdeb0c5` or newer. Keep migration `032`,
the private bucket, and the scanner volume; never restore a pre-cutover image
or move private objects into the public bucket. Roll frontend-school back
separately if its file-ID API contract is not compatible.

## Focused Troubleshooting

- Process unavailable: check container state and `/health`.
- Process healthy but not serving traffic: inspect `/ready` and its dependency, then service-network DNS/URLs.
- Backend-school cannot resolve tenants: check `BACKEND_ADMIN_URL`, internal secret/caller headers, request origin/subdomain consistency, and backend-admin readiness.
- Menu changes missing: confirm the post-deployment `Synchronize menu routes` step,
  `DEPLOY_KEY`, `SUBDOMAIN`, and backend route-registration response.
- Permission changes stale: verify migration rows, generated registry versions, cache invalidation, and the `permission_changed` client refresh.
- Migration checksum failure: restore the original migration file; add a new migration for the intended change.
- National IDs unreadable or unsearchable: stop writes and verify key/version configuration. Do not guess keys or overwrite ciphertext/blind indexes.
- File Platform not ready: inspect the safe `filePlatform` readiness field, then verify both bucket `HeadBucket` calls and `clamdcheck.sh` inside `schoolorbit-clamd`.
- Upload failure: verify R2 credentials, distinct bucket names, scanner health, purpose limits, and durable reconciliation state.

Use structured logs with correlation context, but redact secrets, cookies, national IDs, request bodies, and raw realtime query strings.
