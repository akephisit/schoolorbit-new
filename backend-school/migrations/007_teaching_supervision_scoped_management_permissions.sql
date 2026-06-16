-- Scoped teaching supervision management permissions.
-- Subject-group heads can manage booking requests for teachers in their own
-- subject group. School-level result approval remains a separate permission.

WITH supervision_permissions (code, name, module, action, scope, description) AS (
    VALUES
        (
            'supervision.manage.organization_unit',
            'จัดการนิเทศในหน่วยงาน',
            'supervision',
            'manage',
            'organization_unit',
            'อนุมัติคำขอและจัดการรายการนิเทศของบุคลากรในหน่วยงานเดียวกัน'
        ),
        (
            'supervision.manage.organization_tree',
            'จัดการนิเทศในสายงาน',
            'supervision',
            'manage',
            'organization_tree',
            'อนุมัติคำขอและจัดการรายการนิเทศในหน่วยงานของตนเองและหน่วยงานย่อย'
        )
)
INSERT INTO permissions (code, name, module, action, scope, description)
SELECT code, name, module, action, scope, description
FROM supervision_permissions
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;

WITH subject_group_manage_permission AS (
    SELECT id
    FROM permissions
    WHERE code = 'supervision.manage.organization_unit'
),
subject_group_units AS (
    SELECT id
    FROM organization_units
    WHERE unit_type = 'subject_group'
      AND is_active = true
),
subject_group_lead_positions AS (
    SELECT position_code
    FROM (VALUES ('head'), ('deputy_head')) AS positions(position_code)
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    created_at,
    created_by,
    position_code
)
SELECT
    subject_group_units.id,
    subject_group_manage_permission.id,
    now(),
    NULL,
    subject_group_lead_positions.position_code
FROM subject_group_units
CROSS JOIN subject_group_manage_permission
CROSS JOIN subject_group_lead_positions
ON CONFLICT DO NOTHING;
