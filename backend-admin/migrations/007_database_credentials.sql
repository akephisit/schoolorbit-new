-- Expand only. Existing DSNs remain for explicit cutover reconciliation, never runtime fallback.
ALTER TABLE schools ADD COLUMN db_runtime_password BYTEA;
ALTER TABLE schools ADD COLUMN db_migration_password BYTEA;
ALTER TABLE schools ADD COLUMN db_credentials_reconciled_at TIMESTAMPTZ;
ALTER TABLE schools ADD COLUMN db_credentials_reconciliation JSONB;
ALTER TABLE schools ADD CONSTRAINT paired_database_credentials CHECK (
    (db_runtime_password IS NULL) = (db_migration_password IS NULL)
);
ALTER TABLE schools ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE admin_users ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE deployment_history ALTER COLUMN id SET DEFAULT uuidv7();
