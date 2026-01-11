-- ===================================================================
-- Migration 010: Scoped Permissions System (Schema Setup)
-- Description: Setup permissions and role_permissions tables (Normalized Schema)
-- Date: 2025-12-24
-- Updated: 2026-01-11 - Include table creation for referenced by later migrations
-- ===================================================================

-- ===================================================================
-- 1. Create Normalized Tables
-- ===================================================================

-- Permissions table
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
-- 2. Add Scope Column to Permissions Table (Legacy support logic if tables existed)
-- ===================================================================
-- (Previously this migration only added a column, now it handles full schema)
-- Keeps compatibility if run on existing DB where table might exist differently
-- but for fresh install, above CREATE statements handle it.

-- ===================================================================
-- 3. Verification
-- ===================================================================
SELECT COUNT(*) as table_count 
FROM information_schema.tables 
WHERE table_name IN ('permissions', 'role_permissions');

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Scoped Permissions System:
--   Format: resource.action.scope
--   Examples: attendance.update.own, grades.read.all
-- ===================================================================
