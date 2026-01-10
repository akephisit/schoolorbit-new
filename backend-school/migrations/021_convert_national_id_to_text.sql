-- Convert national_id from BYTEA (pgcrypto) to TEXT (application encryption)
-- This allows us to use application-level AES-256-GCM encryption instead of pgcrypto

BEGIN;

-- Step 1: Add temporary TEXT column
ALTER TABLE users ADD COLUMN national_id_new TEXT;

-- Step 2: For existing encrypted data, we need to decrypt first
-- NOTE: This requires ENCRYPTION_KEY to be set!
-- Run this manually if you have existing encrypted data:
-- 
-- DO $$
-- DECLARE
--     rec RECORD;
--     decrypted TEXT;
-- BEGIN
--     FOR rec IN SELECT id, national_id FROM users WHERE national_id IS NOT NULL
--     LOOP
--         BEGIN
--             -- Decrypt using pgcrypto
--             decrypted := pgp_sym_decrypt(rec.national_id, current_setting('app.encryption_key'));
--             -- Re-encrypt using application layer (do this in Rust instead)
--             UPDATE users SET national_id_new = decrypted WHERE id = rec.id;
--         EXCEPTION WHEN OTHERS THEN
--             RAISE NOTICE 'Failed to decrypt for user %, keeping as-is', rec.id;
--         END;
--     END LOOP;
-- END $$;

-- Step 3: Drop old BYTEA column
ALTER TABLE users DROP COLUMN national_id;

-- Step 4: Rename new column
ALTER TABLE users RENAME COLUMN national_id_new TO national_id;

-- Step 5: Recreate unique constraint
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_national_id 
ON users(national_id) WHERE national_id IS NOT NULL AND national_id != '';

COMMIT;

-- After this migration:
-- 1. national_id is now TEXT type
-- 2. Application will encrypt/decrypt using AES-256-GCM
-- 3. Stored as base64-encoded ciphertext in database
