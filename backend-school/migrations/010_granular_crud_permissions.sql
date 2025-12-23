-- ===================================================================
-- Migration 010: Update to Granular CRUD Permission System
-- Description: Convert all permissions to CRUD format (create, read, update, delete)
-- Date: 2025-12-23
-- ===================================================================

-- ===================================================================
-- 1. Update Permissions Table - Change existing permissions to CRUD format
-- ===================================================================

-- Update existing permissions to use CRUD actions
UPDATE permissions SET 
    code = 'staff.read',
    name = 'ดูข้อมูลบุคลากร',
    action = 'read'
WHERE code = 'users.view';

UPDATE permissions SET 
    code = 'staff.create',
    name = 'สร้างบุคลากร',
    action = 'create'
WHERE code = 'users.create';

UPDATE permissions SET 
    code = 'staff.update',
    name = 'แก้ไขบุคลากร',
    action = 'update'
WHERE code = 'users.edit';

UPDATE permissions SET 
    code = 'staff.delete',
    name = 'ลบบุคลากร',
    action = 'delete'
WHERE code = 'users.delete';

-- Update students permissions
UPDATE permissions SET 
    code = 'students.read',
    action = 'read'
WHERE code = 'students.view';

UPDATE permissions SET 
    code = 'students.update',
    action = 'update'
WHERE code = 'students.edit';

-- Update grades permissions
UPDATE permissions SET 
    code = 'grades.read',
    action = 'read'
WHERE code = 'grades.view';

UPDATE permissions SET 
    code = 'grades.update',
    action = 'update'
WHERE code = 'grades.edit';

-- Update attendance permissions
UPDATE permissions SET 
    code = 'attendance.read',
    action = 'read'
WHERE code = 'attendance.view';

UPDATE permissions SET 
    code = 'attendance.create',
    action = 'create'
WHERE code = 'attendance.mark';

-- Update documents permissions
UPDATE permissions SET 
    code = 'documents.read',
    action = 'read'
WHERE code = 'documents.view';

-- Update finance permissions
UPDATE permissions SET 
    code = 'finance.read',
    action = 'read'
WHERE code = 'finance.view';

-- Update dashboard permission
UPDATE permissions SET 
    code = 'dashboard.read',
    action = 'read'
WHERE code = 'dashboard.view';

-- Update subjects permission
UPDATE permissions SET 
    code = 'subjects.read',
    action = 'read'
WHERE code = 'subjects.view';

-- Update classes permission
UPDATE permissions SET 
    code = 'classes.read',
    action = 'read'
WHERE code = 'classes.view';

-- Update calendar permission
UPDATE permissions SET 
    code = 'calendar.read',
    action = 'read'
WHERE code = 'calendar.view';

-- Update settings permission
UPDATE permissions SET 
    code = 'settings.read',
    action = 'read'
WHERE code = 'settings.view';

-- Update roles permission (for admin)
INSERT INTO permissions (code, name, module, action, description) VALUES
    ('roles.read', 'ดูข้อมูลบทบาท', 'roles', 'read', 'สามารถดูข้อมูลบทบาท'),
    ('roles.create', 'สร้างบทบาท', 'roles', 'create', 'สามารถสร้างบทบาทใหม่'),
    ('roles.update', 'แก้ไขบทบาท', 'roles', 'update', 'สามารถแก้ไขข้อมูลบทบาท'),
    ('roles.delete', 'ลบบทบาท', 'roles', 'delete', 'สามารถลบบทบาท')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 2. Create helper function to update role permissions
-- ===================================================================

CREATE OR REPLACE FUNCTION replace_permission_in_roles(
    old_permission TEXT,
    new_permission TEXT
) RETURNS void AS $$
BEGIN
    UPDATE roles
    SET permissions = array_replace(permissions, old_permission, new_permission)
    WHERE old_permission = ANY(permissions);
END;
$$ LANGUAGE plpgsql;

-- ===================================================================
-- 3. Update all role permissions to use new CRUD format
-- ===================================================================

-- Replace old permissions with new ones in all roles
SELECT replace_permission_in_roles('users.view', 'staff.read');
SELECT replace_permission_in_roles('users.create', 'staff.create');
SELECT replace_permission_in_roles('users.edit', 'staff.update');
SELECT replace_permission_in_roles('users.delete', 'staff.delete');

SELECT replace_permission_in_roles('students.view', 'students.read');
SELECT replace_permission_in_roles('students.edit', 'students.update');

SELECT replace_permission_in_roles('grades.view', 'grades.read');
SELECT replace_permission_in_roles('grades.edit', 'grades.update');

SELECT replace_permission_in_roles('attendance.view', 'attendance.read');
SELECT replace_permission_in_roles('attendance.mark', 'attendance.create');

SELECT replace_permission_in_roles('documents.view', 'documents.read');

SELECT replace_permission_in_roles('finance.view', 'finance.read');

SELECT replace_permission_in_roles('dashboard.view', 'dashboard.read');
SELECT replace_permission_in_roles('subjects.view', 'subjects.read');
SELECT replace_permission_in_roles('classes.view', 'classes.read');
SELECT replace_permission_in_roles('calendar.view', 'calendar.read');
SELECT replace_permission_in_roles('settings.view', 'settings.read');

-- ===================================================================
-- 4. Add roles.read permission to roles that need to view roles
-- ===================================================================

-- Add roles.read to admin roles
UPDATE roles
SET permissions = array_append(permissions, 'roles.read')
WHERE code IN ('ADMIN', 'DIRECTOR', 'VICE_DIRECTOR')
  AND NOT ('roles.read' = ANY(permissions));

-- Add roles CRUD permissions to ADMIN role
UPDATE roles
SET permissions = permissions || ARRAY['roles.create', 'roles.update', 'roles.delete']::TEXT[]
WHERE code = 'ADMIN'
  AND NOT ('roles.create' = ANY(permissions));

-- ===================================================================
-- 5. Verify the changes
-- ===================================================================

SELECT 
    r.code,
    r.name,
    array_length(r.permissions, 1) as permission_count,
    r.permissions
FROM roles r
WHERE r.is_active = true
ORDER BY r.level DESC;

-- Show all permissions in new format
SELECT 
    module,
    action,
    array_agg(code ORDER BY code) as permissions
FROM permissions
GROUP BY module, action
ORDER BY module, action;

-- ===================================================================
-- 6. Cleanup - Drop helper function
-- ===================================================================

DROP FUNCTION IF EXISTS replace_permission_in_roles(TEXT, TEXT);

-- ===================================================================
-- NOTES:
-- ===================================================================
-- New Permission Format:
--   - {resource}.read    - View/List resources
--   - {resource}.create  - Create new resources
--   - {resource}.update  - Edit existing resources
--   - {resource}.delete  - Delete resources
--
-- Wildcard Support:
--   - {resource} (without action) - Grants all CRUD operations
--   - * - Admin wildcard (all permissions)
--
-- Examples:
--   - staff.read - Can view staff list and profiles
--   - staff.create - Can create new staff
--   - staff.update - Can edit staff information
--   - staff.delete - Can delete staff
--   - staff - Can do all staff operations (wildcard)
--
-- Migration Changes:
--   - users.* → staff.*
--   - *.view → *.read
--   - *.edit → *.update
--   - attendance.mark → attendance.create
-- ===================================================================
