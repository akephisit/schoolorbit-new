-- Migration 055: กลุ่มบริหาร (academic/administrative) heads get academic_curriculum.read.all
-- position = NULL means all members of these depts get this permission

INSERT INTO department_permissions (department_id, permission_id, position)
SELECT d.id, p.id, NULL
FROM departments d
CROSS JOIN permissions p
WHERE d.category IN ('academic', 'administrative')
  AND d.code NOT LIKE 'SUBJ-%'        -- exclude subject groups (they already have manage.department)
  AND p.code = 'academic_curriculum.read.all'
ON CONFLICT (department_id, permission_id) DO NOTHING;
