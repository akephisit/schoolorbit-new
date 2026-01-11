-- ===================================================================
-- Migration 014: Update ADMIN Role with Wildcard Permission (FIXED)
-- Description: Give ADMIN role wildcard (*) permission for all access
-- Date: 2025-12-30
-- Updated: 2026-01-11 - Use normalized schema
-- ===================================================================

-- Note: Wildcard '*' permission should be handled in permission registry
-- and inserted into permissions table, then assigned to ADMIN role

-- Ensure wildcard permission exists
INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
    '*',
    'ทั้งหมด (Wildcard)',
    'system',
    '*',
    'all',
    'สิทธิ์เข้าถึงทุกอย่างในระบบ (Admin only)'
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description;

-- Assign wildcard permission to ADMIN role
INSERT INTO role_permissions (role_id, permission_id)
SELECT 
    (SELECT id FROM roles WHERE code = 'ADMIN'),
    (SELECT id FROM permissions WHERE code = '*')
ON CONFLICT DO NOTHING;

-- Verify the update
SELECT 
    r.code, 
    r.name,
    COUNT(rp.permission_id) as permission_count,
    array_agg(p.code ORDER BY p.code) as permissions
FROM roles r
LEFT JOIN role_permissions rp ON r.id = rp.role_id
LEFT JOIN permissions p ON rp.permission_id = p.id
WHERE r.code = 'ADMIN'
GROUP BY r.id, r.code, r.name;
