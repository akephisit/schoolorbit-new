-- ===================================================================
-- Migration 020: Clean Normalized Permissions Schema
-- Description: Remove legacy JSON permission column, use only normalized tables
-- Date: 2026-01-11
-- ===================================================================

-- ===================================================================
-- 1. Drop Legacy Permission Column
-- ===================================================================
ALTER TABLE roles DROP COLUMN IF EXISTS permission;

-- ===================================================================
-- 2. Ensure Permission Tables Exist
-- ===================================================================

-- Permissions table (if not exists from previous migrations)
CREATE TABLE IF NOT EXISTS permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    module VARCHAR(50) NOT NULL,
    action VARCHAR(50) NOT NULL,
    scope VARCHAR(50) NOT NULL DEFAULT 'all',
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_permissions_code ON permissions(code);
CREATE INDEX IF NOT EXISTS idx_permissions_module ON permissions(module);
CREATE INDEX IF NOT EXISTS idx_permissions_scope ON permissions(scope);

-- Role Permissions junction table
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);

CREATE INDEX IF NOT EXISTS idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX IF NOT EXISTS idx_role_permissions_permission ON role_permissions(permission_id);

-- ===================================================================
-- 3. Verification
-- ===================================================================
SELECT 
    COUNT(*) as total_roles,
    COUNT(*) FILTER (WHERE id IN (SELECT DISTINCT role_id FROM role_permissions)) as roles_with_permissions
FROM roles;

SELECT COUNT(*) as total_permissions FROM permissions;
SELECT COUNT(*) as total_role_permissions FROM role_permissions;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- This is a CLEAN migration - no backward compatibility with JSON column
-- Permissions will be synced from Rust permission registry (src/permissions/registry.rs)
-- Role permissions must be assigned through the admin UI or permission sync
-- ===================================================================
