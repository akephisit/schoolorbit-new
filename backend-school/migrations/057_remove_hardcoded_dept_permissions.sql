-- Migration 057: ลบ permissions ที่ migration 055 insert ไว้แบบ hardcode
-- ให้ admin กำหนด department_permissions เองผ่าน UI แทน

DELETE FROM department_permissions dp
USING departments d, permissions p
WHERE dp.department_id = d.id
  AND dp.permission_id = p.id
  AND d.category IN ('academic', 'administrative')
  AND d.code NOT LIKE 'SUBJ-%'
  AND p.code = 'academic_curriculum.read.all';
