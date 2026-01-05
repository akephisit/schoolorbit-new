-- ===================================================================
-- Migration 013: Student Management System
-- Description: เพิ่ม STUDENT role และ STUDENT_MANAGER role
-- Date: 2026-01-05
-- ===================================================================

-- ===================================================================
-- 1. Insert STUDENT Role
-- ===================================================================
INSERT INTO roles (code, name, name_en, category, level, permissions) VALUES
(
    'STUDENT',
    'นักเรียน',
    'Student',
    'student',
    1,
    ARRAY[
        'dashboard',
        'student.read.own',
        'student.update.own'
    ]
)
ON CONFLICT (code) DO UPDATE SET
    permissions = EXCLUDED.permissions,
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    category = EXCLUDED.category,
    level = EXCLUDED.level;

COMMENT ON COLUMN roles.permissions IS 'Permission codes (auto-synced from registry.rs)';

-- ===================================================================
-- 2. Insert STUDENT_MANAGER Role (สำหรับครู/Admin ที่จัดการนักเรียน)
-- ===================================================================
INSERT INTO roles (code, name, name_en, category, level, permissions) VALUES
(
    'STUDENT_MANAGER',
    'ผู้จัดการนักเรียน',
    'Student Manager',
    'administrative',
    50,
    ARRAY[
        'dashboard',
        'student.read.all',
        'student.create',
        'student.update.all'
    ]
)
ON CONFLICT (code) DO UPDATE SET
    permissions = EXCLUDED.permissions,
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    category = EXCLUDED.category,
    level = EXCLUDED.level;

-- ===================================================================
-- 3. Update ADMIN role to include student permissions
-- ===================================================================
UPDATE roles
SET permissions = array_cat(
    permissions,
    ARRAY[
        'student.read.all',
        'student.create',
        'student.update.all',
        'student.delete'
    ]::TEXT[]
)
WHERE code = 'ADMIN'
AND NOT (permissions @> ARRAY['student.read.all']::TEXT[]);

-- ===================================================================
-- 4. Add helpful comments
-- ===================================================================
COMMENT ON TABLE student_info IS 'ข้อมูลเฉพาะนักเรียน - ใช้ร่วมกับ users table (user_type = student)';
COMMENT ON COLUMN student_info.student_id IS 'รหัสนักเรียน (เช่น 66001, 66002)';
COMMENT ON COLUMN student_info.grade_level IS 'ระดับชั้น (เช่น ม.1, ม.2, ม.3)';
COMMENT ON COLUMN student_info.class_room IS 'ห้อง (เช่น 1, 2, 3)';

-- ===================================================================
-- 5. Verify installation
-- ===================================================================
SELECT 
    code,
    name,
    category,
    level,
    array_length(permissions, 1) as permission_count
FROM roles
WHERE code IN ('STUDENT', 'STUDENT_MANAGER')
ORDER BY level;
