-- ===================================================================
-- Migration 015: Update ADMIN Role with Essential Permissions
-- Description: Update ADMIN role to have only essential permissions
--              for managing staff and roles. Admin can assign other
--              permissions to roles as needed.
-- Date: 2025-12-30
-- ===================================================================

-- Update ADMIN role with essential permissions only
UPDATE roles SET permissions = ARRAY[
  -- Staff Management (4)
  'staff.read.all',
  'staff.create.all',
  'staff.update.all',
  'staff.delete.all',
  
  -- Roles Management (6)
  'roles.read.all',
  'roles.create.all',
  'roles.update.all',
  'roles.delete.all',
  'roles.assign.all',
  'roles.remove.all'
] WHERE code = 'ADMIN';

-- Note: Admin can create other roles and assign additional permissions
-- (menu, settings, features) as needed through the roles management UI.

-- Verify the update
SELECT 
  code, 
  name, 
  array_length(permissions, 1) as permission_count,
  permissions
FROM roles 
WHERE code = 'ADMIN';
