WITH eligible_targets AS (
    SELECT version.id AS timetable_version_id,
           offering.id AS learning_offering_id,
           offering.academic_term_id,
           offering.academic_year_id,
           COALESCE(
               subject_version.periods_per_week,
               activity_version.periods_per_week
           ) AS weekly_period_target
    FROM academic_timetable_versions version
    JOIN learning_offerings offering
      ON offering.academic_term_id = version.academic_term_id
     AND offering.academic_year_id = version.academic_year_id
    LEFT JOIN course_offering_details course_detail
      ON course_detail.learning_offering_id = offering.id
    LEFT JOIN subject_versions subject_version
      ON subject_version.id = course_detail.subject_version_id
    LEFT JOIN activity_offering_details activity_detail
      ON activity_detail.learning_offering_id = offering.id
    LEFT JOIN activity_versions activity_version
      ON activity_version.id = activity_detail.activity_version_id
    WHERE version.status = 'draft'
      AND offering.status IN ('draft', 'published')
      AND (offering.starts_on IS NULL OR offering.starts_on <= version.effective_from)
      AND (offering.ends_on IS NULL OR offering.ends_on >= version.effective_from)
      AND COALESCE(
              subject_version.periods_per_week,
              activity_version.periods_per_week
          ) > 0
      AND EXISTS (
          SELECT 1
          FROM learning_offering_targets offering_target
          WHERE offering_target.learning_offering_id = offering.id
      )
),
inserted AS (
    INSERT INTO academic_timetable_version_targets (
        timetable_version_id,
        learning_offering_id,
        academic_term_id,
        academic_year_id,
        weekly_period_target,
        migration_provenance
    )
    SELECT eligible.timetable_version_id,
           eligible.learning_offering_id,
           eligible.academic_term_id,
           eligible.academic_year_id,
           eligible.weekly_period_target,
           jsonb_build_object('reconciledByMigration', 80)
    FROM eligible_targets eligible
    ON CONFLICT (timetable_version_id, learning_offering_id) DO NOTHING
    RETURNING timetable_version_id
)
UPDATE academic_timetable_versions version
SET row_version = version.row_version + 1,
    updated_at = now()
WHERE version.id IN (
    SELECT DISTINCT inserted.timetable_version_id
    FROM inserted
);
