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

-- Note: Permission data is now managed by the Rust permission registry 
-- (src/permissions/registry.rs) and synced automatically.
-- The registry contains 18 permissions that will be inserted during
-- the permission sync process.

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
