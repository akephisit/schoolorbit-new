# Operations

This guide describes production-facing procedures and invariants. Development conventions live in [`.rules`](../.rules), and executable verification commands live in [Testing](./TESTING.md).

## Runtime Topology

- `backend-admin` listens on `0.0.0.0:8080`, owns the admin database, school records, tenant database provisioning metadata, and deployment coordination.
- `backend-school` listens on `0.0.0.0:8081`, calls backend-admin over the internal network, resolves the tenant from the request, and connects to that tenant's PostgreSQL database.
- `frontend-admin` is the administrative web application.
- `frontend-school` is built and deployed per tenant/subdomain and calls the school API.

Local topology is defined in [`docker-compose.yml`](../docker-compose.yml). The prebuilt production-oriented Podman topology is in [`podman-compose.yml`](../podman-compose.yml). Containers use service DNS names internally; do not use `localhost` for one container to reach another.

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

Backend deploys wait for `/ready` before declaring success. Tenant frontend builds register route/menu metadata only when both `VITE_DEPLOY_KEY` and `SUBDOMAIN` are present. Missing either value intentionally skips registration; production deploys must provide both.

After deployment, verify readiness first, then run the smoke test and the relevant browser workflow with runtime credentials.

## Reverse Proxy and Realtime

The current school API proxy reference is [`nginx-configs/school-api.schoolorbit.app.conf`](../nginx-configs/school-api.schoolorbit.app.conf); [`backend-admin/nginx.conf.example`](../backend-admin/nginx.conf.example) is the admin reference. Review the active host configuration before applying repository examples.

Preserve:

- WebSocket upgrade headers and long-lived connection timeouts;
- direct, uncached realtime paths;
- the expected tenant/frontend CORS origins and credentials;
- forwarded origin/host information required for tenant resolution;
- access-log redaction so tokens, cookies, raw query strings, and PII are not recorded.

Validate WebSocket heartbeat, reconnect, and authenticated server-owned identity through the same proxy path clients use.

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

Menu records originate from frontend route metadata during production builds. Provide matching `VITE_DEPLOY_KEY`, backend `DEPLOY_KEY`, and `SUBDOMAIN`; verify registration did not silently skip.

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
5. Deploy backend-school only, then wait for `/ready`. Deploy frontend-school after backend readiness.

The backend-school workflow performs exact-name checks before creation through the pinned AWS CLI image, uploads an isolated backend-school Compose definition, and recreates only `schoolorbit-backend-school`. It does not replace the production stack Compose or restart unrelated services.

To diagnose private browser delivery, request a fresh typed grant through the authenticated file-download endpoint and keep `data.url` in memory. Fetch that URL separately with the tenant `Origin`, credentials omitted, and referrer disabled. Confirm the R2 response includes a matching `Access-Control-Allow-Origin`; never print, persist, or paste the grant URL because its query string is a temporary bearer credential.

For the first production rollout, `upgrade_file_platform_env.sh` performs the
public-bucket rename idempotently when the server still has only
`R2_BUCKET_NAME`, then derives a deployment-specific private bucket name from
the R2 account ID. This upgrades the environment file once; backend-school
still requires the new public/private settings and never accepts the legacy
variable as a runtime fallback.

### Durable lifecycle and recovery

Uploads scan and inspect bytes before reserving public delivery. Originals and derivatives use immutable versions. Delete revokes delivery in metadata first, then removes objects. Provider or metadata failures leave durable operations for the background reconciler; retries use leases, bounded exponential backoff, and a terminal attempt limit.

When reconciliation is unhealthy:

1. keep the backend running only if `/ready` is healthy;
2. inspect safe reconciliation counters and error codes, never raw keys or signed URLs;
3. restore scanner or bucket access before retrying terminal work;
4. compare file/version/derivative rows with object counts using file IDs and tenant/purpose aggregates;
5. do not manually delete metadata rows or reuse object keys.

Rollback is additive: keep migration `031`, the private bucket, and scanner volume. Retag the previous backend image and recreate backend-school only. Extra schema/configuration is safe for the previous binary; do not move private objects into the public bucket. Roll frontend-school back separately if its file-ID API contract is not compatible.

## Focused Troubleshooting

- Process unavailable: check container state and `/health`.
- Process healthy but not serving traffic: inspect `/ready` and its dependency, then service-network DNS/URLs.
- Backend-school cannot resolve tenants: check `BACKEND_ADMIN_URL`, internal secret/caller headers, request origin/subdomain consistency, and backend-admin readiness.
- Menu changes missing: confirm production build logs, `VITE_DEPLOY_KEY`, `DEPLOY_KEY`, and `SUBDOMAIN`.
- Permission changes stale: verify migration rows, generated registry versions, cache invalidation, and the `permission_changed` client refresh.
- Migration checksum failure: restore the original migration file; add a new migration for the intended change.
- National IDs unreadable or unsearchable: stop writes and verify key/version configuration. Do not guess keys or overwrite ciphertext/blind indexes.
- File Platform not ready: inspect the safe `filePlatform` readiness field, then verify both bucket `HeadBucket` calls and `clamdcheck.sh` inside `schoolorbit-clamd`.
- Upload failure: verify R2 credentials, distinct bucket names, scanner health, purpose limits, and durable reconciliation state.

Use structured logs with correlation context, but redact secrets, cookies, national IDs, request bodies, and raw realtime query strings.
