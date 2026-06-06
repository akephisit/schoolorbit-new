-- Migration 124: Normalize organization permission grant baseline
-- ORG-GRANTS-BASELINE-V1
--
-- Organization unit placement is the source of scoped workflow access. This
-- migration resets only the grant template owned by the organization baseline;
-- user roles, delegations, and memberships are configured separately.

DO $$
DECLARE
    missing_permissions INTEGER;
BEGIN
    WITH required_permissions(code) AS (
        VALUES
            ('academic_curriculum.manage.organization_unit'),
            ('organization_work.read.own'),
            ('organization_work.read.organization_unit'),
            ('organization_work.create'),
            ('organization_work.update.own'),
            ('organization_work.approve.organization_unit'),
            ('staff_profile.read.organization_tree'),
            ('staff_profile.read.school'),
            ('staff_pii.read.school')
    )
    SELECT COUNT(*)
    INTO missing_permissions
    FROM required_permissions required
    LEFT JOIN permissions p ON p.code = required.code
    WHERE p.id IS NULL;

    IF missing_permissions > 0 THEN
        RAISE EXCEPTION 'Organization permission grant baseline failed: % required permission(s) missing', missing_permissions;
    END IF;
END $$;

WITH managed_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'academic_curriculum.manage.organization_unit',
        'organization_work.read.own',
        'organization_work.read.organization_unit',
        'organization_work.create',
        'organization_work.update.own',
        'organization_work.approve.organization_unit',
        'staff_profile.read.organization_unit',
        'staff_profile.read.organization_tree',
        'staff_profile.read.school',
        'staff_pii.read.school'
    )
)
DELETE FROM organization_permission_grants opg
USING managed_permissions
WHERE opg.permission_id = managed_permissions.id;

WITH subject_permissions AS (
    SELECT id
    FROM permissions
    WHERE code = 'academic_curriculum.manage.organization_unit'
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at
)
SELECT ou.id,
       subject_permissions.id,
       NULL,
       NOW()
FROM organization_units ou
CROSS JOIN subject_permissions
WHERE ou.is_active = true
  AND ou.code LIKE 'SUBJ-%'
  AND ou.unit_type = 'subject_group';

WITH member_work_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'organization_work.read.own',
        'organization_work.create',
        'organization_work.update.own'
    )
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at
)
SELECT ou.id,
       member_work_permissions.id,
       NULL,
       NOW()
FROM organization_units ou
CROSS JOIN member_work_permissions
WHERE ou.is_active = true
  AND ou.code <> 'SCHOOL';

WITH leader_positions(position_code) AS (
    VALUES
        ('head'),
        ('deputy_head'),
        ('deputy_director')
),
leader_work_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'organization_work.read.organization_unit',
        'organization_work.approve.organization_unit',
        'staff_profile.read.organization_tree'
    )
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at
)
SELECT ou.id,
       leader_work_permissions.id,
       leader_positions.position_code,
       NOW()
FROM organization_units ou
CROSS JOIN leader_work_permissions
CROSS JOIN leader_positions
WHERE ou.is_active = true
  AND ou.code <> 'SCHOOL';

WITH school_director_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'staff_profile.read.school',
        'staff_pii.read.school',
        'organization_work.read.organization_unit',
        'organization_work.approve.organization_unit'
    )
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    position_code,
    created_at
)
SELECT school.id,
       school_director_permissions.id,
       'director',
       NOW()
FROM organization_units school
CROSS JOIN school_director_permissions
WHERE school.code = 'SCHOOL'
  AND school.is_active = true;

DO $$
DECLARE
    invalid_subject_grants INTEGER;
    missing_subject_grants INTEGER;
    missing_school_director_grants INTEGER;
BEGIN
    SELECT COUNT(*)
    INTO invalid_subject_grants
    FROM organization_permission_grants opg
    JOIN permissions p ON p.id = opg.permission_id
    JOIN organization_units ou ON ou.id = opg.organization_unit_id
    WHERE p.code = 'academic_curriculum.manage.organization_unit'
      AND (
          ou.code NOT LIKE 'SUBJ-%'
          OR ou.unit_type <> 'subject_group'
          OR opg.position_code IS NOT NULL
      );

    IF invalid_subject_grants > 0 THEN
        RAISE EXCEPTION 'Organization permission grant baseline failed: % invalid subject curriculum grant(s)', invalid_subject_grants;
    END IF;

    SELECT COUNT(*)
    INTO missing_subject_grants
    FROM organization_units ou
    WHERE ou.is_active = true
      AND ou.code LIKE 'SUBJ-%'
      AND ou.unit_type = 'subject_group'
      AND NOT EXISTS (
          SELECT 1
          FROM organization_permission_grants opg
          JOIN permissions p ON p.id = opg.permission_id
          WHERE opg.organization_unit_id = ou.id
            AND opg.position_code IS NULL
            AND p.code = 'academic_curriculum.manage.organization_unit'
      );

    IF missing_subject_grants > 0 THEN
        RAISE EXCEPTION 'Organization permission grant baseline failed: % subject group(s) missing curriculum manage grant', missing_subject_grants;
    END IF;

    WITH required_school_permissions(code) AS (
        VALUES
            ('staff_profile.read.school'),
            ('staff_pii.read.school')
    )
    SELECT COUNT(*)
    INTO missing_school_director_grants
    FROM required_school_permissions required
    JOIN permissions p ON p.code = required.code
    CROSS JOIN organization_units school
    LEFT JOIN organization_permission_grants opg
      ON opg.organization_unit_id = school.id
     AND opg.permission_id = p.id
     AND opg.position_code = 'director'
    WHERE school.code = 'SCHOOL'
      AND school.is_active = true
      AND opg.organization_unit_id IS NULL;

    IF missing_school_director_grants > 0 THEN
        RAISE EXCEPTION 'Organization permission grant baseline failed: SCHOOL/director missing % required grant(s)', missing_school_director_grants;
    END IF;
END $$;
