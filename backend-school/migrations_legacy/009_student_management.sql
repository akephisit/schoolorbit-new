-- ===================================================================
-- Migration 011: Student Management System (FIXED for normalized schema)
-- Description: เพิ่ม STUDENT role และ STUDENT_MANAGER role
-- Date: 2026-01-05
-- Updated: 2026-01-11 - Use normalized schema
-- ===================================================================

-- ===================================================================
-- 1. Insert STUDENT Role
-- ===================================================================
INSERT INTO roles (code, name, name_en, user_type, level) VALUES
(
    'STUDENT',
    'นักเรียน',
    'Student',
    'student',
    1
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    user_type = EXCLUDED.user_type,
    level = EXCLUDED.level;

-- Assign permissions to STUDENT role
INSERT INTO role_permissions (role_id, permission_id)
SELECT 
    (SELECT id FROM roles WHERE code = 'STUDENT'),
    id
FROM permissions
WHERE code IN (
    'dashboard',
    'student.read.own',
    'student.update.own'
)
ON CONFLICT DO NOTHING;

-- ===================================================================
-- 2. Insert STUDENT_MANAGER Role
-- ===================================================================
INSERT INTO roles (code, name, name_en, user_type, level) VALUES
(
    'STUDENT_MANAGER',
    'ผู้จัดการนักเรียน',
    'Student Manager',
    'staff',
    50
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    user_type = EXCLUDED.user_type,
    level = EXCLUDED.level;

-- Assign permissions to STUDENT_MANAGER role
INSERT INTO role_permissions (role_id, permission_id)
SELECT 
    (SELECT id FROM roles WHERE code = 'STUDENT_MANAGER'),
    id
FROM permissions
WHERE code IN (
    'dashboard',
    'student.read.all',
    'student.create',
    'student.update.all'
)
ON CONFLICT DO NOTHING;

-- ===================================================================
-- 3. Update ADMIN role to include student permissions
-- ===================================================================
INSERT INTO role_permissions (role_id, permission_id)
SELECT 
    (SELECT id FROM roles WHERE code = 'ADMIN'),
    id
FROM permissions
WHERE code IN (
    'student.read.all',
    'student.create',
    'student.update.all',
    'student.delete'
)
ON CONFLICT DO NOTHING;

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
    r.code,
    r.name,
    r.user_type,
    r.level,
    COUNT(rp.permission_id) as permission_count
FROM roles r
LEFT JOIN role_permissions rp ON r.id = rp.role_id
WHERE r.code IN ('STUDENT', 'STUDENT_MANAGER')
GROUP BY r.id, r.code, r.name, r.user_type, r.level
ORDER BY r.level;
