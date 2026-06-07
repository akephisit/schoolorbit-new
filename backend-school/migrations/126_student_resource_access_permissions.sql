-- Migration 126: Student resource-aware access permissions.
-- Splits school-wide student read from PII and adds assigned-scope advisor access.

CREATE TEMP TABLE tmp_student_read_all_roles AS
SELECT rp.role_id, rp.created_at
FROM role_permissions rp
JOIN permissions p ON p.id = rp.permission_id
WHERE p.code = 'student.read.all';

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'student.read.own',
        'ดูข้อมูลนักเรียนของตนเอง',
        'student',
        'read',
        'own',
        'นักเรียนดูข้อมูลทั่วไปของตนเอง'
    ),
    (
        'student.read.assigned',
        'ดูนักเรียนที่รับผิดชอบ',
        'student',
        'read',
        'assigned',
        'ดูข้อมูลทั่วไปของนักเรียนที่อยู่ในห้อง/งานที่รับผิดชอบ'
    ),
    (
        'student.read.school',
        'ดูนักเรียนทั้งโรงเรียน',
        'student',
        'read',
        'school',
        'ดูข้อมูลทั่วไปของนักเรียนทั้งโรงเรียน'
    ),
    (
        'student.update.own',
        'แก้ไขข้อมูลนักเรียนของตนเอง',
        'student',
        'update',
        'own',
        'นักเรียนแก้ไขข้อมูลติดต่อพื้นฐานของตนเอง'
    ),
    (
        'student.create',
        'เพิ่มนักเรียน',
        'student',
        'create',
        'school',
        'สร้างข้อมูลนักเรียน'
    ),
    (
        'student.update.all',
        'แก้ไขนักเรียน',
        'student',
        'update',
        'school',
        'แก้ไขข้อมูลนักเรียนทั้งโรงเรียน'
    ),
    (
        'student.delete',
        'ลบนักเรียน',
        'student',
        'delete',
        'school',
        'ลบนักเรียน'
    ),
    (
        'student_pii.read.own',
        'ดูข้อมูลอ่อนไหวนักเรียนของตนเอง',
        'student_pii',
        'read',
        'own',
        'ดูข้อมูลอ่อนไหวของนักเรียนเฉพาะของตนเอง'
    ),
    (
        'student_pii.read.assigned',
        'ดูข้อมูลอ่อนไหวนักเรียนที่รับผิดชอบ',
        'student_pii',
        'read',
        'assigned',
        'ดูข้อมูลอ่อนไหวของนักเรียนที่อยู่ในห้อง/งานที่รับผิดชอบ'
    ),
    (
        'student_pii.read.school',
        'ดูข้อมูลอ่อนไหวนักเรียนทั้งโรงเรียน',
        'student_pii',
        'read',
        'school',
        'ดูข้อมูลอ่อนไหวของนักเรียนทั้งโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

WITH old_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.all'
),
new_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.school'
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT rp.role_id, new_permission.id, rp.created_at
FROM role_permissions rp
JOIN old_permission ON old_permission.id = rp.permission_id
CROSS JOIN new_permission
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH old_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.all'
),
new_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.school'
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at,
    created_by
)
SELECT opg.organization_unit_id,
       new_permission.id,
       opg.position_code,
       opg.created_at,
       opg.created_by
FROM organization_permission_grants opg
JOIN old_permission ON old_permission.id = opg.permission_id
CROSS JOIN new_permission
WHERE NOT EXISTS (
    SELECT 1
    FROM organization_permission_grants existing
    WHERE existing.organization_unit_id = opg.organization_unit_id
      AND existing.permission_id = new_permission.id
      AND existing.position_code IS NOT DISTINCT FROM opg.position_code
);

WITH old_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.all'
),
new_permission AS (
    SELECT id FROM permissions WHERE code = 'student.read.school'
)
INSERT INTO organization_permission_delegations (
    from_user_id,
    to_user_id,
    permission_id,
    organization_unit_id,
    reason,
    started_at,
    expires_at,
    revoked_at,
    created_at
)
SELECT opd.from_user_id,
       opd.to_user_id,
       new_permission.id,
       opd.organization_unit_id,
       opd.reason,
       opd.started_at,
       opd.expires_at,
       opd.revoked_at,
       opd.created_at
FROM organization_permission_delegations opd
JOIN old_permission ON old_permission.id = opd.permission_id
CROSS JOIN new_permission
WHERE NOT EXISTS (
    SELECT 1
    FROM organization_permission_delegations existing
    WHERE existing.from_user_id = opd.from_user_id
      AND existing.to_user_id = opd.to_user_id
      AND existing.permission_id = new_permission.id
      AND existing.organization_unit_id IS NOT DISTINCT FROM opd.organization_unit_id
      AND existing.started_at = opd.started_at
);

WITH pii_permission AS (
    SELECT id FROM permissions WHERE code = 'student_pii.read.school'
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT role_id, pii_permission.id, created_at
FROM tmp_student_read_all_roles
CROSS JOIN pii_permission
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code IN (
    'student.read.own',
    'student.update.own',
    'student_pii.read.own'
)
WHERE r.code = 'STUDENT'
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code IN (
    'student.read.assigned',
    'student_pii.read.assigned'
)
WHERE r.code = 'TEACHER'
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON p.code IN (
    'student.read.school',
    'student.create',
    'student.update.all',
    'student_pii.read.school'
)
WHERE r.code = 'STUDENT_MANAGER'
ON CONFLICT (role_id, permission_id) DO NOTHING;

DELETE FROM permissions WHERE code = 'student.read.all';

DROP TABLE tmp_student_read_all_roles;
