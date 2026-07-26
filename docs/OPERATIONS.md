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

Backend-school uses Cloudflare R2-compatible object storage. Runtime configuration includes:

- `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, and `R2_SECRET_ACCESS_KEY`;
- `R2_BUCKET_NAME`, `R2_REGION`, and `R2_PUBLIC_URL`;
- optional `CDN_URL`;
- upload size/type limits from the `MAX_*` and `ALLOWED_*` variables in Compose.

Keep database metadata and object lifecycle consistent. Validate MIME type and size before upload, authorize reads/deletes, keep private objects out of public exposure, and treat cleanup failures as operationally visible. Back up or retain source objects during migrations until metadata and object counts are reconciled.

## Focused Troubleshooting

- Process unavailable: check container state and `/health`.
- Process healthy but not serving traffic: inspect `/ready` and its dependency, then service-network DNS/URLs.
- Backend-school cannot resolve tenants: check `BACKEND_ADMIN_URL`, internal secret/caller headers, request origin/subdomain consistency, and backend-admin readiness.
- Menu changes missing: confirm production build logs, `VITE_DEPLOY_KEY`, `DEPLOY_KEY`, and `SUBDOMAIN`.
- Permission changes stale: verify migration rows, generated registry versions, cache invalidation, and the `permission_changed` client refresh.
- Migration checksum failure: restore the original migration file; add a new migration for the intended change.
- National IDs unreadable or unsearchable: stop writes and verify key/version configuration. Do not guess keys or overwrite ciphertext/blind indexes.
- Upload failure: verify R2 credentials, bucket/region/public URL, upload limits, and whether DB metadata or objects need reconciliation.

Use structured logs with correlation context, but redact secrets, cookies, national IDs, request bodies, and raw realtime query strings.
