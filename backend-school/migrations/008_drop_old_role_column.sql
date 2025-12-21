-- Migration 008: Drop old 'role' column and use 'user_type' instead
-- ปัญหา: column 'role' จาก migration 001 ยังมี NOT NULL constraint
-- แก้ไข: ลบ column 'role' เก่าออก เพราะตอนนี้ใช้ 'user_type' แทนแล้ว

-- Drop the old 'role' column
ALTER TABLE users 
    DROP COLUMN IF EXISTS role CASCADE;

-- Verify user_type exists and has proper constraint
-- (ถ้ายังไม่มีให้เพิ่ม - แต่ควรมีอยู่แล้วจาก migration 005)
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'users' AND column_name = 'user_type'
    ) THEN
        ALTER TABLE users 
            ADD COLUMN user_type VARCHAR(50) NOT NULL DEFAULT 'staff';
    END IF;
END $$;

-- Ensure user_type has the check constraint
ALTER TABLE users 
    DROP CONSTRAINT IF EXISTS chk_user_type;
    
ALTER TABLE users 
    ADD CONSTRAINT chk_user_type 
    CHECK (user_type IN ('student', 'staff', 'parent'));

-- Update index
DROP INDEX IF EXISTS idx_users_role;
CREATE INDEX IF NOT EXISTS idx_users_user_type ON users(user_type);

COMMENT ON COLUMN users.user_type IS 'ประเภทผู้ใช้: student, staff, parent (แทนที่ role column เก่า)';
