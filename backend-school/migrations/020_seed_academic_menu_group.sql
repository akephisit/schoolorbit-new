-- Add Academic Menu Group
INSERT INTO menu_groups (code, name, icon, display_order)
VALUES ('academic', 'งานวิชาการ', 'School', 30)
ON CONFLICT (code) DO NOTHING;
