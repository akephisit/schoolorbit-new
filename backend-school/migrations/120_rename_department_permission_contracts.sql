-- Migration 120: Rename department-era permission contracts to organization units.
-- Keep permission ids where possible so existing role/grant references remain intact.

WITH permission_renames (
    old_code,
    new_code,
    new_name,
    new_module,
    new_action,
    new_scope,
    new_description
) AS (
    VALUES
        (
            'academic_curriculum.manage.department',
            'academic_curriculum.manage.organization_unit',
            'จัดการรายวิชากลุ่มสาระตัวเอง',
            'academic_curriculum',
            'manage',
            'organization_unit',
            'เพิ่ม/แก้ไข/ลบ รายวิชาเฉพาะกลุ่มสาระการเรียนรู้ที่ตัวเองสังกัด'
        ),
        (
            'dept_work.read.own',
            'organization_work.read.own',
            'ดูงานของตนเอง',
            'organization_work',
            'read',
            'own',
            'ดูรายการงานที่ตนเองรับผิดชอบ'
        ),
        (
            'dept_work.read.department',
            'organization_work.read.organization_unit',
            'ดูงานในหน่วยงาน',
            'organization_work',
            'read',
            'organization_unit',
            'ดูรายการงานทั้งหมดในหน่วยงานที่สังกัด'
        ),
        (
            'dept_work.create',
            'organization_work.create',
            'สร้าง/ส่งงาน',
            'organization_work',
            'create',
            'own',
            'ส่งงานหรือสร้างบันทึกงานใหม่'
        ),
        (
            'dept_work.update.own',
            'organization_work.update.own',
            'แก้ไขงานตนเอง',
            'organization_work',
            'update',
            'own',
            'แก้ไขรายละเอียดงานของตนเอง'
        ),
        (
            'dept_work.approve.department',
            'organization_work.approve.organization_unit',
            'อนุมัติงานในหน่วยงาน',
            'organization_work',
            'approve',
            'organization_unit',
            'อนุมัติหรือตรวจสอบงานของสมาชิกในหน่วยงาน'
        )
)
UPDATE permissions p
SET code = permission_renames.new_code,
    name = permission_renames.new_name,
    module = permission_renames.new_module,
    action = permission_renames.new_action,
    scope = permission_renames.new_scope,
    description = permission_renames.new_description,
    updated_at = NOW()
FROM permission_renames
WHERE p.code = permission_renames.old_code
  AND NOT EXISTS (
      SELECT 1
      FROM permissions existing
      WHERE existing.code = permission_renames.new_code
  );

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT role_permissions.role_id, duplicate_pairs.new_id, role_permissions.created_at
FROM role_permissions
JOIN duplicate_pairs ON duplicate_pairs.old_id = role_permissions.permission_id
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
UPDATE organization_permission_delegations delegation
SET permission_id = duplicate_pairs.new_id
FROM duplicate_pairs
WHERE delegation.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
UPDATE organization_permission_grants grant_row
SET permission_id = duplicate_pairs.new_id
FROM duplicate_pairs
WHERE grant_row.permission_id = duplicate_pairs.old_id
  AND NOT EXISTS (
      SELECT 1
      FROM organization_permission_grants existing
      WHERE existing.organization_unit_id = grant_row.organization_unit_id
        AND existing.permission_id = duplicate_pairs.new_id
        AND existing.position_code IS NOT DISTINCT FROM grant_row.position_code
  );

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM organization_permission_grants grant_row
USING duplicate_pairs
WHERE grant_row.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM role_permissions role_permission
USING duplicate_pairs
WHERE role_permission.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('academic_curriculum.manage.department', 'academic_curriculum.manage.organization_unit'),
        ('dept_work.read.own', 'organization_work.read.own'),
        ('dept_work.read.department', 'organization_work.read.organization_unit'),
        ('dept_work.create', 'organization_work.create'),
        ('dept_work.update.own', 'organization_work.update.own'),
        ('dept_work.approve.department', 'organization_work.approve.organization_unit')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM permissions permission_row
USING duplicate_pairs
WHERE permission_row.id = duplicate_pairs.old_id;

WITH permission_renames (
    new_code,
    new_name,
    new_module,
    new_action,
    new_scope,
    new_description
) AS (
    VALUES
        (
            'academic_curriculum.manage.organization_unit',
            'จัดการรายวิชากลุ่มสาระตัวเอง',
            'academic_curriculum',
            'manage',
            'organization_unit',
            'เพิ่ม/แก้ไข/ลบ รายวิชาเฉพาะกลุ่มสาระการเรียนรู้ที่ตัวเองสังกัด'
        ),
        (
            'organization_work.read.own',
            'ดูงานของตนเอง',
            'organization_work',
            'read',
            'own',
            'ดูรายการงานที่ตนเองรับผิดชอบ'
        ),
        (
            'organization_work.read.organization_unit',
            'ดูงานในหน่วยงาน',
            'organization_work',
            'read',
            'organization_unit',
            'ดูรายการงานทั้งหมดในหน่วยงานที่สังกัด'
        ),
        (
            'organization_work.create',
            'สร้าง/ส่งงาน',
            'organization_work',
            'create',
            'own',
            'ส่งงานหรือสร้างบันทึกงานใหม่'
        ),
        (
            'organization_work.update.own',
            'แก้ไขงานตนเอง',
            'organization_work',
            'update',
            'own',
            'แก้ไขรายละเอียดงานของตนเอง'
        ),
        (
            'organization_work.approve.organization_unit',
            'อนุมัติงานในหน่วยงาน',
            'organization_work',
            'approve',
            'organization_unit',
            'อนุมัติหรือตรวจสอบงานของสมาชิกในหน่วยงาน'
        )
)
UPDATE permissions p
SET name = permission_renames.new_name,
    module = permission_renames.new_module,
    action = permission_renames.new_action,
    scope = permission_renames.new_scope,
    description = permission_renames.new_description,
    updated_at = NOW()
FROM permission_renames
WHERE p.code = permission_renames.new_code;
