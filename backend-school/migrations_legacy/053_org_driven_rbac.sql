-- Migration 053: Org-driven RBAC
-- 1. Link departments to subject_groups
-- 2. Make department_permissions position-aware
-- 3. Add academic_curriculum.manage.department permission

-- ── 1. subject_group_id on departments ──────────────────────────────────────
ALTER TABLE departments
    ADD COLUMN IF NOT EXISTS subject_group_id UUID REFERENCES subject_groups(id);

UPDATE departments d
SET subject_group_id = sg.id
FROM subject_groups sg
WHERE d.code = 'SUBJ-' || sg.code;

-- ── 2. position filter on department_permissions ─────────────────────────────
-- NULL  = permission applies to ALL positions in the department
-- value = permission applies ONLY to that specific position (head, deputy_head, member, coordinator)
ALTER TABLE department_permissions
    ADD COLUMN IF NOT EXISTS position TEXT DEFAULT NULL;

-- ── 3. New permission: academic_curriculum.manage.department ─────────────────
INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
    'academic_curriculum.manage.department',
    'จัดการรายวิชากลุ่มสาระตัวเอง',
    'academic_curriculum',
    'manage',
    'department',
    'เพิ่ม/แก้ไข/ลบ รายวิชาเฉพาะกลุ่มสาระการเรียนรู้ที่ตัวเองสังกัด'
)
ON CONFLICT (code) DO NOTHING;

-- ── 4. Assign manage.department to all SUBJ-* departments (all positions) ────
INSERT INTO department_permissions (department_id, permission_id, position)
SELECT d.id, p.id, NULL
FROM departments d
CROSS JOIN permissions p
WHERE d.code LIKE 'SUBJ-%'
  AND p.code = 'academic_curriculum.manage.department'
ON CONFLICT (department_id, permission_id) DO NOTHING;

-- ── 5. Assign dept_work.approve.department only to heads of SUBJ-* and admin groups ──
INSERT INTO department_permissions (department_id, permission_id, position)
SELECT d.id, p.id, 'head'
FROM departments d
CROSS JOIN permissions p
WHERE d.category IN ('academic', 'administrative')
  AND p.code = 'dept_work.approve.department'
ON CONFLICT (department_id, permission_id) DO NOTHING;
