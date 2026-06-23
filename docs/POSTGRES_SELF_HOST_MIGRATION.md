# PostgreSQL 18 Self-Host Migration Runbook

## Goal

Move SchoolOrbit from Neon-hosted PostgreSQL to PostgreSQL 18 self-hosted. After cutover, application runtime uses only self-hosted PostgreSQL; Neon is only a migration source, export source, or temporary archive while validation completes.

## Required Environment

Set these values through the deployment secret manager or host environment. Do not commit actual secret values.

```dotenv
POSTGRES_USER=schoolorbit_provisioner
POSTGRES_PASSWORD=<provisioner-password>
ADMIN_APP_PASSWORD=<backend-admin-app-password>
TENANT_OWNER_PASSWORD=<tenant-owner-password>

DATABASE_URL=postgresql://schoolorbit_admin_app:<backend-admin-app-password>@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:<provisioner-password>@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=<tenant-owner-password>
SELF_HOSTED_POSTGRES_SSLMODE=disable

BACKUP_ROOT=/opt/schoolorbit/backups/postgres
BACKUP_RETENTION_DAYS=14
BACKUP_RCLONE_REMOTE=
```

Keep `DATABASE_URL` in sync with `ADMIN_APP_PASSWORD`. Keep `SELF_HOSTED_POSTGRES_TENANT_PASSWORD` in sync with `TENANT_OWNER_PASSWORD`.

## Role And Admin DB Bootstrap

The compose files mount `scripts/init_postgres_roles.sh` into the PostgreSQL image. PostgreSQL runs that script only on first volume initialization, so changes to passwords or roles after the volume exists require manual verification and repair.

Verify roles and the admin database:

```bash
podman exec -it schoolorbit-postgres \
  psql -U "$POSTGRES_USER" -d postgres \
  -v ON_ERROR_STOP=1 \
  -c "SELECT rolname, rolcreatedb FROM pg_roles WHERE rolname IN ('schoolorbit_provisioner', 'schoolorbit_admin_app', 'schoolorbit_tenant_owner') ORDER BY rolname;" \
  -c "SELECT datname, pg_catalog.pg_get_userbyid(datdba) AS owner FROM pg_database WHERE datname = 'schoolorbit_admin';"
```

If the first-init script did not run, repair with values supplied from the environment:

```bash
podman exec -it schoolorbit-postgres \
  psql -U "$POSTGRES_USER" -d postgres \
  -v ON_ERROR_STOP=1 \
  -v provisioner_role="$POSTGRES_USER" \
  -v admin_app_password="$ADMIN_APP_PASSWORD" \
  -v tenant_owner_password="$TENANT_OWNER_PASSWORD"
```

Then run inside `psql`:

```sql
ALTER ROLE :"provisioner_role" WITH LOGIN CREATEDB;

SELECT format('CREATE ROLE schoolorbit_admin_app LOGIN PASSWORD %L', :'admin_app_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'schoolorbit_admin_app');
\gexec
ALTER ROLE schoolorbit_admin_app WITH LOGIN PASSWORD :'admin_app_password';

SELECT format('CREATE ROLE schoolorbit_tenant_owner LOGIN PASSWORD %L', :'tenant_owner_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'schoolorbit_tenant_owner');
\gexec
ALTER ROLE schoolorbit_tenant_owner WITH LOGIN PASSWORD :'tenant_owner_password';

GRANT schoolorbit_tenant_owner TO :"provisioner_role";

SELECT 'CREATE DATABASE schoolorbit_admin OWNER schoolorbit_admin_app'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'schoolorbit_admin');
\gexec
ALTER DATABASE schoolorbit_admin OWNER TO schoolorbit_admin_app;
GRANT CONNECT ON DATABASE schoolorbit_admin TO schoolorbit_admin_app;
GRANT CONNECT ON DATABASE postgres TO :"provisioner_role";
```

## Migrate Backend-Admin Data

Take a final custom-format admin dump from the old source before switching runtime configuration:

```bash
pg_dump "$OLD_ADMIN_DATABASE_URL" \
  --format=custom \
  --no-owner \
  --no-privileges \
  --file=/tmp/schoolorbit_admin_final.dump
```

Restore into the self-hosted admin database:

```bash
pg_restore \
  --dbname "$DATABASE_URL" \
  --no-owner \
  --no-privileges \
  --clean \
  --if-exists \
  /tmp/schoolorbit_admin_final.dump
```

