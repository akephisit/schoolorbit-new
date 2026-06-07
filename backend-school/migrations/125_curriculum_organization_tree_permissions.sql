-- Migration 125: Add explicit curriculum organization-tree permissions.
--
-- Permission grants do not inherit through the organization hierarchy. A parent
-- unit that needs curriculum access for child subject groups must receive an
-- explicit organization_tree permission grant.

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'academic_curriculum.read.organization_tree',
        'ดูหลักสูตร/รายวิชาในสายงาน',
        'academic_curriculum',
        'read',
        'organization_tree',
        'ดูข้อมูลรายวิชาในหน่วยงานของตนเองและหน่วยงานย่อย'
    ),
    (
        'academic_curriculum.manage.organization_tree',
        'จัดการรายวิชาในสายงาน',
        'academic_curriculum',
        'manage',
        'organization_tree',
        'เพิ่ม/แก้ไข/ลบ รายวิชาในหน่วยงานของตนเองและหน่วยงานย่อย'
    )
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

DO $$
DECLARE
    missing_permissions INTEGER;
BEGIN
    WITH required_permissions(code) AS (
        VALUES
            ('academic_curriculum.read.organization_tree'),
            ('academic_curriculum.manage.organization_tree')
    )
    SELECT COUNT(*)
    INTO missing_permissions
    FROM required_permissions required
    LEFT JOIN permissions p ON p.code = required.code
    WHERE p.id IS NULL;

    IF missing_permissions > 0 THEN
        RAISE EXCEPTION 'Curriculum organization tree permission migration failed: % permission(s) missing', missing_permissions;
    END IF;
END $$;
