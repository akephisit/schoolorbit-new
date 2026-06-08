-- Migration 121: Staff profile scoped access foundation.
-- Adds resource-aware staff profile permissions and makes SCHOOL root usable for directors.

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'staff_profile.read.own',
        'ดูโปรไฟล์บุคลากรของตนเอง',
        'staff_profile',
        'read',
        'own',
        'ดูข้อมูลโปรไฟล์บุคลากรของตนเอง'
    ),
    (
        'staff_profile.read.organization_unit',
        'ดูโปรไฟล์บุคลากรในหน่วยงาน',
        'staff_profile',
        'read',
        'organization_unit',
        'ดูข้อมูลโปรไฟล์บุคลากรที่อยู่ในหน่วยงานเดียวกัน'
    ),
    (
        'staff_profile.read.organization_tree',
        'ดูโปรไฟล์บุคลากรในสายงาน',
        'staff_profile',
        'read',
        'organization_tree',
        'ดูข้อมูลโปรไฟล์บุคลากรในหน่วยงานของตนเองและหน่วยงานย่อย'
    ),
    (
        'staff_profile.read.school',
        'ดูโปรไฟล์บุคลากรทั้งโรงเรียน',
        'staff_profile',
        'read',
        'school',
        'ดูข้อมูลโปรไฟล์บุคลากรทั้งโรงเรียน'
    ),
    (
        'staff_pii.read.own',
        'ดูข้อมูลอ่อนไหวบุคลากรของตนเอง',
        'staff_pii',
        'read',
        'own',
        'ดูข้อมูลอ่อนไหวของบุคลากรเฉพาะของตนเอง'
    ),
    (
        'staff_pii.read.school',
        'ดูข้อมูลอ่อนไหวบุคลากรทั้งโรงเรียน',
        'staff_pii',
        'read',
        'school',
        'ดูข้อมูลอ่อนไหวของบุคลากรทั้งโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

-- Preserve existing school-wide staff read behavior for roles/grants that already had staff.read.all.
WITH source_permission AS (
    SELECT id FROM permissions WHERE code = 'staff.read.all'
),
target_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN ('staff_profile.read.school', 'staff_pii.read.school')
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT rp.role_id, target_permissions.id, rp.created_at
FROM role_permissions rp
JOIN source_permission ON source_permission.id = rp.permission_id
CROSS JOIN target_permissions
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH source_permission AS (
    SELECT id FROM permissions WHERE code = 'staff.read.all'
),
target_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN ('staff_profile.read.school', 'staff_pii.read.school')
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at,
    created_by
)
SELECT opg.organization_unit_id,
       target_permissions.id,
       opg.position_code,
       opg.created_at,
       opg.created_by
FROM organization_permission_grants opg
JOIN source_permission ON source_permission.id = opg.permission_id
CROSS JOIN target_permissions
WHERE NOT EXISTS (
    SELECT 1
    FROM organization_permission_grants existing
    WHERE existing.organization_unit_id = opg.organization_unit_id
      AND existing.permission_id = target_permissions.id
      AND existing.position_code IS NOT DISTINCT FROM opg.position_code
);

WITH source_permission AS (
    SELECT id FROM permissions WHERE code = 'staff.read.all'
),
target_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN ('staff_profile.read.school', 'staff_pii.read.school')
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
       target_permissions.id,
       opd.organization_unit_id,
       opd.reason,
       opd.started_at,
       opd.expires_at,
       opd.revoked_at,
       opd.created_at
FROM organization_permission_delegations opd
JOIN source_permission ON source_permission.id = opd.permission_id
CROSS JOIN target_permissions
WHERE NOT EXISTS (
    SELECT 1
    FROM organization_permission_delegations existing
    WHERE existing.from_user_id = opd.from_user_id
      AND existing.to_user_id = opd.to_user_id
      AND existing.permission_id = target_permissions.id
      AND existing.organization_unit_id IS NOT DISTINCT FROM opd.organization_unit_id
      AND existing.started_at = opd.started_at
);

-- Ensure directors are attached to the SCHOOL root so organization-tree/school placement is explicit.
INSERT INTO organization_members (
    user_id,
    organization_unit_id,
    position_code,
    position_title,
    is_primary,
    responsibilities,
    started_at
)
SELECT ur.user_id,
       school.id,
       'director',
       'ผู้อำนวยการ',
       false,
       'ตำแหน่งระดับโรงเรียนสำหรับสิทธิ์และ workflow ทั้งโรงเรียน',
       COALESCE(ur.started_at, CURRENT_DATE)
FROM user_roles ur
JOIN roles r ON r.id = ur.role_id
JOIN organization_units school ON school.code = 'SCHOOL'
WHERE r.code = 'DIRECTOR'
  AND ur.ended_at IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM organization_members existing
      WHERE existing.user_id = ur.user_id
        AND existing.organization_unit_id = school.id
        AND (existing.ended_at IS NULL OR existing.ended_at > CURRENT_DATE)
  );

-- Directors at SCHOOL get school-wide staff profile and PII access through organization placement.
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at
)
SELECT school.id, p.id, 'director', NOW()
FROM organization_units school
CROSS JOIN permissions p
WHERE school.code = 'SCHOOL'
  AND p.code IN ('staff_profile.read.school', 'staff_pii.read.school')
  AND NOT EXISTS (
      SELECT 1
      FROM organization_permission_grants existing
      WHERE existing.organization_unit_id = school.id
        AND existing.permission_id = p.id
        AND existing.position_code = 'director'
  );
