-- ===================================================================
-- Migration 015: Clean Up Permissions - Registry as Single Source of Truth
-- Description: Remove all old permissions from migrations, leaving only
--              the 18 permissions defined in the Rust permission registry
-- Date: 2025-12-30
-- ===================================================================

-- Delete all existing permissions
-- They will be re-created by the Rust permission sync system
DELETE FROM permissions;

-- Note: The permission registry (permissions/registry.rs) contains 18 permissions:
-- 
-- Staff (4):
--   - staff.read.all
--   - staff.create.all
--   - staff.update.all
--   - staff.delete.all
--
-- Roles (6):
--   - roles.read.all
--   - roles.create.all
--   - roles.update.all
--   - roles.delete.all
--   - roles.assign.all
--   - roles.remove.all
--
-- Menu (4):
--   - menu.read.all
--   - menu.create.all
--   - menu.update.all
--   - menu.delete.all
--
-- Settings (2):
--   - settings.read
--   - settings.update
--
-- Features (2):
--   - features.read.all
--   - features.update.all
--
-- These will be automatically synced to the database by the 
-- permission sync system when the backend starts or when a 
-- new school is provisioned.

-- Update role permissions to use only the new 18 permissions
-- (Removing references to old permissions that no longer exist)

UPDATE roles SET permissions = ARRAY[
  'staff.read.all',
  'staff.create.all',
  'staff.update.all',
  'staff.delete.all',
  'roles.read.all',
  'roles.create.all',
  'roles.update.all',
  'roles.delete.all',
  'roles.assign.all',
  'roles.remove.all',
  'menu.read.all',
  'menu.create.all',
  'menu.update.all',
  'menu.delete.all',
  'settings.read',
  'settings.update',
  'features.read.all',
  'features.update.all'
] WHERE code = 'ADMIN' AND is_active = true;

-- Note: Other roles will be updated as needed
-- For now, ADMIN role gets all permissions
