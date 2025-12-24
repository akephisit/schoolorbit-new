-- Add settings module permissions for managing features and menus
-- These permissions allow users to manage system features and menu structure

INSERT INTO permissions (code, name, module, action, scope, description) VALUES
  -- Feature Toggle permissions
  ('settings.features.read', 'ดูการตั้งค่าระบบงาน', 'settings', 'features', 'read', 'ดูสถานะเปิด/ปิดระบบงานต่างๆ'),
  ('settings.features.update', 'จัดการระบบงาน', 'settings', 'features', 'update', 'เปิด/ปิดระบบงานต่างๆ ได้'),
  
  -- Menu management permissions
  ('settings.menu.read', 'ดูโครงสร้างเมนู', 'settings', 'menu', 'read', 'ดูโครงสร้างเมนูของระบบ'),
  ('settings.menu.create', 'สร้างเมนู', 'settings', 'menu', 'create', 'สร้างเมนูใหม่'),
  ('settings.menu.update', 'แก้ไขเมนู', 'settings', 'menu', 'update', 'แก้ไขเมนูที่มีอยู่'),
  ('settings.menu.delete', 'ลบเมนู', 'settings', 'menu', 'delete', 'ลบเมนู'),
  
  -- Admin wildcard permission (full access)
  ('settings.manage.all', 'จัดการระบบทั้งหมด', 'settings', 'manage', 'all', 'จัดการการตั้งค่าและเมนูทั้งหมด')
ON CONFLICT (code) DO NOTHING;

-- Note: Module-based permission system allows users with ANY permission in a module
-- to manage that module's features and menus. For example:
-- 
-- - User with 'attendance.update.all' can toggle attendance-related features
-- - User with 'staff.read.own' can manage staff-related menus
-- - User with 'settings.manage.all' or '*.*.all' has full access to everything
