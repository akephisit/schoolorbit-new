-- Add migration metadata columns to schools table

ALTER TABLE schools 
ADD COLUMN IF NOT EXISTS migration_version INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_migrated_at TIMESTAMP,
ADD COLUMN IF NOT EXISTS migration_status VARCHAR(20) DEFAULT 'pending',
ADD COLUMN IF NOT EXISTS migration_error TEXT;

-- Create index for faster queries
CREATE INDEX IF NOT EXISTS idx_schools_migration_status ON schools(migration_status);
CREATE INDEX IF NOT EXISTS idx_schools_migration_version ON schools(migration_version);

-- Add comment
COMMENT ON COLUMN schools.migration_version IS 'Latest migration version applied to this school database';
COMMENT ON COLUMN schools.last_migrated_at IS 'Timestamp of last successful migration';
COMMENT ON COLUMN schools.migration_status IS 'Migration status: pending, migrated, failed, outdated';
COMMENT ON COLUMN schools.migration_error IS 'Error message if migration failed';
