-- Allow staff in a subject group to review assessment structures for the
-- same subject group while keeping edits scoped to assigned courses.

WITH assessment_permissions (code, name, module, action, scope, description) AS (
    VALUES (
        'academic_assessment.read.organization_unit',
        'ดูโครงสร้างคะแนนในกลุ่มสาระ',
        'academic_assessment',
        'read',
        'organization_unit',
        'ดูโครงสร้างคะแนนของรายวิชาในกลุ่มสาระเดียวกันแบบอ่านอย่างเดียว'
    )
)
INSERT INTO permissions (code, name, module, action, scope, description)
SELECT code, name, module, action, scope, description
FROM assessment_permissions
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;

WITH subject_group_read_permission AS (
    SELECT id
    FROM permissions
    WHERE code = 'academic_assessment.read.organization_unit'
),
subject_group_units AS (
    SELECT id
    FROM organization_units
    WHERE unit_type = 'subject_group'
      AND subject_group_id IS NOT NULL
      AND is_active = true
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    created_at,
    created_by,
    position_code
)
SELECT subject_group_units.id, subject_group_read_permission.id, now(), NULL, NULL
FROM subject_group_units
CROSS JOIN subject_group_read_permission
ON CONFLICT DO NOTHING;
