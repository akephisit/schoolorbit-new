# PostgreSQL Self-Host Migration Design

Date: 2026-06-23
Updated: 2026-06-24
Status: Approved direction

## Goal

Move SchoolOrbit database hosting away from Neon to PostgreSQL 18 self-hosted. This is a self-host-only migration: after the change, `backend-admin` creates and deletes tenant databases directly through a PostgreSQL admin connection. The codebase will not keep a Neon runtime path.

Backups for the first cutover use PostgreSQL-native backup scripts, object storage, and restore drills. Databasus is not part of the first migration scope.

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
2. Remove Neon from the production provisioning/deletion code path.
3. Do not add a dual provider abstraction for `neon|self_hosted_postgres`.
4. Use one database per school tenant. Do not collapse tenants into shared schemas.
5. Do not do logical replication for the first migration.
6. Use native `pg_dump`/`pg_restore` scripts plus object storage and restore checks for the first backup foundation.
7. Existing Neon tenant databases are copied only when they are worth keeping.
8. Neon may remain as an external source archive for a short operational window, but not as an application fallback.

## Architecture

Replace Neon-specific database creation with one self-hosted provisioning unit:

```text
SchoolService
  -> SelfHostedPostgresProvisioner
       -> CREATE DATABASE schoolorbit_sandbox
       -> build self-hosted tenant connection string
       -> wait until tenant DB accepts connections
```

`SchoolService` keeps the current high-level flow:

1. validate school input;
2. create the tenant database through `SelfHostedPostgresProvisioner`;
3. insert the `schools` row with `status = 'provisioning'` and the self-hosted connection string;
4. call `backend-school /internal/provision`;
5. deploy/finalize the tenant frontend;
6. mark the school active.

The provisioner returns:

```text
ProvisionedDatabase {
  database_name: String,
  connection_string: String,
}
```

Deletion also uses `SelfHostedPostgresProvisioner`:

- terminate active sessions for the tenant DB;
- run `DROP DATABASE IF EXISTS ... WITH (FORCE)`;
- if deletion cannot complete, log/report manual cleanup needed before removing the operational record.

## Self-Hosted PostgreSQL Layout

The self-hosted cluster contains:

- one platform/admin database, e.g. `schoolorbit_admin`;
- one tenant database per school, e.g. `schoolorbit_sandbox`;
- `schoolorbit_provisioner`, which can create and drop tenant databases;
- `schoolorbit_admin_app`, used by `backend-admin` runtime;
- `schoolorbit_tenant_owner`, used in tenant connection strings and tenant migrations.

Required tenant migration capabilities:

- create `uuid-ossp`;
- create `pg_trgm`;
- create tables, indexes, enums, PL/pgSQL functions, triggers, and comments.

The first implementation uses one tenant owner role for all tenant databases. Per-tenant database roles and independent tenant password rotation are out of scope for this migration and can be added after the self-hosted cutover is stable.

## Environment

Use self-hosted PostgreSQL variables only:

```text
DATABASE_URL=postgresql://schoolorbit_admin_app:...@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:...@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=...
SELF_HOSTED_POSTGRES_SSLMODE=disable|require
BACKUP_STORAGE_URL=s3://...
BACKUP_RETENTION_DAYS=14
```

Production should not require `NEON_*` variables after the switch.

## Infrastructure

Production stack adds:

- PostgreSQL 18 container or VM-managed service with persistent volume;
- object storage for backup artifacts, preferably Cloudflare R2 or another S3-compatible target;
- host cron or systemd timers for backup and restore-check scripts;
- health checks for Postgres;
- disk usage monitoring and alerting;
- a restore runbook.

Databasus, pgBackRest, WAL-G, and point-in-time recovery are not part of the first migration. They remain future options after the self-hosted baseline is stable.

## Native Backup Foundation

The first backup foundation is script-based:

1. `scripts/backup_postgres.sh`
   - backs up `schoolorbit_admin`;
   - reads tenant database URLs from `backend-admin.schools`;
   - runs `pg_dump --format=custom` for each database;
   - uploads dumps to object storage;
   - enforces retention.
2. `scripts/restore_check_postgres.sh`
   - downloads or reads the latest backup artifact;
   - restores it into a disposable database;
   - runs sanity queries;
   - drops the disposable database.

Minimum restore checks:

- admin DB: `SELECT COUNT(*) FROM schools;`
- tenant DB: successful migration row count, permission count, and basic auth-related table presence.

## Migration Flow

### Phase 1: Stand Up Self-Hosted Database

