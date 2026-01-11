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

-- Get the settings group ID for menu items
DO $$
DECLARE
    settings_group_id UUID;
BEGIN
    SELECT id INTO settings_group_id FROM menu_groups WHERE code = 'settings';
    
    -- Insert Admin Dashboard menu item
    INSERT INTO menu_items (
        id, code, name, name_en, path, icon, 
        group_id, required_permission, display_order, is_active
    )
    VALUES (
        gen_random_uuid(),
        'admin_dashboard',
        'ระบบจัดการ',
        'Admin Dashboard',
        '/admin',
        'Shield',
        settings_group_id,
        'settings',  -- Any settings.* permission
        1,
        true
    )
    ON CONFLICT (code) DO NOTHING;
    
    -- Insert Feature Toggles menu item
    INSERT INTO menu_items (
        id, code, name, name_en, path, icon, 
        group_id, required_permission, display_order, is_active
    )
    VALUES (
        gen_random_uuid(),
        'feature_toggles',
        'จัดการระบบงาน',
        'Feature Toggles',
        '/admin/features',
        'Power',
        settings_group_id,
        'settings',  -- Any settings.* permission
        2,
        true
    )
    ON CONFLICT (code) DO NOTHING;
    
    -- Insert Menu Management menu item
    INSERT INTO menu_items (
        id, code, name, name_en, path, icon, 
        group_id, required_permission, display_order, is_active
    )
    VALUES (
        gen_random_uuid(),
        'menu_management',
        'จัดการเมนู',
        'Menu Management',
        '/admin/menu',
        'Menu',
        settings_group_id,
        'settings',  -- Any settings.* permission
        3,
        true
    )
    ON CONFLICT (code) DO NOTHING;
    
END $$;

-- Note: Users need at least one 'settings.*' permission to see these menu items
-- For example: settings.features.read, settings.menu.read, or settings.manage.all
-- 
-- To grant access to a user, assign them a role with settings permissions:
-- 
-- Example: Give user admin role (which should have settings.manage.all)
-- INSERT INTO user_roles (user_id, role_id)
-- SELECT $user_id, id FROM roles WHERE name = 'ผู้ดูแลระบบ';
