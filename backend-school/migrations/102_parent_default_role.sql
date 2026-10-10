-- Provision the default role omitted from the baseline. Existing role policy is preserved.
WITH created AS (
    INSERT INTO roles (code, name, name_en, user_type, level, is_active)
    VALUES ('PARENT', 'ผู้ปกครอง', 'Parent', 'parent', 1, true)
    ON CONFLICT (code) DO NOTHING
    RETURNING id
)
INSERT INTO role_permissions (role_id, permission_id)
SELECT created.id, p.id FROM created CROSS JOIN permissions p
WHERE p.code = 'attendance.read.own'
ON CONFLICT DO NOTHING;

-- No role history distinguishes never-provisioned users from ended/custom assignments.
INSERT INTO user_roles (user_id, role_id, is_primary)
SELECT u.id, r.id, true
FROM users u JOIN roles r ON r.code = 'PARENT' AND r.user_type = 'parent' AND r.is_active
WHERE u.user_type = 'parent' AND u.status = 'active'
  AND EXISTS (SELECT 1 FROM student_parents sp WHERE sp.parent_user_id = u.id)
  AND NOT EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = u.id)
  AND EXISTS (SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id
              WHERE rp.role_id = r.id AND p.code = 'attendance.read.own')
ON CONFLICT DO NOTHING;
