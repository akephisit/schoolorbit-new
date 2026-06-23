# PostgreSQL Self-Host Migration Design

Date: 2026-06-23
Status: Draft for review

## Goal

Move SchoolOrbit production database hosting away from Neon because Neon cost is now too high. The target platform is PostgreSQL 18 self-hosted, with Databasus managing scheduled backups, restore visibility, and point-in-time recovery where configured.

The migration strategy is clean migration, not near-zero-downtime replication. The current production data set is small or non-critical enough that the first production path can prioritize simplicity and rollback clarity over live replication.

## Current State

SchoolOrbit has two database roles in the architecture:

- `backend-admin` owns the platform/admin database. It stores schools, deployment metadata, and each tenant database connection string.
- `backend-school` resolves tenant database URLs through `backend-admin`, then opens per-tenant `PgPool`s and runs tenant migrations lazily.

The tenant side is already well positioned for migration:

- Active tenant migrations use a clean baseline under `backend-school/migrations/001_baseline.sql`.
- Legacy migrations are archived under `backend-school/migrations_legacy/`.
- Existing cutover scripts prepare clean tenant databases, copy tenant data without `_sqlx_migrations`, sync permissions, and compare row counts.

The main Neon coupling is in `backend-admin`:

- tenant creation calls Neon API directly;
- tenant deletion calls Neon API directly;
- connection strings are built as Neon URLs;
- production compose requires `NEON_*` environment variables.

## Decisions

1. Use PostgreSQL 18 self-hosted for the platform/admin DB and tenant DBs.
2. Use Databasus for backup orchestration and restore verification, not as a database host or HA system.
3. Keep one database per school tenant. Do not collapse tenants into shared schemas.
4. Do not do logical replication for the first migration.
5. Keep Neon support temporarily behind a provider interface so rollback and staged migration remain possible.
6. New tenant provisioning should default to self-hosted PostgreSQL after the provider switch.
7. Existing Neon tenant databases can be copied only when they are worth keeping.

## Architecture

Add a provider boundary inside `backend-admin`:

```text
SchoolService
  -> DatabaseProvider
       -> NeonDatabaseProvider
       -> SelfHostedPostgresProvider
```

`SchoolService` keeps the current high-level flow:

1. validate school input;
2. ask the provider to create a tenant database and return metadata plus connection string;
3. insert the `schools` row with `status = 'provisioning'`;
4. call `backend-school /internal/provision`;
5. deploy/finalize the tenant frontend;
6. mark the school active.

Provider outputs should use a small typed result:

```text
ProvisionedDatabase {
  provider: "neon" | "self_hosted_postgres",
  database_name: String,
  connection_string: String,
  external_id: Option<String>,
}
```

Deletion uses the same provider boundary. For self-hosted PostgreSQL, deletion should either:

- terminate active sessions and `DROP DATABASE ... WITH (FORCE)` when supported, or
- mark cleanup pending and let an operational cleanup command retry safely.

## Self-Hosted PostgreSQL Layout

The self-hosted cluster should contain:

- one platform/admin database, e.g. `schoolorbit_admin`;
- one tenant database per school, e.g. `schoolorbit_sandbox`;
- a provisioning/admin role with permission to create databases and roles;
- an app role for `backend-admin`;
- tenant owner/app credentials used in each tenant connection string.

Required tenant migration capabilities:

- create `uuid-ossp`;
- create `pg_trgm`;
- create tables, indexes, enums, PL/pgSQL functions, triggers, and comments.

The first implementation will use one tenant owner role for all tenant databases. Per-tenant database roles and independent tenant password rotation are out of scope for this migration and can be added after the self-hosted cutover is stable.

## Environment

Add these provider-focused environment variables:

```text
DATABASE_PROVIDER=self_hosted_postgres
DATABASE_URL=postgresql://.../schoolorbit_admin
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://provisioner:...@postgres:5432/postgres
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=...
SELF_HOSTED_POSTGRES_SSLMODE=disable|require
```

Production should not keep `NEON_*` variables as required after the switch. They can remain optional while `DATABASE_PROVIDER=neon` is still supported.

## Infrastructure

Production stack should add:

- PostgreSQL 18 container or VM-managed service with persistent volume;
- Databasus container/service;
- backup storage target, preferably Cloudflare R2 or S3-compatible storage;
- health checks for Postgres and Databasus;
- disk usage monitoring and alerting;
- clear restore runbook.

The first self-host deployment should run side by side with Neon. Neon remains a read-only fallback source until the migration is verified and a retention window has passed.

## Migration Flow

### Phase 1: Stand Up Self-Hosted Database

