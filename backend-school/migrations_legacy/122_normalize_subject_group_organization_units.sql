-- Migration 122: Normalize subject group organization units
-- Subject-group organization units must map directly to subject_groups by code
-- and live under Academic Affairs for organization-tree authorization.

-- The curriculum seed uses subject_groups.code = 'OC' for "การงานอาชีพ".
-- Older staff-management seed used the organization code SUBJ-OT. Rename it
-- so all subject-group organization units follow SUBJ-{subject_groups.code}.
UPDATE organization_units
SET code = 'SUBJ-OC',
    name_en = 'Occupations Department',
    updated_at = NOW()
WHERE code = 'SUBJ-OT'
  AND NOT EXISTS (
      SELECT 1
      FROM organization_units existing
      WHERE existing.code = 'SUBJ-OC'
  );

WITH academic_root AS (
    SELECT id
    FROM organization_units
    WHERE code = 'ACAD-01'
),
subject_unit_map AS (
    SELECT
        ou.id AS organization_unit_id,
        sg.id AS subject_group_id,
        academic_root.id AS academic_root_id
    FROM organization_units ou
    JOIN subject_groups sg ON ou.code = 'SUBJ-' || sg.code
    CROSS JOIN academic_root
    WHERE ou.code LIKE 'SUBJ-%'
)
UPDATE organization_units ou
SET subject_group_id = subject_unit_map.subject_group_id,
    parent_unit_id = subject_unit_map.academic_root_id,
    category = 'academic',
    unit_type = 'subject_group',
    updated_at = NOW()
FROM subject_unit_map
WHERE ou.id = subject_unit_map.organization_unit_id;

DO $$
DECLARE
    invalid_subject_units INTEGER;
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM organization_units
        WHERE code = 'ACAD-01'
    ) THEN
        RAISE EXCEPTION 'Missing ACAD-01 organization unit required for subject-group placement';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM organization_units
        WHERE code = 'SUBJ-OT'
    ) THEN
        RAISE EXCEPTION 'Legacy subject-group organization code SUBJ-OT remains; expected SUBJ-OC';
    END IF;

    WITH academic_root AS (
        SELECT id
        FROM organization_units
        WHERE code = 'ACAD-01'
    )
    SELECT COUNT(*)
    INTO invalid_subject_units
    FROM organization_units ou
    LEFT JOIN subject_groups sg ON sg.id = ou.subject_group_id
    CROSS JOIN academic_root
    WHERE ou.code LIKE 'SUBJ-%'
      AND (
          ou.subject_group_id IS NULL
          OR sg.id IS NULL
          OR ou.code <> 'SUBJ-' || sg.code
          OR ou.category <> 'academic'
          OR ou.unit_type <> 'subject_group'
          OR ou.parent_unit_id IS DISTINCT FROM academic_root.id
      );

    IF invalid_subject_units > 0 THEN
        RAISE EXCEPTION 'Subject-group organization normalization failed: % invalid SUBJ-* unit(s)', invalid_subject_units;
    END IF;
END $$;
