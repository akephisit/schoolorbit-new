-- The baseline contains two overlapping status constraints; one accidentally
-- excludes the already supported retirement status. Keep one canonical owner.
ALTER TABLE users DROP CONSTRAINT check_users_status;
ALTER TABLE users DROP CONSTRAINT chk_status;
ALTER TABLE users ADD CONSTRAINT check_users_status
    CHECK (status IN ('active', 'inactive', 'suspended', 'resigned', 'retired'));
