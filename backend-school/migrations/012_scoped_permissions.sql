-- ===================================================================
-- Migration 012: Scoped Permissions
-- Description: Add scope-based permissions for data-level authorization
-- Date: 2025-12-23
-- ===================================================================

-- ===================================================================
-- 1. Add Scope Column to Permissions Table
-- ===================================================================
ALTER TABLE permissions 
ADD COLUMN IF NOT EXISTS scope VARCHAR(50) DEFAULT 'all';

CREATE INDEX IF NOT EXISTS idx_permissions_scope ON permissions(scope);

COMMENT ON COLUMN permissions.scope IS 'Permission scope: own (assigned resources), department, all (admin)';

-- ===================================================================
-- 2. Insert Scoped Permission Variants
-- ===================================================================

-- Attendance Permissions
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('attendance.read.own', 'ดูการเข้าเรียนของตัวเอง', 'attendance', 'read', 'own', 'ดูการเข้าเรียนของห้องที่สอน'),
  ('attendance.read.all', 'ดูการเข้าเรียนทั้งหมด', 'attendance', 'read', 'all', 'ดูการเข้าเรียนทุกห้อง'),
  ('attendance.create.own', 'บันทึกการเข้าเรียนของตัวเอง', 'attendance', 'create', 'own', 'บันทึกการเข้าเรียนของห้องที่สอน'),
  ('attendance.create.all', 'บันทึกการเข้าเรียนทั้งหมด', 'attendance', 'create', 'all', 'บันทึกการเข้าเรียนทุกห้อง'),
  ('attendance.update.own', 'แก้ไขการเข้าเรียนของตัวเอง', 'attendance', 'update', 'own', 'แก้ไขการเข้าเรียนของห้องที่สอน'),
  ('attendance.update.all', 'แก้ไขการเข้าเรียนทั้งหมด', 'attendance', 'update', 'all', 'แก้ไขการเข้าเรียนทุกห้อง'),
  ('attendance.delete.own', 'ลบการเข้าเรียนของตัวเอง', 'attendance', 'delete', 'own', 'ลบการเข้าเรียนของห้องที่สอน'),
  ('attendance.delete.all', 'ลบการเข้าเรียนทั้งหมด', 'attendance', 'delete', 'all', 'ลบการเข้าเรียนทุกห้อง')
ON CONFLICT (code) DO NOTHING;

-- Grades Permissions
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('grades.read.own', 'ดูคะแนนของตัวเอง', 'grades', 'read', 'own', 'ดูคะแนนของห้องที่สอน'),
  ('grades.read.department', 'ดูคะแนนของฝ่าย', 'grades', 'read', 'department', 'ดูคะแนนของฝ่ายที่สังกัด'),
  ('grades.read.all', 'ดูคะแนนทั้งหมด', 'grades', 'read', 'all', 'ดูคะแนนทุกห้อง'),
  ('grades.create.own', 'บันทึกคะแนนของตัวเอง', 'grades', 'create', 'own', 'บันทึกคะแนนของห้องที่สอน'),
  ('grades.create.all', 'บันทึกคะแนนทั้งหมด', 'grades', 'create', 'all', 'บันทึกคะแนนทุกห้อง'),
  ('grades.update.own', 'แก้ไขคะแนนของตัวเอง', 'grades', 'update', 'own', 'แก้ไขคะแนนของห้องที่สอน'),
  ('grades.update.department', 'แก้ไขคะแนนของฝ่าย', 'grades', 'update', 'department', 'แก้ไขคะแนนของฝ่าย'),
  ('grades.update.all', 'แก้ไขคะแนนทั้งหมด', 'grades', 'update', 'all', 'แก้ไขคะแนนทุกห้อง'),
  ('grades.delete.own', 'ลบคะแนนของตัวเอง', 'grades', 'delete', 'own', 'ลบคะแนนของห้องที่สอน'),
  ('grades.delete.all', 'ลบคะแนนทั้งหมด', 'grades', 'delete', 'all', 'ลบคะแนนทุกห้อง')
ON CONFLICT (code) DO NOTHING;

-- Students Permissions
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('students.read.own', 'ดูนักเรียนของตัวเอง', 'students', 'read', 'own', 'ดูนักเรียนในห้องที่สอน'),
  ('students.read.all', 'ดูนักเรียนทั้งหมด', 'students', 'read', 'all', 'ดูนักเรียนทุกห้อง'),
  ('students.update.own', 'แก้ไขนักเรียนของตัวเอง', 'students', 'update', 'own', 'แก้ไขนักเรียนในห้องที่สอน'),
  ('students.update.all', 'แก้ไขนักเรียนทั้งหมด', 'students', 'update', 'all', 'แก้ไขนักเรียนทุกห้อง')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 3. Update Default Roles with Scoped Permissions
-- ===================================================================

-- TEACHER: Only assigned classes (own scope)
UPDATE roles
SET permissions = ARRAY[
  'dashboard.read',
  'attendance.read.own',
  'attendance.create.own',
  'attendance.update.own',
  'grades.read.own',
  'grades.create.own',
  'grades.update.own',
  'students.read.own'
]
WHERE code = 'TEACHER' AND is_active = true;

-- DEPT_HEAD: Department level access
UPDATE roles
SET permissions = array_cat(permissions, ARRAY[
  'grades.read.department',
  'grades.update.department'
])
WHERE code = 'DEPT_HEAD' AND is_active = true;

-- VICE_DIRECTOR: All access for attendance and grades
UPDATE roles
SET permissions = array_cat(permissions, ARRAY[
  'attendance.read.all',
  'attendance.update.all',
  'grades.read.all',
  'grades.update.all'
])
WHERE code = 'VICE_DIRECTOR' AND is_active = true;

-- DIRECTOR: Full access
UPDATE roles
SET permissions = array_cat(permissions, ARRAY[
  'attendance.read.all',
  'attendance.create.all',
  'attendance.update.all',
  'attendance.delete.all',
  'grades.read.all',
  'grades.create.all',
  'grades.update.all',
  'grades.delete.all',
  'students.read.all',
  'students.update.all'
])
WHERE code = 'DIRECTOR' AND is_active = true;

-- ===================================================================
-- 4. Update Existing Permissions with Default Scope
-- ===================================================================
UPDATE permissions 
SET scope = 'all'
WHERE scope IS NULL;

-- ===================================================================
-- 5. Verification Query
-- ===================================================================
SELECT 
    module,
    scope,
    COUNT(*) as permission_count
FROM permissions
GROUP BY module, scope
ORDER BY module, scope;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Scope-based Permissions:
--   - own: User can only access/modify assigned resources
--   - department: User can access department-level resources
--   - all: User can access all resources (admin level)
--
-- Backward Compatibility:
--   - Existing permissions without scope default to 'all'
--   - Parser treats 2-part format (resource.action) as 'all' scope
--
-- Use Cases:
--   - Teacher with attendance.update.own: Can only update assigned classes
--   - Dept Head with grades.read.department: Can read department grades
--   - Director with grades.update.all: Can update all grades
--
-- Next Steps:
--   - Implement scope checking in handlers
--   - Add check_resource_ownership function
--   - Test with real scenarios
-- ===================================================================
