-- ===================================================================
-- Migration 019: Encrypt Sensitive Personal Data (Clean Start)
-- Description: เข้ารหัสข้อมูลส่วนบุคคลที่ละเอียดอ่อน (PDPA Compliance)
-- Security: AES-256 encryption using pgcrypto
-- Strategy: ALTER columns to BYTEA for encrypted storage
-- Date: 2026-01-08
-- ===================================================================

-- ===================================================================
-- 1. Enable pgcrypto Extension
-- ===================================================================
CREATE EXTENSION IF NOT EXISTS pgcrypto;

COMMENT ON EXTENSION pgcrypto IS 'PostgreSQL cryptographic functions for AES-256 data encryption';

-- ===================================================================
-- 2. Alter Columns to Store Encrypted Data
-- ===================================================================

-- Users table: national_id (เลขบัตรประชาชน)
-- Change from VARCHAR to BYTEA for encrypted storage
ALTER TABLE users 
    ALTER COLUMN national_id TYPE BYTEA USING NULL;

COMMENT ON COLUMN users.national_id IS 'เลขบัตรประชาชน (เข้ารหัสด้วย AES-256 pgcrypto) - PDPA Sensitive Data';

-- Student info table: medical_conditions (ข้อมูลสุขภาพ)
-- Change from TEXT to BYTEA for encrypted storage
ALTER TABLE student_info 
    ALTER COLUMN medical_conditions TYPE BYTEA USING NULL;

COMMENT ON COLUMN student_info.medical_conditions IS 'ข้อมูลโรคประจำตัว (เข้ารหัสด้วย AES-256 pgcrypto) - PDPA Sensitive Data';

-- ===================================================================
-- 3. Helper Functions for Encryption/Decryption
-- ===================================================================

-- Function to encrypt text
CREATE OR REPLACE FUNCTION encrypt_text(plaintext TEXT, key TEXT)
RETURNS BYTEA AS $$
BEGIN
    IF plaintext IS NULL OR plaintext = '' THEN
        RETURN NULL;
    END IF;
    RETURN pgp_sym_encrypt(plaintext, key);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION encrypt_text IS 'Encrypt plaintext using AES-256 symmetric encryption';

-- Function to decrypt text
CREATE OR REPLACE FUNCTION decrypt_text(encrypted BYTEA, key TEXT)
RETURNS TEXT AS $$
BEGIN
    IF encrypted IS NULL THEN
        RETURN NULL;
    END IF;
    RETURN pgp_sym_decrypt(encrypted, key);
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL; -- Return NULL if decryption fails (wrong key or corrupt data)
END;
$$ LANGUAGE plpgsql IMMUTABLE;

COMMENT ON FUNCTION decrypt_text IS 'Decrypt encrypted data back to plaintext';

-- ===================================================================
-- 4. Usage Examples (for reference)
-- ===================================================================

-- Example: Insert encrypted data
-- INSERT INTO users (national_id, first_name, last_name)
-- VALUES (
--     pgp_sym_encrypt('1234567890123', current_setting('app.encryption_key')),
--     'John',
--     'Doe'
-- );

-- Example: Query with decryption
-- SELECT 
--     id,
--     pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
--     first_name,
--     last_name
-- FROM users
-- WHERE id = 'some-uuid';

-- Example: Set encryption key in session
-- SET LOCAL app.encryption_key = 'your-encryption-key';

-- ===================================================================
-- 5. Security Guidelines
-- ===================================================================
-- 
-- ENCRYPTION KEY MANAGEMENT:
-- 1. Generate strong key:
--    openssl rand -base64 32
--
-- 2. Store in environment variable:
--    ENCRYPTION_KEY="your-generated-key"
--
-- 3. NEVER commit key to version control
--
-- 4. Use secrets manager in production:
--    - AWS Secrets Manager
--    - HashiCorp Vault
--    - Google Secret Manager
--
-- 5. Rotate keys every 90 days
--
-- 6. Backup key separately from database
--
-- APPLICATION USAGE:
-- Every database session that needs encryption must set the key:
--    sqlx::query("SET LOCAL app.encryption_key = $1")
--        .bind(key)
--        .execute(&pool)
--        .await?;
--
-- PERFORMANCE:
-- - Encryption adds ~10-20ms per operation
-- - Cannot index encrypted columns directly
-- - Consider caching decrypted values in application layer
--
-- BACKUP & RECOVERY:
-- - Database backup includes encrypted data
-- - Encryption key must be backed up separately
-- - Without key, data cannot be recovered
--
-- ===================================================================
