-- Settings module permissions
-- These are now managed by the Rust permission registry (src/permissions/registry.rs)
-- and will be synced automatically.

-- Note: Module-based permission system allows users with ANY permission in a module
-- to manage that module's features and menus. For example:
-- 
-- - User with 'attendance.update.all' can toggle attendance-related features
-- - User with 'staff.read.own' can manage staff-related menus
-- - User with 'settings.manage.all' or '*.*.all' has full access to everything
