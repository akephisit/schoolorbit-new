-- Default teaching supervision staff permissions.
-- Permission sync runs after migrations, so this migration upserts the required
-- permission rows before attaching them to roles and organization units.

WITH supervision_permissions (code, name, module, action, scope, description) AS (
    VALUES
        (
            'supervision.read.own',
            'ดูผลนิเทศของตนเอง',
            'supervision',
            'read',
            'own',
            'ดูรายการและผลนิเทศการสอนของตนเอง'
        ),
        (
            'supervision.request.own',
            'จองคาบนิเทศของตนเอง',
            'supervision',
            'request',
            'own',
            'ส่งคำขอจองคาบเพื่อรับการนิเทศการสอนของตนเอง'
        ),
        (
            'supervision.read.assigned',
            'ดูรายการนิเทศที่ได้รับมอบหมาย',
            'supervision',
            'read',
            'assigned',
            'ดูรายการนิเทศการสอนที่ได้รับมอบหมายให้ประเมิน'
        ),
        (
            'supervision.evaluate.assigned',
            'ประเมินรายการนิเทศที่ได้รับมอบหมาย',
            'supervision',
            'evaluate',
            'assigned',
            'กรอกและส่งผลประเมินรายการนิเทศที่ตนเองได้รับมอบหมาย'
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

WITH base_staff_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'supervision.read.own',
        'supervision.request.own',
        'supervision.read.assigned',
        'supervision.evaluate.assigned'
    )
),
staff_roles AS (
    SELECT id
    FROM roles
    WHERE user_type = 'staff'
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT staff_roles.id, base_staff_permissions.id, now()
FROM staff_roles
CROSS JOIN base_staff_permissions
ON CONFLICT DO NOTHING;

WITH base_staff_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'supervision.read.own',
        'supervision.request.own',
        'supervision.read.assigned',
        'supervision.evaluate.assigned'
    )
),
active_organization_units AS (
    SELECT id
    FROM organization_units
    WHERE is_active = true
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    created_at,
    created_by,
    position_code
)
SELECT active_organization_units.id, base_staff_permissions.id, now(), NULL, NULL
FROM active_organization_units
CROSS JOIN base_staff_permissions
ON CONFLICT DO NOTHING;
