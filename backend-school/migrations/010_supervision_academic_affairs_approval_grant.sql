-- Grant final teaching supervision approval to Academic Affairs leads.
--
-- Subject-group heads certify completed evaluations. The Academic Affairs
-- management-group head/deputy head is the school-level approver for the
-- simplified supervision flow.

WITH approval_permission AS (
    INSERT INTO permissions (code, name, module, action, scope, description)
    VALUES (
        'supervision.approve.school',
        'อนุมัติผลนิเทศทั้งโรงเรียน',
        'supervision',
        'approve',
        'school',
        'อนุมัติผลนิเทศการสอนขั้นสุดท้ายในระดับโรงเรียน'
    )
    ON CONFLICT (code) DO UPDATE
    SET name = EXCLUDED.name,
        module = EXCLUDED.module,
        action = EXCLUDED.action,
        scope = EXCLUDED.scope,
        description = EXCLUDED.description
    RETURNING id
),
academic_affairs_unit AS (
    SELECT id
    FROM organization_units
    WHERE code = 'ACAD-01'
      AND is_active = true
),
academic_affairs_lead_positions AS (
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
    academic_affairs_unit.id,
    approval_permission.id,
    now(),
    NULL,
    academic_affairs_lead_positions.position_code
FROM academic_affairs_unit
CROSS JOIN approval_permission
CROSS JOIN academic_affairs_lead_positions
ON CONFLICT DO NOTHING;
