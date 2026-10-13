-- Contract only after a fresh executable reconciliation of each preserved tenant.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM schools WHERE
        db_runtime_password IS NULL OR db_migration_password IS NULL OR
        db_credentials_reconciled_at IS NULL OR
        db_credentials_reconciled_at < NOW() - INTERVAL '24 hours' OR
        (db_credentials_reconciliation->>'passed') IS DISTINCT FROM 'true' OR
        COALESCE(db_credentials_reconciliation->>'sourceDigest', '') = '' OR
        (db_credentials_reconciliation->>'sourceDigest') IS DISTINCT FROM
        (db_credentials_reconciliation->>'targetDigest')) THEN
        RAISE EXCEPTION 'DATABASE_CUTOVER_RECONCILIATION_REQUIRED';
    END IF;
END $$;
ALTER TABLE schools DROP COLUMN db_connection_string;
UPDATE schools SET config = config - 'db_id';