1. Deploy PostgreSQL 18.
2. Create platform/admin database.
3. Restore or migrate `backend-admin` data from Neon.
4. Run `backend-admin` migrations.
5. Configure Databasus backups for the admin database and future tenant databases.
6. Perform a test restore into a disposable database.

### Phase 2: Implement Provider Boundary

1. Move Neon-specific code behind `NeonDatabaseProvider`.
2. Add `SelfHostedPostgresProvider`.
3. Add config parsing for `DATABASE_PROVIDER`.
4. Update provisioning progress messages so they no longer hardcode "Neon" when self-hosted mode is active.
5. Update deletion and rollback cleanup through the provider.
6. Update compose/env examples.

### Phase 3: Sandbox Tenant

1. Provision a new sandbox tenant through the self-host provider.
2. Verify `backend-school /internal/provision` applies clean baseline migrations.
3. Verify login, `/api/auth/me`, tenant resolution, CORS, and smoke tests.
4. Add Databasus backup job for the sandbox tenant.
5. Restore sandbox backup into a disposable database and validate a simple login/query path.

### Phase 4: Optional Data Copy for Important Tenants

For each tenant worth preserving:

1. Create a clean target tenant database on self-hosted PostgreSQL.
2. Run the existing clean baseline preparation script.
3. Run tenant data cutover in dry-run mode.
4. Run tenant data cutover in apply mode.
5. Update `schools.db_connection_string` to the self-hosted target connection string.
6. Hit the tenant through the real backend and verify smoke checks.

### Phase 5: Cut Over Defaults

1. Set production `DATABASE_PROVIDER=self_hosted_postgres`.
2. Remove required `NEON_*` env vars from production deployment.
3. Keep Neon credentials available only for rollback until the retention window ends.
4. Keep Neon databases read-only or untouched for 7-14 days.
5. After validation, delete Neon tenant databases and close paid Neon resources.

## Error Handling and Rollback

Provisioning rollback must clean up in this order:

1. mark school row as failed when a row exists;
2. drop the newly created tenant DB when it is safe;
3. delete the school row only if the provider cleanup succeeded and no tenant data was committed;
4. include cleanup errors in the returned provisioning error.

If tenant deletion cannot drop the database because connections are active, the app should report cleanup pending instead of pretending deletion fully succeeded.

Rollback from self-host migration is simple while Neon is retained:

- restore the old `schools.db_connection_string`;
- set `DATABASE_PROVIDER=neon` if new provisioning must temporarily return to Neon;
- redeploy services with the previous env;
- verify tenant login and smoke checks.

## Security and Privacy

The migration must preserve the current app-side PII standard:

- `national_id` remains AES-GCM encrypted by the backend using `ENCRYPTION_KEY`;
- `*_national_id_hash` remains HMAC blind-indexed using `BLIND_INDEX_KEY`;
- plaintext PII must not be logged during provisioning, cutover, dump, restore, or backup verification.

Connection strings are secrets. They must not be logged. Admin/provisioner credentials should not be shared with runtime tenant connections if per-tenant roles are implemented.

## Verification

Minimum verification before production cutover:

- `backend-admin` starts and runs migrations against self-hosted PostgreSQL.
- `backend-school` provisions a self-hosted sandbox tenant.
- tenant baseline applies with exactly one successful SQLx migration row at version `1`.
- `scripts/check_migration_rebaseline_ready.sh` passes for the self-hosted tenant.
- smoke test passes for sandbox.
- Databasus creates a backup and a restore is tested into a disposable database.
- `git diff --check` and relevant backend checks pass for provider changes.

## Out of Scope

- zero-downtime logical replication;
- automatic high availability/failover;
- cross-region PostgreSQL replication;
- changing tenant isolation from database-per-school to schema-per-school;
- editing old applied migration files;
- moving R2/file storage.

## Risks

- Self-hosting shifts responsibility for patching, disk space, backup quality, restore drills, and security hardening to the project.
- Databasus reduces backup operational burden but does not replace monitoring or HA.
- A single self-hosted PostgreSQL node is cheaper but has more outage risk than a managed provider.
- The first provider refactor touches provisioning and deletion paths, so sandbox provisioning must be verified before production use.

## References

- `backend-admin/src/services/school_service.rs`
- `backend-admin/src/clients/neon_client.rs`
- `backend-school/src/db/pool_manager.rs`
- `backend-school/src/db/migration.rs`
- `docs/TESTING.md`
- PostgreSQL 18 release notes: https://www.postgresql.org/docs/release/18.4/
- Neon pricing/plans: https://neon.com/docs/introduction/plans
- Databasus: https://databasus.com/
