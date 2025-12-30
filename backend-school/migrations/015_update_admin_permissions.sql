-- ===================================================================
-- Migration 015: Update ADMIN Role with Wildcard Permission
-- Description: Update ADMIN role to have wildcard (*) permission
--              which grants access to all permissions automatically.
-- Date: 2025-12-30
-- ===================================================================

-- Update ADMIN role with wildcard permission
-- The wildcard '*' grants access to ALL permissions automatically
UPDATE roles SET permissions = ARRAY['*'] WHERE code = 'ADMIN';

-- Note: The permission checking logic (User.has_permission) supports wildcard:
-- if permissions.contains(&"*") { return Ok(true); }

-- Verify the update
SELECT 
  code, 
  name, 
  array_length(permissions, 1) as permission_count,
  permissions
FROM roles 
WHERE code = 'ADMIN';
