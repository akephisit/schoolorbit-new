-- Academic workload and term delivery
--
-- Repair provenance-scoped legacy catalog workload with the approved 20-week
-- rule and give every course offering its own term-specific weekly target.

ALTER TABLE course_offering_details
    ADD COLUMN weekly_period_target INTEGER,
    ADD CONSTRAINT course_offering_details_weekly_period_target_check
        CHECK (weekly_period_target IS NULL OR weekly_period_target > 0);

ALTER TABLE subject_versions
    DISABLE TRIGGER subject_versions_published_immutable;

ALTER TABLE activity_versions
    DISABLE TRIGGER activity_versions_published_immutable;

ALTER TABLE course_offering_details
    DISABLE TRIGGER course_offering_details_published_immutable;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM subject_versions version
        WHERE version.migration_provenance @> '{"migration":41}'::jsonb
          AND version.periods_per_week IS NULL
          AND version.hours_per_semester > 0
          AND mod(version.hours_per_semester, 20) <> 0
          AND (
              EXISTS (
                  SELECT 1
                  FROM curriculum_course_requirements requirement
                  WHERE requirement.subject_version_id = version.id
              )
              OR EXISTS (
                  SELECT 1
                  FROM course_offering_details detail
                  WHERE detail.subject_version_id = version.id
              )
          )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_051_SUBJECT_HOURS_NOT_DIVISIBLE';
    END IF;
END
$$;

UPDATE subject_versions
SET periods_per_week = hours_per_semester / 20,
    migration_provenance = migration_provenance
        || jsonb_build_object(
            'workloadRepair',
            jsonb_build_object('migration', 51, 'instructionalWeeks', 20)
        ),
    row_version = row_version + 1,
    updated_at = now()
WHERE migration_provenance @> '{"migration":41}'::jsonb
  AND periods_per_week IS NULL
  AND hours_per_semester > 0
  AND mod(hours_per_semester, 20) = 0;

UPDATE activity_versions
SET hours_per_term = hours_per_week * 20,
    migration_provenance = migration_provenance
        || jsonb_build_object(
            'workloadRepair',
            jsonb_build_object('migration', 51, 'instructionalWeeks', 20)
        ),
    row_version = row_version + 1,
    updated_at = now()
WHERE migration_provenance @> '{"migration":41}'::jsonb
  AND hours_per_term IS NULL
  AND hours_per_week > 0;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM curriculum_course_requirements requirement
        JOIN subject_versions version ON version.id = requirement.subject_version_id
        WHERE version.credit <= 0
           OR version.hours_per_semester IS NULL
           OR version.hours_per_semester <= 0
           OR version.periods_per_week IS NULL
           OR version.periods_per_week <= 0
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_051_CURRICULUM_SUBJECT_METRICS_INCOMPLETE';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM curriculum_activity_requirements requirement
        JOIN activity_versions version ON version.id = requirement.activity_version_id
        WHERE version.hours_per_week <= 0
           OR version.hours_per_term IS NULL
           OR version.hours_per_term <= 0
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_051_CURRICULUM_ACTIVITY_METRICS_INCOMPLETE';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM course_offering_details detail
        JOIN subject_versions version ON version.id = detail.subject_version_id
        WHERE version.periods_per_week IS NULL
           OR version.periods_per_week <= 0
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_051_OFFERING_PERIOD_TARGET_UNRESOLVED';
    END IF;
END
$$;

UPDATE course_offering_details detail
SET weekly_period_target = version.periods_per_week,
    migration_provenance = detail.migration_provenance
        || jsonb_build_object(
            'workloadRepair',
            jsonb_build_object('migration', 51, 'instructionalWeeks', 20)
        )
FROM subject_versions version
WHERE version.id = detail.subject_version_id
  AND detail.weekly_period_target IS NULL;

SET CONSTRAINTS course_offering_details_exact_subtype IMMEDIATE;

ALTER TABLE course_offering_details
    ALTER COLUMN weekly_period_target SET NOT NULL;

ALTER TABLE subject_versions
    ENABLE TRIGGER subject_versions_published_immutable;

ALTER TABLE activity_versions
    ENABLE TRIGGER activity_versions_published_immutable;

ALTER TABLE course_offering_details
    ENABLE TRIGGER course_offering_details_published_immutable;

DO $$
DECLARE
    enabled_count INTEGER;
BEGIN
    SELECT count(*)::integer
    INTO enabled_count
    FROM pg_trigger trigger_record
    JOIN pg_class relation ON relation.oid = trigger_record.tgrelid
    JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
    WHERE namespace.nspname = current_schema()
      AND trigger_record.tgname = ANY(ARRAY[
          'subject_versions_published_immutable',
          'activity_versions_published_immutable',
          'course_offering_details_published_immutable'
      ])
      AND trigger_record.tgenabled = 'O';

    IF enabled_count <> 3 THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_051_TRIGGER_RESTORE_FAILED';
    END IF;
END
$$;
