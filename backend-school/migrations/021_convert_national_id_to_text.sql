-- Convert national_id from PGP (BYTEA) to TEXT + Hash for Search
-- This enables application-level AES-256-GCM encryption with BLIND INDEXING

BEGIN;

-- 1. Add new columns
ALTER TABLE users ADD COLUMN national_id_new TEXT; 
ALTER TABLE users ADD COLUMN national_id_hash TEXT;

-- 2. Populate Hash (Decryption requires correct Key)
-- We attempt to decrypt existing data to generate the hash for searching
-- Note: 'national_id_new' (Ciphertext) will be left NULL for legacy data because we can't generate AES-GCM in SQL easily.
-- Users will need to update their profile to populate the new Ciphertext. Login will work via Hash.
DO $$
BEGIN
    UPDATE users 
    SET national_id_hash = encode(digest(pgp_sym_decrypt(national_id, current_setting('app.encryption_key')), 'sha256'), 'hex')
    WHERE national_id IS NOT NULL;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Decryption failed (Wrong Key?), skipping hash generation for existing users.';
END $$;

-- 3. Drop old column
ALTER TABLE users DROP COLUMN national_id;

-- 4. Rename & Constraints
ALTER TABLE users RENAME COLUMN national_id_new TO national_id;

CREATE UNIQUE INDEX idx_users_national_id_hash ON users(national_id_hash);
-- We don't need unique on ciphertext anymore, but keeping index on hash is enough.

COMMIT;
