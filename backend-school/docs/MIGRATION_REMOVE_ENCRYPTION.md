# Migration: Remove pgcrypto encryption from national_id

**Decision:** เปลี่ยน `national_id` จาก encrypted (BYTEA) เป็น plaintext (TEXT)

## เหตุผล:

1. **pgcrypto ไม่ทำงานกับ Neon** - session variables ไม่เสถียร
2. **Application-level encryption ซับซ้อน** - ต้องแก้ 14 ที่
3. **Neon มี encryption at rest** - ปลอดภัยอยู่แล้ว
4. **Performance** - ไม่ต้อง decrypt ทุกครั้ง

## Migration Steps:

### 1. สร้าง migration ใหม่:

\`\`\`sql
-- migrations/021_convert_national_id_to_text.sql

-- Decrypt existing data and convert to TEXT
BEGIN;

-- Add temporary column
ALTER TABLE users ADD COLUMN national_id_temp TEXT;

-- Decrypt and copy (ถ้ามีข้อมูลอยู่)
-- NOTE: ต้อง run ด้วย ENCRYPTION_KEY
DO $$
DECLARE
    rec RECORD;
    decrypted TEXT;
BEGIN
    FOR rec IN SELECT id, national_id FROM users WHERE national_id IS NOT NULL
    LOOP
        BEGIN
            decrypted := pgp_sym_decrypt(rec.national_id, current_setting('app.encryption_key'));
            UPDATE users SET national_id_temp = decrypted WHERE id = rec.id;
        EXCEPTION WHEN OTHERS THEN
            -- If decrypt fails, skip (already plaintext?)
            RAISE NOTICE 'Failed to decrypt for user %', rec.id;
        END;
    END LOOP;
END $$;

-- Drop old column
ALTER TABLE users DROP COLUMN national_id;

-- Rename temp column
ALTER TABLE users RENAME COLUMN national_id_temp TO national_id;

-- Recreate unique constraint
CREATE UNIQUE INDEX idx_users_national_id ON users(national_id) WHERE national_id IS NOT NULL;

COMMIT;
\`\`\`

### 2. ลบ pgcrypto queries ทั้งหมด:

Replace all:
\`\`\`sql
-- Before
pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id

-- After  
national_id
\`\`\`

### 3. Clean up:

- ❌ ลบ `field_encryption.rs` (ไม่ใช้แล้ว)
- ❌ ลบ `decrypt_helpers.rs` (ไม่ใช้แล้ว)
- ❌ ลบ dependencies: aes-gcm, base64, sha256, rand

## Security:

✅ Neon มี encryption at rest  
✅ HTTPS สำหรับ transit  
✅ Database access control  
✅ มากพอสำหรับ national ID  

## Alternative (ถ้าต้องการ encrypt):

ใช้ application-level encryption แต่ต้องแก้ทุกที่ที่ query national_id (14 ที่)

**Recommended: Go with plaintext!** 🚀
