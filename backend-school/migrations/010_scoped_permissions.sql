-- ===================================================================
-- Migration 010: Scoped Permissions System (No-op)
-- Description: Schema Moved to Migration 005
-- Date: 2026-01-11
-- ===================================================================

-- Tables (permissions, role_permissions) are now created in 005_create_staff_management.sql
-- This allows later migrations (013, 014) to confidently insert data without dependency loops.
-- Use this file only for future alterations if needed, or leave empty.

SELECT 1; -- No operation
