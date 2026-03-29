-- Migration 064: Seed settings.read และ settings.update permissions
-- ใช้กับหน้าตั้งค่าโรงเรียน (school-settings)

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    ('settings.read',   'ดูการตั้งค่า',    'settings', 'read',   'all', 'ดูการตั้งค่าระบบ'),
    ('settings.update', 'แก้ไขการตั้งค่า', 'settings', 'update', 'all', 'แก้ไขการตั้งค่าระบบ เช่น logo โรงเรียน')
ON CONFLICT (code) DO NOTHING;
