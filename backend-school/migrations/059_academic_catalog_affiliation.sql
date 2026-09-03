-- Canonical academic catalog affiliation.
--
-- A subject belongs to exactly one learning area. The responsible organization unit is
-- derived from that learning area. Every learner-development activity belongs to ACAD-ACT.
-- Learning offerings retain a required snapshot of the affiliation resolved from the catalog.

ALTER TABLE subjects
    ADD COLUMN subject_group_id UUID REFERENCES subject_groups(id) ON DELETE RESTRICT;

DO $$
DECLARE
    learner_activity_group_id UUID;
    learner_activity_unit_id UUID;
    invalid_subject_id UUID;
    invalid_group_id UUID;
BEGIN
    IF (SELECT COUNT(*) FROM subject_groups WHERE code = 'AC') <> 1 THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_ACTIVITY_GROUP_INVALID';
    END IF;

    IF (SELECT COUNT(*) FROM organization_units WHERE code = 'ACAD-ACT') <> 1 THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_ACTIVITY_UNIT_INVALID';
    END IF;

    SELECT id INTO learner_activity_group_id
      FROM subject_groups
     WHERE code = 'AC';

    SELECT id INTO learner_activity_unit_id
      FROM organization_units
     WHERE code = 'ACAD-ACT';

    IF NOT EXISTS (
        SELECT 1
          FROM subject_groups
         WHERE id = learner_activity_group_id
           AND is_active IS TRUE
    ) OR NOT EXISTS (
        SELECT 1
          FROM organization_units
         WHERE id = learner_activity_unit_id
           AND is_active IS TRUE
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_ACTIVITY_AFFILIATION_INACTIVE';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM organization_units
         WHERE id = learner_activity_unit_id
           AND subject_group_id IS NOT NULL
           AND subject_group_id <> learner_activity_group_id
    ) OR EXISTS (
        SELECT 1
          FROM organization_units
         WHERE id <> learner_activity_unit_id
           AND subject_group_id = learner_activity_group_id
           AND is_active IS TRUE
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_ACTIVITY_AFFILIATION_CONFLICT';
    END IF;

    SELECT version.subject_id
      INTO invalid_subject_id
      FROM subject_versions version
     WHERE version.group_id IS NOT NULL
     GROUP BY version.subject_id
    HAVING COUNT(DISTINCT version.group_id) > 1
     ORDER BY version.subject_id
     LIMIT 1;

    IF invalid_subject_id IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_SUBJECT_GROUP_AMBIGUOUS:%', invalid_subject_id;
    END IF;
END;
$$;

UPDATE organization_units unit
   SET subject_group_id = activity_group.id,
       metadata = unit.metadata || jsonb_build_object(
           'academicCatalogAffiliationMigration', 59
       ),
       updated_at = now()
  FROM subject_groups activity_group
 WHERE unit.code = 'ACAD-ACT'
   AND activity_group.code = 'AC';

WITH version_affiliations AS (
    SELECT version.subject_id,
           min(version.group_id::text)::uuid AS subject_group_id
      FROM subject_versions version
     WHERE version.group_id IS NOT NULL
     GROUP BY version.subject_id
)
UPDATE subjects subject
   SET subject_group_id = affiliation.subject_group_id,
       updated_at = now()
  FROM version_affiliations affiliation
 WHERE affiliation.subject_id = subject.id;

UPDATE subjects subject
   SET subject_group_id = owner.subject_group_id,
       updated_at = now()
  FROM organization_units owner
 WHERE subject.subject_group_id IS NULL
   AND owner.id = subject.owning_organization_unit_id
   AND owner.subject_group_id IS NOT NULL;

DO $$
DECLARE
    invalid_subject_id UUID;
    invalid_group_id UUID;
BEGIN
    SELECT subject.id
      INTO invalid_subject_id
      FROM subjects subject
      LEFT JOIN subject_groups subject_group ON subject_group.id = subject.subject_group_id
     WHERE subject.subject_group_id IS NULL
        OR subject_group.id IS NULL
        OR subject_group.is_active IS NOT TRUE
     ORDER BY subject.id
     LIMIT 1;

    IF invalid_subject_id IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_SUBJECT_GROUP_MISSING:%', invalid_subject_id;
    END IF;

    SELECT subject.subject_group_id
      INTO invalid_group_id
      FROM subjects subject
     LEFT JOIN organization_units owner
        ON owner.subject_group_id = subject.subject_group_id
       AND owner.is_active IS TRUE
     GROUP BY subject.subject_group_id
    HAVING COUNT(DISTINCT owner.id) <> 1
     ORDER BY subject.subject_group_id
     LIMIT 1;

    IF invalid_group_id IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_SUBJECT_OWNER_AMBIGUOUS:%', invalid_group_id;
    END IF;

    SELECT unit.subject_group_id
      INTO invalid_group_id
      FROM organization_units unit
     WHERE unit.subject_group_id IS NOT NULL
       AND unit.is_active IS TRUE
     GROUP BY unit.subject_group_id
    HAVING COUNT(*) > 1
     ORDER BY unit.subject_group_id
     LIMIT 1;

    IF invalid_group_id IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_ACTIVE_OWNER_DUPLICATE:%', invalid_group_id;
    END IF;
END;
$$;

UPDATE subjects subject
   SET owning_organization_unit_id = owner.id,
       updated_at = now()
  FROM organization_units owner
 WHERE owner.subject_group_id = subject.subject_group_id
   AND owner.is_active IS TRUE
   AND subject.owning_organization_unit_id IS DISTINCT FROM owner.id;

UPDATE activities activity
   SET owning_organization_unit_id = owner.id,
       updated_at = now()
  FROM organization_units owner
 WHERE owner.code = 'ACAD-ACT'
   AND activity.owning_organization_unit_id IS DISTINCT FROM owner.id;

ALTER TABLE learning_offerings
    DISABLE TRIGGER learning_offerings_published_immutable,
    DISABLE TRIGGER learning_offerings_exact_subtype;

UPDATE learning_offerings offering
   SET owning_organization_unit_id = subject.owning_organization_unit_id,
       updated_at = now()
  FROM course_offering_details detail
  JOIN subjects subject ON subject.id = detail.subject_id
 WHERE offering.id = detail.learning_offering_id
   AND offering.kind = 'course'
   AND offering.owning_organization_unit_id IS DISTINCT FROM subject.owning_organization_unit_id;

UPDATE learning_offerings offering
   SET owning_organization_unit_id = activity.owning_organization_unit_id,
       updated_at = now()
  FROM activity_offering_details detail
  JOIN activities activity ON activity.id = detail.activity_id
 WHERE offering.id = detail.learning_offering_id
   AND offering.kind = 'activity'
   AND offering.owning_organization_unit_id IS DISTINCT FROM activity.owning_organization_unit_id;

ALTER TABLE learning_offerings
    ENABLE TRIGGER learning_offerings_exact_subtype,
    ENABLE TRIGGER learning_offerings_published_immutable;

DO $$
DECLARE
    invalid_offering_id UUID;
BEGIN
    SELECT offering.id
      INTO invalid_offering_id
      FROM learning_offerings offering
      LEFT JOIN course_offering_details course
        ON course.learning_offering_id = offering.id
      LEFT JOIN subjects subject ON subject.id = course.subject_id
      LEFT JOIN activity_offering_details activity_detail
        ON activity_detail.learning_offering_id = offering.id
      LEFT JOIN activities activity ON activity.id = activity_detail.activity_id
     WHERE offering.owning_organization_unit_id IS DISTINCT FROM
           CASE offering.kind
               WHEN 'course' THEN subject.owning_organization_unit_id
               WHEN 'activity' THEN activity.owning_organization_unit_id
           END
     ORDER BY offering.id
     LIMIT 1;

    IF invalid_offering_id IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CATALOG_059_OFFERING_AFFILIATION_UNRESOLVED:%', invalid_offering_id;
    END IF;
END;
$$;

ALTER TABLE subjects
    ALTER COLUMN subject_group_id SET NOT NULL,
    ALTER COLUMN owning_organization_unit_id SET NOT NULL;

ALTER TABLE activities
    ALTER COLUMN owning_organization_unit_id SET NOT NULL;

ALTER TABLE learning_offerings
    ALTER COLUMN owning_organization_unit_id SET NOT NULL;

CREATE INDEX idx_subjects_subject_group ON subjects(subject_group_id);

CREATE UNIQUE INDEX organization_units_one_active_subject_group
    ON organization_units(subject_group_id)
    WHERE subject_group_id IS NOT NULL AND is_active IS TRUE;

DROP INDEX IF EXISTS idx_subjects_group;

ALTER TABLE subject_versions
    DROP CONSTRAINT IF EXISTS subjects_group_id_fkey,
    DROP COLUMN group_id;

COMMENT ON COLUMN subjects.subject_group_id IS
    'Canonical learning-area affiliation for the stable subject identity.';

COMMENT ON COLUMN subjects.owning_organization_unit_id IS
    'Policy owner derived from the canonical subject group.';

COMMENT ON COLUMN activities.owning_organization_unit_id IS
    'Policy owner fixed to the learner-development activity unit (ACAD-ACT).';

COMMENT ON COLUMN learning_offerings.owning_organization_unit_id IS
    'Required affiliation snapshot derived from the source catalog entry when opened.';

COMMENT ON COLUMN organization_units.subject_group_id IS
    'Maps an active academic owner to one canonical learning area, including ACAD-ACT.';
