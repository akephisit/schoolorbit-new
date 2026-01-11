-- ===================================================================
-- Migration 020: Clean Legacy Schema (No-op)
-- Description: No legacy columns to clean (Schema is clean from 005)
-- Date: 2026-01-11
-- ===================================================================

-- Roles table created in 005 uses normalized schema from start.
-- No JSON permission column exists to drop.

SELECT 1; -- No operation
