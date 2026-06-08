-- Migration 119: Organization Unit Foundation
-- Move school structure from department-named tables to organization_* tables.
-- Old tables are used only as the migration source; runtime code should use organization_*.

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. Rename organization structure tables
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE IF EXISTS departments RENAME TO organization_units;
ALTER TABLE IF EXISTS department_members RENAME TO organization_members;
ALTER TABLE IF EXISTS department_permissions RENAME TO organization_permission_grants;
ALTER TABLE IF EXISTS permission_delegations RENAME TO organization_permission_delegations;

ALTER TABLE IF EXISTS organization_units
    RENAME COLUMN parent_department_id TO parent_unit_id;

ALTER TABLE IF EXISTS organization_units
    RENAME COLUMN org_type TO unit_type;

ALTER TABLE IF EXISTS organization_members
    RENAME COLUMN department_id TO organization_unit_id;

ALTER TABLE IF EXISTS organization_members
    RENAME COLUMN position TO position_code;

ALTER TABLE IF EXISTS organization_members
    RENAME COLUMN is_primary_department TO is_primary;

ALTER TABLE IF EXISTS organization_permission_grants
    RENAME COLUMN department_id TO organization_unit_id;

ALTER TABLE IF EXISTS organization_permission_grants
    RENAME COLUMN position TO position_code;

ALTER TABLE IF EXISTS organization_permission_delegations
    RENAME COLUMN department_id TO organization_unit_id;

ALTER TABLE IF EXISTS user_roles
    RENAME COLUMN department_id TO organization_unit_id;

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. Organization unit metadata and normalized type/category
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE organization_units
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE organization_units
SET category = CASE
    WHEN code = 'ACAD-01' OR code LIKE 'ACAD-%' OR code LIKE 'SUBJ-%' THEN 'academic'
    WHEN code = 'PER-01' OR code LIKE 'PER-%' THEN 'personnel'
    WHEN code = 'BUD-01' OR code LIKE 'BUD-%' THEN 'budget'
    WHEN code = 'GEN-01' OR code LIKE 'GEN-%' THEN 'general'
    ELSE COALESCE(NULLIF(category, ''), 'other')
END;

UPDATE organization_units
SET unit_type = CASE
    WHEN subject_group_id IS NOT NULL THEN 'subject_group'
    WHEN parent_unit_id IS NULL THEN 'management_group'
    WHEN unit_type = 'group' THEN 'management_group'
    WHEN unit_type = 'unit' THEN 'division'
    ELSE COALESCE(NULLIF(unit_type, ''), 'division')
END;

INSERT INTO organization_units (
    id, code, name, name_en, description, parent_unit_id,
    category, unit_type, is_active, display_order, metadata
)
VALUES (
    gen_random_uuid(),
    'SCHOOL',
    'โรงเรียน',
    'School',
    'หน่วยงานรากของโครงสร้างโรงเรียน',
    NULL,
    'other',
    'school',
    true,
    -1000,
    '{}'::jsonb
)
ON CONFLICT (code) DO NOTHING;

UPDATE organization_units
SET parent_unit_id = (SELECT id FROM organization_units WHERE code = 'SCHOOL')
WHERE parent_unit_id IS NULL
  AND code <> 'SCHOOL';

ALTER TABLE organization_units
    DROP CONSTRAINT IF EXISTS organization_units_unit_type_check;

ALTER TABLE organization_units
    ADD CONSTRAINT organization_units_unit_type_check
    CHECK (unit_type IN (
        'school',
        'management_group',
        'division',
        'subject_group',
        'committee',
        'team',
        'unit',
        'custom'
    ));

ALTER TABLE organization_units
    DROP CONSTRAINT IF EXISTS organization_units_category_check;

ALTER TABLE organization_units
    ADD CONSTRAINT organization_units_category_check
    CHECK (category IN (
        'academic',
        'personnel',
        'budget',
        'general',
        'student_affairs',
        'administrative',
        'other'
    ));

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. Organization membership position normalization
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE organization_members
    ADD COLUMN IF NOT EXISTS position_title TEXT;

UPDATE organization_members
SET position_code = CASE
    WHEN position_code IN ('director', 'deputy_director', 'head', 'deputy_head', 'coordinator', 'member') THEN position_code
    WHEN position_code IN ('deputy', 'assistant_head') THEN 'deputy_head'
    WHEN position_code IN ('teacher', 'staff') THEN 'member'
    ELSE 'member'
END,
position_title = COALESCE(position_title, CASE
    WHEN position_code = 'director' THEN 'ผู้อำนวยการ'
    WHEN position_code = 'deputy_director' THEN 'รองผู้อำนวยการ'
    WHEN position_code = 'head' THEN 'หัวหน้า'
    WHEN position_code IN ('deputy_head', 'deputy', 'assistant_head') THEN 'รองหัวหน้า'
    WHEN position_code = 'coordinator' THEN 'ผู้ประสานงาน'
    ELSE 'สมาชิก'
END);

WITH ranked AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY user_id
               ORDER BY is_primary DESC, started_at DESC, created_at DESC, id
           ) AS rn
    FROM organization_members
    WHERE ended_at IS NULL OR ended_at > CURRENT_DATE
)
UPDATE organization_members om
SET is_primary = ranked.rn = 1
FROM ranked
WHERE om.id = ranked.id;

