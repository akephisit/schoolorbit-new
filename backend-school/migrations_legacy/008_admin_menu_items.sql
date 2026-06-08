-- Add Settings menu group and admin menu items
-- This allows admins to see and access feature toggles and menu management

-- Insert Settings menu group
INSERT INTO menu_groups (id, code, name, name_en, icon, display_order, is_active)
VALUES (
    gen_random_uuid(),
    'settings',
    'การตั้งค่า',
    'Settings',
    'Settings',
    999,  -- Show at the bottom
    true
)
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- Insert Admin Menu Items (REMOVED)
-- ===================================================================
-- Note: Admin menu items are now synced from the frontend application automatically.
-- The DO block for insertion has been removed.

-- Note: Users need at least one 'settings.*' permission to see these menu items
-- For example: settings.features.read, settings.menu.read, or settings.manage.all
-- 
-- To grant access to a user, assign them a role with settings permissions:
-- 
-- Example: Give user admin role (which should have settings.manage.all)
-- INSERT INTO user_roles (user_id, role_id)
-- SELECT $user_id, id FROM roles WHERE name = 'ผู้ดูแลระบบ';
