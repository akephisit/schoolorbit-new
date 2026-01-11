-- ===================================================================
-- Migration 020: Migrate Role Permissions to Normalized Schema
-- Description: Convert roles.permission JSON array to role_permissions table
-- Date: 2026-01-11
-- ===================================================================

-- ===================================================================
-- 1. Insert permissions into role_permissions from roles.permission
-- ===================================================================

-- For each role that has permissions in the permission column,
-- insert them into role_permissions table
INSERT INTO role_permissions (role_id, permission_id)
SELECT 
    r.id as role_id,
    p.id as permission_id
FROM roles r
CROSS JOIN LATERAL jsonb_array_elements_text(r.permission::jsonb) AS perm_code
JOIN permissions p ON p.code = perm_code
WHERE r.permission IS NOT NULL 
  AND r.permission != 'null'
  AND r.permission != '[]'
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- ===================================================================
-- 2. Verification
-- ===================================================================
SELECT 
    r.name as role_name,
    COUNT(rp.permission_id) as permission_count,
    array_length(r.permission::jsonb::text[]::text[], 1) as old_permission_count
FROM roles r
LEFT JOIN role_permissions rp ON r.id = rp.role_id
WHERE r.permission IS NOT NULL
GROUP BY r.id, r.name, r.permission;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- This migration preserves the old permission column for backward compatibility
-- Future migrations can drop the permission column after confirming all data is migrated
-- ===================================================================