Start `backend-admin` with the self-hosted environment and confirm its migrations run against `DATABASE_URL`. Keep the old source dump archived until production validation and restore checks pass.

## Backup And Restore Check

Run a native backup from the repository root:

```bash
DATABASE_URL="$DATABASE_URL" \
BACKUP_ROOT="$BACKUP_ROOT" \
BACKUP_RETENTION_DAYS="$BACKUP_RETENTION_DAYS" \
BACKUP_RCLONE_REMOTE="$BACKUP_RCLONE_REMOTE" \
  ./scripts/backup_postgres.sh
```

Check the newest admin dump:

```bash
latest_backup_dir="$(find "$BACKUP_ROOT" -mindepth 1 -maxdepth 1 -type d -name '????????T??????Z' | sort | tail -1)"
admin_dump="$(find "$latest_backup_dir" -maxdepth 1 -type f -name 'schoolorbit_admin_*.dump' | sort | tail -1)"

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=admin \
RESTORE_CHECK_DUMP_PATH="$admin_dump" \
  ./scripts/restore_check_postgres.sh
```

Check a tenant dump:

```bash
tenant_dump="$(find "$latest_backup_dir" -maxdepth 1 -type f -name 'tenant_*.dump' | sort | tail -1)"

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=tenant \
RESTORE_CHECK_DUMP_PATH="$tenant_dump" \
  ./scripts/restore_check_postgres.sh
```

Schedule backups with cron or a systemd timer only after at least one admin and one tenant restore check pass.

## Provision Sandbox Tenant

Provision a sandbox school through the normal admin UI or API. Confirm the new tenant has a self-hosted connection string in `backend-admin.schools.db_connection_string`, but do not print the full URL in logs or tickets.

Run the clean-baseline audit against the sandbox tenant:

```bash
MIGRATION_AUDIT_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_sandbox?sslmode=disable" \
  ./scripts/check_migration_rebaseline_ready.sh
```

Run the sandbox smoke test with credentials supplied through environment variables:

```bash
SMOKE_TENANT_URL=https://sandbox.schoolorbit.app \
SMOKE_API_URL=https://school-api.schoolorbit.app \
SMOKE_USERNAME="$SMOKE_USERNAME" \
SMOKE_PASSWORD="$SMOKE_PASSWORD" \
  ./scripts/smoke_test.sh
```

After the sandbox smoke test passes, run `scripts/backup_postgres.sh` again and restore-check the sandbox tenant dump.

## Optional Copy For Important Old Tenants

Only copy tenants that need to be preserved. For each tenant, create or provision a clean target tenant database on self-hosted PostgreSQL first.

Prepare the clean target:

```bash
PREPARE_CLEAN_TENANT_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
PREPARE_CLEAN_TENANT_CONFIRM=public \
PREPARE_CLEAN_TENANT_ALLOW_NON_TEST=1 \
  ./scripts/prepare_clean_tenant_db.sh
```

Dry-run the data copy from the old source export database:

```bash
CUTOVER_SOURCE_DATABASE_URL="$OLD_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
  ./scripts/cutover_tenant_data.sh
```

Apply the data copy only after the dry run validates table lists and row counts:

```bash
CUTOVER_MODE=apply \
CUTOVER_SOURCE_DATABASE_URL="$OLD_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
CUTOVER_TARGET_SCHEMA=public \
CUTOVER_ALLOW_PUBLIC_TARGET=1 \
CUTOVER_ALLOW_NON_TEST_TARGET=1 \
CUTOVER_CONFIRM_TARGET_TRUNCATE=public \
  ./scripts/cutover_tenant_data.sh
```

Update `backend-admin.schools.db_connection_string` for the tenant only after apply mode passes and smoke checks succeed against the target.

## Shutdown Paid External DB Resources

After production validation is complete:

1. Take final admin and tenant dumps from any old external source that remains.
2. Store final dumps in object storage or another approved archive location.
3. Run restore checks for representative admin and tenant dumps.
4. Verify production login, `/api/auth/me`, tenant resolution, and smoke tests.
5. Confirm scheduled backups and restore checks are running against self-hosted PostgreSQL.
6. Delete old external databases and close paid external database resources.

Keep the final source exports long enough to satisfy operational rollback and audit needs, but do not configure the application to fall back to them at runtime.
