-- Add common menu groups for the application
-- These groups are used by auto-registered menu items

INSERT INTO menu_groups (id, code, name, name_en, icon, display_order, is_active)
VALUES
    -- Main navigation
    (gen_random_uuid(), 'main', 'หลัก', 'Main', 'Home', 1, true),
    
    -- HR/Staff management
    (gen_random_uuid(), 'hr', 'บุคลากร', 'Human Resources', 'Users', 10, true),
    
    -- Academic/Students
    (gen_random_uuid(), 'academic', 'การเรียนการสอน', 'Academic', 'GraduationCap', 20, true),
    
    -- Finance
    (gen_random_uuid(), 'finance', 'การเงิน', 'Finance', 'DollarSign', 30, true),
    
    -- Reports
    (gen_random_uuid(), 'reports', 'รายงาน', 'Reports', 'FileText', 40, true),
    
    -- System settings (already exists, but add if missing)
    (gen_random_uuid(), 'system', 'ระบบ', 'System', 'Settings', 900, true)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    icon = EXCLUDED.icon,
    display_order = EXCLUDED.display_order;

-- Note: 'settings' group already exists from 012_admin_menu_items.sql
