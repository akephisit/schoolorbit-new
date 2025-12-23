-- ===================================================================
-- Migration 013: Enforce Scoped Permissions (Remove Backward Compatibility)
-- Description: Convert all 2-part permissions to 3-part scoped format
-- Date: 2025-12-24
-- ===================================================================

-- ===================================================================
-- 1. Update Existing 2-Part Permissions to Include Scope
-- ===================================================================

-- Dashboard permissions (read-only, scope = all)
UPDATE permissions SET code = 'dashboard.read.all', scope = 'all' WHERE code = 'dashboard.read';

-- Subjects permissions (split into .own and .all)
DELETE FROM permissions WHERE code IN ('subjects.read', 'subjects.create', 'subjects.update', 'subjects.delete');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('subjects.read.all', 'View All Subjects', 'subjects', 'read', 'all', 'View all subjects'),
  ('subjects.create.all', 'Create Subjects', 'subjects', 'create', 'all', 'Create new subjects'),
  ('subjects.update.all', 'Update Subjects', 'subjects', 'update', 'all', 'Edit subjects'),
  ('subjects.delete.all', 'Delete Subjects', 'subjects', 'delete', 'all', 'Delete subjects')
ON CONFLICT (code) DO NOTHING;

-- Classes permissions (split into .own and .all)
DELETE FROM permissions WHERE code IN ('classes.read', 'classes.create', 'classes.update', 'classes.delete');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('classes.read.own', 'View Own Classes', 'classes', 'read', 'own', 'View assigned classes'),
  ('classes.read.all', 'View All Classes', 'classes', 'read', 'all', 'View all classes'),
  ('classes.create.all', 'Create Classes', 'classes', 'create', 'all', 'Create new classes'),
  ('classes.update.all', 'Update Classes', 'classes', 'update', 'all', 'Edit classes'),
  ('classes.delete.all', 'Delete Classes', 'classes', 'delete', 'all', 'Delete classes')
ON CONFLICT (code) DO NOTHING;

-- Calendar permissions (scope = all)
DELETE FROM permissions WHERE code IN ('calendar.read', 'calendar.create', 'calendar.update', 'calendar.delete');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('calendar.read.all', 'View Calendar', 'calendar', 'read', 'all', 'Access calendar and events'),
  ('calendar.create.all', 'Create Events', 'calendar', 'create', 'all', 'Create calendar events'),
  ('calendar.update.all', 'Update Events', 'calendar', 'update', 'all', 'Update calendar events'),
  ('calendar.delete.all', 'Delete Events', 'calendar', 'delete', 'all', 'Delete calendar events')
ON CONFLICT (code) DO NOTHING;

-- Settings permissions (scope = all)
DELETE FROM permissions WHERE code IN ('settings.read', 'settings.update');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('settings.read.all', 'View Settings', 'settings', 'read', 'all', 'Access system settings'),
  ('settings.update.all', 'Update Settings', 'settings', 'update', 'all', 'Modify system settings')
ON CONFLICT (code) DO NOTHING;

-- Roles permissions (scope = all)
DELETE FROM permissions WHERE code IN ('roles.read', 'roles.create', 'roles.update', 'roles.delete');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('roles.read.all', 'View Roles', 'roles', 'read', 'all', 'View roles and permissions'),
  ('roles.create.all', 'Create Roles', 'roles', 'create', 'all', 'Create new roles'),
  ('roles.update.all', 'Update Roles', 'roles', 'update', 'all', 'Edit roles'),
  ('roles.delete.all', 'Delete Roles', 'roles', 'delete', 'all', 'Delete roles')
ON CONFLICT (code) DO NOTHING;

-- Staff permissions (already have some from migration 005, add scoped versions)
DELETE FROM permissions WHERE code IN ('staff.read', 'staff.create', 'staff.update', 'staff.delete');
INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  ('staff.read.own', 'View Own Profile', 'staff', 'read', 'own', 'View own staff profile'),
  ('staff.read.all', 'View All Staff', 'staff', 'read', 'all', 'View all staff profiles'),
  ('staff.create.all', 'Create Staff', 'staff', 'create', 'all', 'Create new staff'),
  ('staff.update.own', 'Update Own Profile', 'staff', 'update', 'own', 'Update own staff profile'),
  ('staff.update.all', 'Update All Staff', 'staff', 'update', 'all', 'Update any staff profile'),
  ('staff.delete.all', 'Delete Staff', 'staff', 'delete', 'all', 'Delete staff')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 2. Update All Roles to Use Scoped Permissions
-- ===================================================================

-- TEACHER: Own scope for most things
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
] WHERE code = 'TEACHER';

-- DEPT_HEAD: Department scope
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'subjects.read.all',
  'classes.read.all',
  'calendar.read.all',
  'grades.read.department',
  'grades.update.department',
  'students.read.own',
  'staff.read.all'
] WHERE code = 'DEPT_HEAD';

-- VICE_DIRECTOR: All scope for academic
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
] WHERE code = 'VICE_DIRECTOR';

-- DIRECTOR: Full access
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
  'students.update.all',
  'staff.read.all',
  'staff.create.all',
  'staff.update.all',
  'staff.delete.all',
  'roles.read.all'
] WHERE code = 'DIRECTOR';

-- SECRETARY: Read-only mostly
UPDATE roles SET permissions = ARRAY[
  'dashboard.read.all',
  'calendar.read.all',
  'calendar.create.all',
  'settings.read.all',
  'students.read.all',
  'staff.read.all'
] WHERE code = 'SECRETARY';

-- ADMIN: Roles management
UPDATE roles SET permissions = ARRAY[
  'roles.read.all',
  'roles.create.all',
  'roles.update.all',
  'roles.delete.all',
  'staff.read.all',
  'staff.create.all',
  'staff.update.all'
] WHERE code = 'ADMIN';

-- ===================================================================
-- 3. Remove All 2-Part Permissions (Cleanup)
-- ===================================================================

-- Delete any remaining 2-part permissions
DELETE FROM permissions 
WHERE scope IS NULL 
   OR (code NOT LIKE '%.%.%' AND code LIKE '%.%');

-- Ensure all permissions have a scope
UPDATE permissions 
SET scope = 'all' 
WHERE scope IS NULL;

-- ===================================================================
-- 4. Verification
-- ===================================================================
SELECT 
    module,
    scope,
    COUNT(*) as count
FROM permissions
GROUP BY module, scope
ORDER BY module, scope;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Breaking Change: This migration removes backward compatibility
-- - All permissions now use 3-part format (resource.action.scope)
-- - No more 2-part format support
-- - All roles updated to use scoped permissions
--
-- Scope Usage:
-- - .own: For assigned resources (teachers with their classes)
-- - .department: For department-level access (dept heads)
-- - .all: For full access (directors, admins)
--
-- After this migration:
-- - Backend parser should only accept 3-part format
-- - Frontend should use hasScopedPermission()
-- - No wildcards without explicit scope
-- ===================================================================
