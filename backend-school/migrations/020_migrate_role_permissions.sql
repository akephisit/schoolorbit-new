-- ===================================================================
-- Migration 020: Clean Normalized Permissions Schema
-- Description: Remove JSON permission column, create normalized tables
-- Date: 2026-01-11
-- Note: Clean migration - assumes fresh database
-- ===================================================================

-- ===================================================================
-- 1. Drop Legacy Permission Column
-- ===================================================================
ALTER TABLE roles DROP COLUMN IF EXISTS permission;

-- ===================================================================
-- 2. Create Permission Tables
-- ===================================================================

-- Permissions table
CREATE TABLE permissions (
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

CREATE INDEX idx_permissions_code ON permissions(code);
CREATE INDEX idx_permissions_module ON permissions(module);
CREATE INDEX idx_permissions_scope ON permissions(scope);

-- Role Permissions junction table
CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);

CREATE INDEX idx_role_permissions_role ON role_permissions(role_id);
CREATE INDEX idx_role_permissions_permission ON role_permissions(permission_id);

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Permissions will be auto-synced from Rust registry (src/permissions/registry.rs)
-- Role permissions must be assigned through admin UI after sync
-- ===================================================================
