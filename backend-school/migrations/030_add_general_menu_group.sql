-- Create 'General Affairs' menu group
INSERT INTO menu_groups (id, code, name, icon, display_order, is_active)
VALUES (
    uuid_generate_v4(),
    'general',
    'งานทั่วไป',
    'Briefcase',
    40,
    true
)
ON CONFLICT (code) DO NOTHING;