ALTER TABLE organization_members
    DROP CONSTRAINT IF EXISTS organization_members_position_code_check;

ALTER TABLE organization_members
    ADD CONSTRAINT organization_members_position_code_check
    CHECK (position_code IN (
        'director',
        'deputy_director',
        'head',
        'deputy_head',
        'coordinator',
        'member'
    ));

CREATE UNIQUE INDEX IF NOT EXISTS idx_org_members_one_active_primary
    ON organization_members(user_id)
    WHERE is_primary = true
      AND ended_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. Position-aware permission grants
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE organization_permission_grants
    DROP CONSTRAINT IF EXISTS department_permissions_pkey;

ALTER TABLE organization_permission_grants
    DROP CONSTRAINT IF EXISTS organization_permission_grants_pkey;

UPDATE organization_permission_grants
SET position_code = CASE
    WHEN position_code IN ('director', 'deputy_director', 'head', 'deputy_head', 'coordinator', 'member') THEN position_code
    WHEN position_code IN ('deputy', 'assistant_head') THEN 'deputy_head'
    WHEN position_code IN ('teacher', 'staff') THEN 'member'
    ELSE position_code
END
WHERE position_code IS NOT NULL;

ALTER TABLE organization_permission_grants
    DROP CONSTRAINT IF EXISTS organization_permission_grants_position_code_check;

ALTER TABLE organization_permission_grants
    ADD CONSTRAINT organization_permission_grants_position_code_check
    CHECK (
        position_code IS NULL OR position_code IN (
            'director',
            'deputy_director',
            'head',
            'deputy_head',
            'coordinator',
            'member'
        )
    );

CREATE UNIQUE INDEX IF NOT EXISTS idx_org_permission_grants_global_unique
    ON organization_permission_grants(organization_unit_id, permission_id)
    WHERE position_code IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_org_permission_grants_position_unique
    ON organization_permission_grants(organization_unit_id, permission_id, position_code)
    WHERE position_code IS NOT NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- 5. Rename indexes for clarity where PostgreSQL allows it
-- ─────────────────────────────────────────────────────────────────────────────

ALTER INDEX IF EXISTS idx_departments_code RENAME TO idx_organization_units_code;
ALTER INDEX IF EXISTS idx_departments_parent RENAME TO idx_organization_units_parent;
ALTER INDEX IF EXISTS idx_departments_is_active RENAME TO idx_organization_units_is_active;
ALTER INDEX IF EXISTS idx_departments_category RENAME TO idx_organization_units_category;
ALTER INDEX IF EXISTS idx_departments_org_type RENAME TO idx_organization_units_unit_type;
ALTER INDEX IF EXISTS idx_dept_members_user_id RENAME TO idx_organization_members_user_id;
ALTER INDEX IF EXISTS idx_dept_members_dept_id RENAME TO idx_organization_members_unit_id;
ALTER INDEX IF EXISTS idx_dept_members_position RENAME TO idx_organization_members_position;
ALTER INDEX IF EXISTS idx_dept_members_active RENAME TO idx_organization_members_active;
ALTER INDEX IF EXISTS idx_department_permissions_dept_id RENAME TO idx_org_permission_grants_unit_id;
ALTER INDEX IF EXISTS idx_delegations_to_user RENAME TO idx_org_delegations_to_user;
ALTER INDEX IF EXISTS idx_delegations_from_user RENAME TO idx_org_delegations_from_user;
ALTER INDEX IF EXISTS idx_delegations_active RENAME TO idx_org_delegations_active;
ALTER INDEX IF EXISTS idx_user_roles_department_id RENAME TO idx_user_roles_organization_unit_id;

-- ─────────────────────────────────────────────────────────────────────────────
-- 6. Comments
-- ─────────────────────────────────────────────────────────────────────────────

COMMENT ON TABLE organization_units IS
    'หน่วยงาน/กลุ่ม/ฝ่าย/กลุ่มสาระ/ทีมในโครงสร้างโรงเรียน';
COMMENT ON COLUMN organization_units.parent_unit_id IS
    'หน่วยงานแม่ในโครงสร้างองค์กร';
COMMENT ON COLUMN organization_units.unit_type IS
    'ประเภทหน่วยงาน: school, management_group, division, subject_group, committee, team, unit, custom';
COMMENT ON COLUMN organization_units.subject_group_id IS
    'เชื่อมกับ subject_groups เมื่อ unit_type = subject_group';
COMMENT ON TABLE organization_members IS
    'สมาชิกในหน่วยงานองค์กรพร้อมตำแหน่งและช่วงเวลาปฏิบัติหน้าที่';
COMMENT ON COLUMN organization_members.position_code IS
    'ตำแหน่งมาตรฐาน: director, deputy_director, head, deputy_head, coordinator, member';
COMMENT ON COLUMN organization_members.position_title IS
    'ชื่อเรียกตำแหน่งเฉพาะโรงเรียน เช่น หัวหน้ากลุ่มสาระ, รองผอ.กลุ่มบริหารวิชาการ';
COMMENT ON TABLE organization_permission_grants IS
    'สิทธิ์ที่หน่วยงานมอบให้สมาชิก โดยสามารถจำกัดตาม position_code';
COMMENT ON TABLE organization_permission_delegations IS
    'การมอบหมายสิทธิ์ชั่วคราวในบริบทหน่วยงานองค์กร';
