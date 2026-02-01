-- Migration: Add username column and switch authentication method
-- Description: Adds username column. 
-- WARNING: This migration TRUNCATES the users table as per user instruction "don't care about old data".

-- 1. Truncate users table to ensure clean state and avoid constraint issues
TRUNCATE TABLE users CASCADE;

-- 2. Add username column (Idempotent)
ALTER TABLE users 
ADD COLUMN IF NOT EXISTS username VARCHAR(50);

-- Update existing NULL usernames if any (though we truncated above)
-- UPDATE users SET username = 'temp_' || id WHERE username IS NULL;

-- Make it NOT NULL if it was nullable
ALTER TABLE users 
ALTER COLUMN username SET NOT NULL;

-- 3. Add constraint (Idempotent approach for Postgres)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'users_username_key') THEN
        ALTER TABLE users ADD CONSTRAINT users_username_key UNIQUE (username);
    END IF;
END $$;

-- 4. Create index for fast login lookup
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

-- 5. Optional backend cleanup: national_id is no longer unique constraint if it was one (it was national_id_hash)
-- We keep national_id_hash unique for now to prevent duplicate national IDs being registered, 
-- even if not used for login.
