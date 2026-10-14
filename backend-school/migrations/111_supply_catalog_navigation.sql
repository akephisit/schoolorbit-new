-- Seed the procurement section; school-owned labels and placement survive synchronization.
INSERT INTO menu_groups (id, code, name, name_en, icon, display_order, is_active, workspace_code)
VALUES (uuidv7(), 'budget_procurement', 'งานพัสดุ', 'Procurement', 'package', 10, true, 'budget')
ON CONFLICT (code) DO NOTHING;