1. Deploy PostgreSQL 18.
2. Create roles and `schoolorbit_admin`.
3. Restore or migrate `backend-admin` data from Neon.
4. Run `backend-admin` migrations.
5. Configure native backup scripts and object storage.
6. Perform a restore check into a disposable database.

### Phase 2: Replace Neon Provisioning

1. Remove `NeonClient` usage from school provisioning/deletion.
2. Add `SelfHostedPostgresProvisioner`.
3. Add config parsing for `SELF_HOSTED_POSTGRES_*`.
4. Update provisioning/deletion progress messages so they use generic tenant database wording.
5. Update rollback cleanup through the self-host provisioner.
6. Update compose/env examples.

### Phase 3: Sandbox Tenant

1. Provision a new sandbox tenant through the self-hosted flow.
2. Verify `backend-school /internal/provision` applies clean baseline migrations.
3. Verify login, `/api/auth/me`, tenant resolution, CORS, and smoke tests.
4. Run backup for the sandbox tenant.
5. Restore sandbox backup into a disposable database and validate sanity queries.

### Phase 4: Optional Data Copy for Important Tenants

For each tenant worth preserving:

1. Create a clean target tenant database on self-hosted PostgreSQL.
2. Run the existing clean baseline preparation script.
3. Run tenant data cutover in dry-run mode.
4. Run tenant data cutover in apply mode.
5. Update `schools.db_connection_string` to the self-hosted target connection string.
6. Hit the tenant through the real backend and verify smoke checks.

### Phase 5: Cut Over and Retire Neon

1. Deploy production with self-hosted env.
2. Remove required `NEON_*` env vars from production deployment.
3. Store final Neon dumps in object storage.
4. Keep Neon untouched briefly only as an external archive source, not as app fallback.
5. After validation and restore checks pass, delete Neon tenant databases and close paid Neon resources.

## Error Handling

Provisioning rollback must clean up in this order:

1. mark school row as failed when a row exists;
2. drop the newly created tenant DB when it is safe;
3. delete the school row only if the database cleanup succeeded and no tenant data was committed;
4. include cleanup errors in the returned provisioning error.

If tenant deletion cannot drop the database because connections are active, the app should report cleanup pending instead of pretending deletion fully succeeded.

Rollback after Neon resources are removed is restore-based:

- restore the latest `schoolorbit_admin` dump;
- restore required tenant dumps;
- redeploy services with self-hosted env;
- verify tenant login and smoke checks.

## Security and Privacy

The migration must preserve the current app-side PII standard:

- `national_id` remains AES-GCM encrypted by the backend using `ENCRYPTION_KEY`;
- `*_national_id_hash` remains HMAC blind-indexed using `BLIND_INDEX_KEY`;
- plaintext PII must not be logged during provisioning, cutover, dump, restore, or backup verification.

Connection strings are secrets. They must not be logged. Admin/provisioner credentials should not be shared with runtime tenant connections.

## Verification

Minimum verification before production cutover:

- `backend-admin` starts and runs migrations against self-hosted PostgreSQL.
- `backend-school` provisions a self-hosted sandbox tenant.
- tenant baseline applies with exactly one successful SQLx migration row at version `1`.
- `scripts/check_migration_rebaseline_ready.sh` passes for the self-hosted tenant.
- smoke test passes for sandbox.
- backup script creates admin and tenant backups.
- restore-check script restores a backup into a disposable database and validates sanity queries.
- `git diff --check` and relevant backend checks pass for provisioning changes.

## Out of Scope

- Neon runtime fallback;
- dual database provider abstraction;
- Databasus in the first migration;
- zero-downtime logical replication;
- automatic high availability/failover;
- cross-region PostgreSQL replication;
- changing tenant isolation from database-per-school to schema-per-school;
- editing old applied migration files;
- moving R2/file storage.

## Risks

- Self-hosting shifts responsibility for patching, disk space, backup quality, restore drills, and security hardening to the project.
- Native backup scripts are simple and transparent, but they do not provide point-in-time recovery by themselves.
- A single self-hosted PostgreSQL node is cheaper but has more outage risk than a managed provider.
- The first provisioner refactor touches provisioning and deletion paths, so sandbox provisioning must be verified before production use.

## References

- `backend-admin/src/services/school_service.rs`
- `backend-admin/src/clients/neon_client.rs`
- `backend-school/src/db/pool_manager.rs`
- `backend-school/src/db/migration.rs`
- `docs/TESTING.md`
- PostgreSQL 18 release notes: https://www.postgresql.org/docs/release/18.4/
- PostgreSQL backup documentation: https://www.postgresql.org/docs/current/backup.html
