-- ===================================================================
-- Migration 012: Scoped Permissions System
-- Description: Implement scope-based permissions for data-level authorization
-- Date: 2025-12-24
-- ===================================================================

-- ===================================================================
-- 1. Add Scope Column to Permissions Table
-- ===================================================================
ALTER TABLE permissions 
ADD COLUMN IF NOT EXISTS scope VARCHAR(50) NOT NULL DEFAULT 'all';

CREATE INDEX IF NOT EXISTS idx_permissions_scope ON permissions(scope);

COMMENT ON COLUMN permissions.scope IS 'Permission scope: own (assigned), department (dept-level), all (admin)';

-- ===================================================================
-- 2. Delete Old 2-Part Permissions
-- ===================================================================
DELETE FROM permissions WHERE code IN (
  -- Dashboard
  'dashboard.read',
  -- Subjects
  'subjects.read', 'subjects.create', 'subjects.update', 'subjects.delete',
  -- Classes
  'classes.read', 'classes.create', 'classes.update', 'classes.delete',
  -- Calendar
  'calendar.read', 'calendar.create', 'calendar.update', 'calendar.delete',
  -- Settings
  'settings.read', 'settings.update',
  -- Roles
  'roles.read', 'roles.create', 'roles.update', 'roles.delete',
  -- Staff (if any old format exists)
  'staff.read', 'staff.create', 'staff.update', 'staff.delete'
);

-- ===================================================================
-- 3. Insert All Scoped Permissions
-- ===================================================================

-- Dashboard (read-only)
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('dashboard.read.all', 'ดูแดชบอร์ด', 'dashboard', 'read', 'all', 'เข้าถึงหน้าแดชบอร์ด')
ON CONFLICT (code) DO NOTHING;

-- Subjects
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('subjects.read.all', 'ดูวิชาทั้งหมด', 'subjects', 'read', 'all', 'ดูรายวิชาทั้งหมด'),
  ('subjects.create.all', 'สร้างวิชา', 'subjects', 'create', 'all', 'สร้างรายวิชาใหม่'),
  ('subjects.update.all', 'แก้ไขวิชา', 'subjects', 'update', 'all', 'แก้ไขรายวิชา'),
  ('subjects.delete.all', 'ลบวิชา', 'subjects', 'delete', 'all', 'ลบรายวิชา')
ON CONFLICT (code) DO NOTHING;

-- Classes
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('classes.read.own', 'ดูห้องของตัวเอง', 'classes', 'read', 'own', 'ดูห้องที่สอน'),
  ('classes.read.all', 'ดูห้องทั้งหมด', 'classes', 'read', 'all', 'ดูห้องเรียนทุกห้อง'),
  ('classes.create.all', 'สร้างห้อง', 'classes', 'create', 'all', 'สร้างห้องเรียนใหม่'),
  ('classes.update.all', 'แก้ไขห้อง', 'classes', 'update', 'all', 'แก้ไขห้องเรียน'),
  ('classes.delete.all', 'ลบห้อง', 'classes', 'delete', 'all', 'ลบห้องเรียน')
ON CONFLICT (code) DO NOTHING;

-- Calendar
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('calendar.read.all', 'ดูปฏิทิน', 'calendar', 'read', 'all', 'เข้าถึงปฏิทินและกิจกรรม'),
  ('calendar.create.all', 'สร้างกิจกรรม', 'calendar', 'create', 'all', 'สร้างกิจกรรมในปฏิทิน'),
  ('calendar.update.all', 'แก้ไขกิจกรรม', 'calendar', 'update', 'all', 'แก้ไขกิจกรรม'),
  ('calendar.delete.all', 'ลบกิจกรรม', 'calendar', 'delete', 'all', 'ลบกิจกรรม')
ON CONFLICT (code) DO NOTHING;

-- Settings
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('settings.read.all', 'ดูการตั้งค่า', 'settings', 'read', 'all', 'เข้าถึงการตั้งค่าระบบ'),
  ('settings.update.all', 'แก้ไขการตั้งค่า', 'settings', 'update', 'all', 'แก้ไขการตั้งค่าระบบ')
ON CONFLICT (code) DO NOTHING;

-- Roles
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('roles.read.all', 'ดูบทบาท', 'roles', 'read', 'all', 'ดูบทบาทและสิทธิ์'),
  ('roles.create.all', 'สร้างบทบาท', 'roles', 'create', 'all', 'สร้างบทบาทใหม่'),
  ('roles.update.all', 'แก้ไขบทบาท', 'roles', 'update', 'all', 'แก้ไขบทบาท'),
  ('roles.delete.all', 'ลบบทบาท', 'roles', 'delete', 'all', 'ลบบทบาท')
ON CONFLICT (code) DO NOTHING;

