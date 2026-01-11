-- ===================================================================
-- Migration 020: Clean Legacy Schema
-- Description: Drop legacy JSON permission column (Tables created in 010)
-- Date: 2026-01-11
-- ===================================================================

-- ===================================================================
-- 1. Drop Legacy Permission Column
-- ===================================================================
ALTER TABLE roles DROP COLUMN IF EXISTS permission;
ALTER TABLE roles DROP COLUMN IF EXISTS permissions; -- Handle both names if existed

-- ===================================================================
-- 2. Verification
-- ===================================================================
SELECT column_name 
FROM information_schema.columns 
WHERE table_name = 'roles' AND column_name IN ('permission', 'permissions');

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Tables (permissions, role_permissions) are now created in 010
-- Data is inserted in 013, 014, etc.
-- This migration just cleans up the old schema.
-- ===================================================================
