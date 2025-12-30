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
-- 2. Permission Data
-- ===================================================================
-- Note: Permission data is now managed by the Rust permission registry 
-- (src/permissions/registry.rs) and synced automatically.
-- The registry contains 18 permissions that will be inserted during
-- the permission sync process.

-- ===================================================================
-- 3. Role Permissions
-- ===================================================================
-- Note: Role permissions are managed through the admin UI.
-- The ADMIN role will receive essential permissions via migration 015.

-- ===================================================================
-- 4. Verification
-- ===================================================================
SELECT 
    COUNT(*) as total_roles
FROM roles;

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