-- Staff
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('staff.read.own', 'ดูข้อมูลตัวเอง', 'staff', 'read', 'own', 'ดูข้อมูลบุคลากรของตัวเอง'),
  ('staff.read.all', 'ดูบุคลากรทั้งหมด', 'staff', 'read', 'all', 'ดูข้อมูลบุคลากรทุกคน'),
  ('staff.create.all', 'เพิ่มบุคลากร', 'staff', 'create', 'all', 'เพิ่มบุคลากรใหม่'),
  ('staff.update.own', 'แก้ไขข้อมูลตัวเอง', 'staff', 'update', 'own', 'แก้ไขข้อมูลตัวเอง'),
  ('staff.update.all', 'แก้ไขบุคลากร', 'staff', 'update', 'all', 'แก้ไขข้อมูลบุคลากรทุกคน'),
  ('staff.delete.all', 'ลบบุคลากร', 'staff', 'delete', 'all', 'ลบบุคลากร')
ON CONFLICT (code) DO NOTHING;

-- Attendance
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

-- Grades
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

-- Students
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('students.read.own', 'ดูนักเรียนของตัวเอง', 'students', 'read', 'own', 'ดูนักเรียนในห้องที่สอน'),
  ('students.read.all', 'ดูนักเรียนทั้งหมด', 'students', 'read', 'all', 'ดูนักเรียนทุกห้อง'),
  ('students.create.all', 'เพิ่มนักเรียน', 'students', 'create', 'all', 'เพิ่มนักเรียนใหม่'),
  ('students.update.own', 'แก้ไขนักเรียนของตัวเอง', 'students', 'update', 'own', 'แก้ไขนักเรียนในห้องที่สอน'),
  ('students.update.all', 'แก้ไขนักเรียนทั้งหมด', 'students', 'update', 'all', 'แก้ไขนักเรียนทุกคน'),
  ('students.delete.all', 'ลบนักเรียน', 'students', 'delete', 'all', 'ลบนักเรียน')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 4. Update All Roles with Scoped Permissions
-- ===================================================================

-- TEACHER: Own scope for assigned resources
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'subjects.read.all',
  'classes.read.own',
  'calendar.read.all',
  'attendance.read.own',
  'attendance.create.own',
  'attendance.update.own',
  'grades.read.own',
  'grades.create.own',
  'grades.update.own',
  'students.read.own',
  'staff.read.own',
  'staff.update.own'
] WHERE code = 'TEACHER' AND is_active = true;

-- DEPT_HEAD: Department level access
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'subjects.read.all',
  'classes.read.all',
  'calendar.read.all',
  'grades.read.department',
  'grades.update.department',
  'students.read.all',
  'staff.read.all'
] WHERE code = 'DEPT_HEAD' AND is_active = true;

-- VICE_DIRECTOR: All access for academic operations
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'subjects.read.all',
  'classes.read.all',
  'calendar.read.all',
  'calendar.create.all',
  'attendance.read.all',
  'attendance.update.all',
  'grades.read.all',
  'grades.update.all',
  'students.read.all',
  'staff.read.all'
] WHERE code = 'VICE_DIRECTOR' AND is_active = true;

-- DIRECTOR: Full access to everything
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'subjects.read.all',
  'subjects.create.all',
  'subjects.update.all',
  'subjects.delete.all',
  'classes.read.all',
  'classes.create.all',
  'classes.update.all',
  'classes.delete.all',
  'calendar.read.all',
  'calendar.create.all',
  'calendar.update.all',
  'calendar.delete.all',
  'settings.read.all',
  'settings.update.all',
  'attendance.read.all',
  'attendance.create.all',
  'attendance.update.all',
  'attendance.delete.all',
  'grades.read.all',
  'grades.create.all',
  'grades.update.all',
  'grades.delete.all',
  'students.read.all',
  'students.create.all',
  'students.update.all',
  'students.delete.all',
  'staff.read.all',
  'staff.create.all',
  'staff.update.all',
  'staff.delete.all',
  'roles.read.all'
] WHERE code = 'DIRECTOR' AND is_active = true;

-- SECRETARY: Administrative support
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'calendar.read.all',
  'calendar.create.all',
  'settings.read.all',
  'students.read.all',
  'staff.read.all'
] WHERE code = 'SECRETARY' AND is_active = true;

-- ADMIN: System and roles management
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'roles.read.all',
  'roles.create.all',
  'roles.update.all',
  'roles.delete.all',
  'staff.read.all',
  'staff.create.all',
  'staff.update.all'
] WHERE code = 'ADMIN' AND is_active = true;

-- ===================================================================
-- 5. Verification
-- ===================================================================
SELECT 
    module,
    scope,
    COUNT(*) as permission_count
FROM permissions
WHERE scope IS NOT NULL
GROUP BY module, scope
ORDER BY module, scope;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Scoped Permissions System:
--   Format: resource.action.scope
--   Examples: attendance.update.own, grades.read.all
--
-- Scopes:
--   - own: User can only access/modify assigned resources
--   - department: User can access department-level resources
--   - all: User can access all resources (admin level)
--
-- Scope Hierarchy:
--   all > department > own
--   Users with broader scope can access narrower scopes
--
-- No Backward Compatibility:
--   - Only 3-part format supported
--   - No 2-part permissions (resource.action)
--   - No wildcards without explicit scope
-- ===================================================================
