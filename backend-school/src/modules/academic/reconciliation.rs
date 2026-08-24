use crate::error::AppError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationCheck {
    pub code: String,
    pub passed: bool,
    pub source_count: i64,
    pub target_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcademicCutoverReconciliation {
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<ReconciliationCheck>,
}

const TARGET_PERMISSION_COUNT: i64 = 27;
pub const PHASE_A_MIGRATION_VERSION: i64 = 44;
pub const RECONCILIATION_MAPPING_VERSION: &str = "academic-core-v1-reconciliation";

fn check(code: &str, source_count: i64, target_count: i64, passed: bool) -> ReconciliationCheck {
    ReconciliationCheck {
        code: code.to_string(),
        passed,
        source_count,
        target_count,
    }
}

pub async fn reconcile_academic_core_cutover(
    pool: &PgPool,
) -> Result<AcademicCutoverReconciliation, AppError> {
    let mut transaction = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *transaction)
        .await?;

    let (source_snapshot_count, target_snapshot_count, audit_snapshot_matches): (i64, i64, bool) =
        sqlx::query_as(
            r#"
            WITH audit AS (
                SELECT source_counts, target_counts
                FROM academic_core_cutover_audits
                WHERE migration_version = 43
                  AND mapping_algorithm_version = 'academic-core-v1'
            )
            SELECT COALESCE((
                       SELECT SUM(value::bigint)
                       FROM audit, jsonb_each_text(audit.source_counts)
                   ), 0)::bigint,
                   COALESCE((
                       SELECT SUM(value::bigint)
                       FROM audit, jsonb_each_text(audit.target_counts)
                   ), 0)::bigint,
                   COALESCE(audit.source_counts = audit.target_counts, false)
            FROM (SELECT true) seed
            LEFT JOIN audit ON true
            "#,
        )
        .fetch_one(&mut *transaction)
        .await?;

    let (mapped_source_count, resolved_target_count): (i64, i64) = sqlx::query_as(
        r#"
        WITH resolved_targets AS (
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN course_assessment_plans target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'course_assessment_plans'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN course_assessment_categories target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'course_assessment_categories'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN course_assessment_items target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'course_assessment_items'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_timetable_entries target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_timetable_entries'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN timetable_entry_instructors target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'timetable_entry_instructors'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_exam_rounds target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_exam_rounds'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_exam_days target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_exam_days'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_exam_schedule_items target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_exam_schedule_items'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_exam_sessions target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_exam_sessions'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_exam_day_room_assignments target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_exam_day_room_assignments'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN supervision_cycles target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'supervision_cycles'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN supervision_observations target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'supervision_observations'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_question_bank_questions target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_question_bank_questions'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN academic_question_bank_choices target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'academic_question_bank_choices'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN admission_tracks target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'admission_tracks'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN admission_applications target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'admission_applications'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN admission_room_assignments target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'admission_room_assignments'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN calendar_events target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'calendar_events'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN calendar_event_targets target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'calendar_event_targets'
            UNION ALL
            SELECT map.source_table, map.source_id
            FROM academic_core_entity_map map
            JOIN certificate_campaigns target ON target.id = map.target_id
            WHERE map.migration_version = 43
              AND map.target_table = 'certificate_campaigns'
        )
        SELECT (SELECT COUNT(*) FROM academic_core_entity_map WHERE migration_version = 43),
               (SELECT COUNT(*) FROM resolved_targets)
        "#,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let physical_target_count = resolved_target_count;
    let count_snapshot_matches = audit_snapshot_matches
        && source_snapshot_count == mapped_source_count
        && target_snapshot_count == resolved_target_count;

    let orphan_count = mapped_source_count - resolved_target_count;

    let cross_context_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT plan.id
            FROM course_assessment_plans plan
            LEFT JOIN learning_offerings offering
              ON offering.id = plan.learning_offering_id
            LEFT JOIN course_offering_details detail
              ON detail.learning_offering_id = plan.learning_offering_id
            WHERE offering.id IS NULL
               OR detail.learning_offering_id IS NULL
               OR offering.kind <> 'course'
               OR offering.academic_term_id <> plan.academic_term_id
               OR offering.academic_year_id <> plan.academic_year_id
               OR detail.subject_version_id <> plan.subject_version_id
               OR detail.academic_term_id <> plan.academic_term_id
               OR detail.academic_year_id <> plan.academic_year_id
            UNION ALL
            SELECT entry.id
            FROM academic_timetable_entries entry
            LEFT JOIN academic_terms term ON term.id = entry.academic_term_id
            LEFT JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
            LEFT JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
            LEFT JOIN homerooms homeroom ON homeroom.id = entry.homeroom_id
            WHERE term.id IS NULL
               OR term.academic_year_id <> entry.academic_year_id
               OR (entry.learning_offering_id IS NOT NULL AND (
                      offering.id IS NULL
                   OR offering.academic_term_id <> entry.academic_term_id
                   OR offering.academic_year_id <> entry.academic_year_id
               ))
               OR (entry.learning_group_id IS NOT NULL AND (
                      learning_group.id IS NULL
                   OR learning_group.learning_offering_id IS DISTINCT FROM entry.learning_offering_id
                   OR learning_group.academic_term_id <> entry.academic_term_id
                   OR learning_group.academic_year_id <> entry.academic_year_id
               ))
               OR (entry.homeroom_id IS NOT NULL AND (
                      homeroom.id IS NULL
                   OR homeroom.academic_year_id <> entry.academic_year_id
               ))
            UNION ALL
            SELECT round.id
            FROM academic_exam_rounds round
            LEFT JOIN academic_terms term ON term.id = round.academic_term_id
            WHERE term.id IS NULL OR term.academic_year_id <> round.academic_year_id
            UNION ALL
            SELECT day.id
            FROM academic_exam_days day
            LEFT JOIN academic_exam_rounds round ON round.id = day.exam_round_id
            WHERE round.id IS NULL
               OR round.academic_term_id <> day.academic_term_id
               OR round.academic_year_id <> day.academic_year_id
            UNION ALL
            SELECT assignment.id
            FROM academic_exam_day_room_assignments assignment
            LEFT JOIN academic_exam_days day ON day.id = assignment.exam_day_id
            LEFT JOIN homerooms homeroom ON homeroom.id = assignment.homeroom_id
            WHERE day.id IS NULL
               OR day.academic_term_id <> assignment.academic_term_id
               OR day.academic_year_id <> assignment.academic_year_id
               OR homeroom.id IS NULL
               OR homeroom.academic_year_id <> assignment.academic_year_id
            UNION ALL
            SELECT item.id
            FROM academic_exam_schedule_items item
            LEFT JOIN academic_exam_rounds round ON round.id = item.exam_round_id
            LEFT JOIN learning_groups learning_group ON learning_group.id = item.learning_group_id
            LEFT JOIN learning_offerings offering ON offering.id = item.learning_offering_id
            LEFT JOIN course_offering_details detail
              ON detail.learning_offering_id = item.learning_offering_id
            LEFT JOIN course_assessment_plans plan ON plan.id = item.course_assessment_plan_id
            LEFT JOIN subject_versions version ON version.id = detail.subject_version_id
            LEFT JOIN homerooms homeroom ON homeroom.id = item.homeroom_id
            WHERE round.id IS NULL
               OR learning_group.id IS NULL
               OR offering.id IS NULL
               OR detail.learning_offering_id IS NULL
               OR plan.id IS NULL
               OR version.id IS NULL
               OR round.academic_term_id <> item.academic_term_id
               OR round.academic_year_id <> item.academic_year_id
               OR learning_group.learning_offering_id <> item.learning_offering_id
               OR learning_group.academic_term_id <> item.academic_term_id
               OR learning_group.academic_year_id <> item.academic_year_id
               OR offering.academic_term_id <> item.academic_term_id
               OR offering.academic_year_id <> item.academic_year_id
               OR detail.academic_term_id <> item.academic_term_id
               OR detail.academic_year_id <> item.academic_year_id
               OR plan.learning_offering_id <> item.learning_offering_id
               OR plan.academic_term_id <> item.academic_term_id
               OR plan.academic_year_id <> item.academic_year_id
               OR plan.subject_version_id <> detail.subject_version_id
               OR version.subject_id <> item.subject_id
               OR homeroom.id IS NULL
               OR homeroom.academic_year_id <> item.academic_year_id
            UNION ALL
            SELECT session.id
            FROM academic_exam_sessions session
            LEFT JOIN academic_exam_schedule_items item
              ON item.id = session.exam_schedule_item_id
            LEFT JOIN academic_exam_days day ON day.id = session.exam_day_id
            WHERE item.id IS NULL
               OR day.id IS NULL
               OR item.exam_round_id <> session.exam_round_id
               OR day.exam_round_id <> session.exam_round_id
            UNION ALL
            SELECT cycle.id
            FROM supervision_cycles cycle
            LEFT JOIN academic_terms term ON term.id = cycle.academic_term_id
            WHERE term.id IS NULL OR term.academic_year_id <> cycle.academic_year_id
            UNION ALL
            SELECT observation.id
            FROM supervision_observations observation
            LEFT JOIN supervision_cycles cycle ON cycle.id = observation.cycle_id
            LEFT JOIN academic_timetable_entries entry
              ON entry.id = observation.timetable_entry_id
            LEFT JOIN learning_groups learning_group
              ON learning_group.id = observation.learning_group_id
            LEFT JOIN homerooms homeroom ON homeroom.id = observation.homeroom_id
            WHERE cycle.id IS NULL
               OR cycle.academic_term_id <> observation.academic_term_id
               OR cycle.academic_year_id <> observation.academic_year_id
               OR (observation.timetable_entry_id IS NOT NULL AND (
                      entry.id IS NULL
                   OR entry.academic_term_id <> observation.academic_term_id
                   OR entry.academic_year_id <> observation.academic_year_id
                   OR entry.learning_group_id IS DISTINCT FROM observation.learning_group_id
                   OR entry.homeroom_id IS DISTINCT FROM observation.homeroom_id
               ))
               OR (observation.learning_group_id IS NOT NULL AND (
                      learning_group.id IS NULL
                   OR learning_group.academic_term_id <> observation.academic_term_id
                   OR learning_group.academic_year_id <> observation.academic_year_id
               ))
               OR (observation.homeroom_id IS NOT NULL AND (
                      homeroom.id IS NULL
                   OR homeroom.academic_year_id <> observation.academic_year_id
               ))
            UNION ALL
            SELECT question.id
            FROM academic_question_bank_questions question
            LEFT JOIN subjects subject ON subject.id = question.subject_id
            LEFT JOIN subject_versions migrated_version
              ON migrated_version.id::text =
                 question.migration_provenance ->> 'legacySubjectVersionId'
            WHERE subject.id IS NULL
               OR (
                    question.migration_provenance @> '{"migration": 43}'::jsonb
                    AND (
                        migrated_version.id IS NULL
                        OR migrated_version.subject_id <> question.subject_id
                    )
               )
            UNION ALL
            SELECT track.id
            FROM admission_tracks track
            LEFT JOIN admission_rounds round ON round.id = track.admission_round_id
            LEFT JOIN academic_years round_year ON round_year.id = round.academic_year_id
            LEFT JOIN study_programs program ON program.id = track.study_program_id
            LEFT JOIN curriculum_versions version ON version.id = program.curriculum_version_id
            LEFT JOIN academic_years starts ON starts.id = version.start_academic_year_id
            LEFT JOIN academic_years ends ON ends.id = version.end_academic_year_id
            WHERE round.id IS NULL
               OR round_year.id IS NULL
               OR version.id IS NULL
               OR starts.id IS NULL
               OR program.id IS NULL
               OR program.status = 'archived'
               OR track.academic_year_id <> round.academic_year_id
               OR starts.start_date > round_year.start_date
               OR (ends.id IS NOT NULL AND ends.end_date < round_year.end_date)
            UNION ALL
            SELECT assignment.id
            FROM admission_room_assignments assignment
            LEFT JOIN homerooms homeroom ON homeroom.id = assignment.homeroom_id
            LEFT JOIN student_academic_years student_year
              ON student_year.id = assignment.student_academic_year_id
            LEFT JOIN homeroom_placements placement
              ON placement.id = assignment.homeroom_placement_id
            WHERE homeroom.id IS NULL
               OR homeroom.academic_year_id <> assignment.academic_year_id
               OR (assignment.student_academic_year_id IS NOT NULL AND (
                      student_year.id IS NULL
                   OR student_year.academic_year_id <> assignment.academic_year_id
                   OR student_year.student_id IS DISTINCT FROM assignment.student_id
               ))
               OR assignment.student_id IS DISTINCT FROM (
                      SELECT application.created_user_id
                      FROM admission_applications application
                      WHERE application.id = assignment.application_id
                  )
               OR (assignment.homeroom_placement_id IS NOT NULL AND (
                      placement.id IS NULL
                   OR placement.student_academic_year_id IS DISTINCT FROM assignment.student_academic_year_id
                   OR placement.academic_year_id <> assignment.academic_year_id
                   OR placement.homeroom_id <> assignment.homeroom_id
               ))
               OR (assignment.student_confirmed AND (
                      assignment.student_academic_year_id IS NULL
                   OR assignment.homeroom_placement_id IS NULL
               ))
            UNION ALL
            SELECT application.id
            FROM admission_applications application
            LEFT JOIN homeroom_placements placement
              ON placement.id = application.homeroom_placement_id
            LEFT JOIN student_academic_years student_year
              ON student_year.id = application.student_academic_year_id
            WHERE (application.homeroom_placement_id IS NOT NULL AND (
                      placement.id IS NULL
                   OR placement.student_academic_year_id IS DISTINCT FROM application.student_academic_year_id
                  ))
               OR (application.student_academic_year_id IS NOT NULL AND (
                      student_year.id IS NULL
                   OR student_year.student_id IS DISTINCT FROM application.created_user_id
                  ))
               OR (application.status = 'enrolled' AND (
                      application.student_academic_year_id IS NULL
                   OR application.homeroom_placement_id IS NULL
                  ))
            UNION ALL
            SELECT event.id
            FROM calendar_events event
            LEFT JOIN academic_terms term ON term.id = event.academic_term_id
            WHERE event.academic_term_id IS NOT NULL
              AND (event.academic_year_id IS NULL
                   OR term.id IS NULL
                   OR term.academic_year_id <> event.academic_year_id)
            UNION ALL
            SELECT target.id
            FROM calendar_event_targets target
            LEFT JOIN calendar_events event ON event.id = target.event_id
            LEFT JOIN homerooms homeroom ON homeroom.id = target.homeroom_id
            WHERE event.id IS NULL
               OR target.academic_year_id IS DISTINCT FROM event.academic_year_id
               OR (target.homeroom_id IS NOT NULL AND (
                      homeroom.id IS NULL
                   OR homeroom.academic_year_id IS DISTINCT FROM target.academic_year_id
               ))
        ) violations
        "#,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let (
        source_principal_count,
        reconciled_principal_count,
        active_target_permission_count,
        active_legacy_permission_count,
    ): (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        WITH permission_map(source_code, target_code) AS (
            VALUES
                ('academic_structure.read.all', 'academic_context.read.school'),
                ('academic_structure.read.all', 'academic_year.read.school'),
                ('academic_structure.read.all', 'academic_term.read.school'),
				('academic_structure.read.all', 'academic_catalog.read.school'),
                ('academic_structure.manage.all', 'academic_context.read.school'),
                ('academic_structure.manage.all', 'academic_year.manage.school'),
                ('academic_structure.manage.all', 'academic_term.manage.school'),
				('academic_structure.manage.all', 'academic_catalog.manage.school'),
                ('academic_classroom.read.all', 'homeroom.read.school'),
                ('academic_classroom.create.all', 'homeroom.manage.school'),
                ('academic_classroom.update.all', 'homeroom.manage.school'),
                ('academic_classroom.delete.all', 'homeroom.manage.school'),
                ('academic_enrollment.read.all', 'student_academic_year.read.school'),
                ('academic_enrollment.update.all', 'student_academic_year.manage.school'),
                ('academic_course_plan.read.all', 'learning_offering.read.school'),
                ('academic_course_plan.manage.all', 'learning_offering.manage.school'),
                ('academic_curriculum.read.all', 'academic_curriculum.read.school'),
                ('academic_curriculum.read.organization_tree', 'academic_curriculum.read.organization_tree'),
                ('academic_curriculum.create.all', 'academic_curriculum.manage.school'),
                ('academic_curriculum.update.all', 'academic_curriculum.manage.school'),
                ('academic_curriculum.delete.all', 'academic_curriculum.manage.school'),
                ('academic_curriculum.manage.organization_unit', 'academic_curriculum.manage.organization_unit'),
                ('academic_curriculum.manage.organization_tree', 'academic_curriculum.manage.organization_tree'),
                ('activity.read.all', 'academic_catalog.read.school'),
                ('activity.read.all', 'learning_offering.read.school'),
                ('activity.manage.all', 'academic_catalog.manage.school'),
                ('activity.manage.all', 'learning_offering.manage.school'),
                ('activity.manage_members.all', 'learning_offering.manage.school'),
                ('activity.manage.own', 'learning_offering.manage.assigned')
        ),
        expected_grants AS (
            SELECT DISTINCT 'role:' || source_grant.role_id::text || ':' || mapping.target_code
                            AS grant_key
            FROM role_permissions source_grant
            JOIN permissions source_permission ON source_permission.id = source_grant.permission_id
            JOIN permission_map mapping ON mapping.source_code = source_permission.code
            UNION
            SELECT DISTINCT 'organization:' || source_grant.organization_unit_id::text || ':'
                            || COALESCE(source_grant.position_code, '') || ':' || mapping.target_code
            FROM organization_permission_grants source_grant
            JOIN permissions source_permission ON source_permission.id = source_grant.permission_id
            JOIN permission_map mapping ON mapping.source_code = source_permission.code
            UNION
            SELECT DISTINCT 'delegation:' || source_grant.from_user_id::text || ':'
                            || source_grant.to_user_id::text || ':'
                            || COALESCE(source_grant.organization_unit_id::text, '') || ':'
                            || source_grant.started_at::text || ':'
                            || COALESCE(source_grant.expires_at::text, '') || ':'
                            || COALESCE(source_grant.revoked_at::text, '') || ':'
                            || mapping.target_code
            FROM organization_permission_delegations source_grant
            JOIN permissions source_permission ON source_permission.id = source_grant.permission_id
            JOIN permission_map mapping ON mapping.source_code = source_permission.code
            UNION
            SELECT DISTINCT 'role:' || source_grant.role_id::text
                            || ':academic_context.read.school'
            FROM role_permissions source_grant
            JOIN permissions retained ON retained.id = source_grant.permission_id
            WHERE retained.is_active
              AND retained.module IN (
                  'academic_year', 'academic_term', 'academic_catalog',
                  'academic_curriculum', 'homeroom', 'student_academic_year',
                  'learning_offering', 'academic_assessment',
                  'academic_exam_schedule', 'academic_question_bank',
                  'academic_timetable_today'
              )
            UNION
            SELECT DISTINCT 'organization:' || source_grant.organization_unit_id::text || ':'
                            || COALESCE(source_grant.position_code, '')
                            || ':academic_context.read.school'
            FROM organization_permission_grants source_grant
            JOIN permissions retained ON retained.id = source_grant.permission_id
            WHERE retained.is_active
              AND retained.module IN (
                  'academic_year', 'academic_term', 'academic_catalog',
                  'academic_curriculum', 'homeroom', 'student_academic_year',
                  'learning_offering', 'academic_assessment',
                  'academic_exam_schedule', 'academic_question_bank',
                  'academic_timetable_today'
              )
            UNION
            SELECT DISTINCT 'delegation:' || source_grant.from_user_id::text || ':'
                            || source_grant.to_user_id::text || ':'
                            || COALESCE(source_grant.organization_unit_id::text, '') || ':'
                            || source_grant.started_at::text || ':'
                            || COALESCE(source_grant.expires_at::text, '') || ':'
                            || COALESCE(source_grant.revoked_at::text, '')
                            || ':academic_context.read.school'
            FROM organization_permission_delegations source_grant
            JOIN permissions retained ON retained.id = source_grant.permission_id
            WHERE retained.is_active
              AND retained.module IN (
                  'academic_year', 'academic_term', 'academic_catalog',
                  'academic_curriculum', 'homeroom', 'student_academic_year',
                  'learning_offering', 'academic_assessment',
                  'academic_exam_schedule', 'academic_question_bank',
                  'academic_timetable_today'
              )
        ),
        actual_grants AS (
            SELECT DISTINCT 'role:' || grant_row.role_id::text || ':' || permission.code AS grant_key
            FROM role_permissions grant_row
            JOIN permissions permission ON permission.id = grant_row.permission_id
            WHERE permission.is_active
            UNION
            SELECT DISTINCT 'organization:' || grant_row.organization_unit_id::text || ':'
                            || COALESCE(grant_row.position_code, '') || ':' || permission.code
            FROM organization_permission_grants grant_row
            JOIN permissions permission ON permission.id = grant_row.permission_id
            WHERE permission.is_active
            UNION
            SELECT DISTINCT 'delegation:' || grant_row.from_user_id::text || ':'
                            || grant_row.to_user_id::text || ':'
                            || COALESCE(grant_row.organization_unit_id::text, '') || ':'
                            || grant_row.started_at::text || ':'
                            || COALESCE(grant_row.expires_at::text, '') || ':'
                            || COALESCE(grant_row.revoked_at::text, '') || ':' || permission.code
            FROM organization_permission_delegations grant_row
            JOIN permissions permission ON permission.id = grant_row.permission_id
            WHERE permission.is_active
        ),
        permission_state AS (
            SELECT COUNT(*) FILTER (
                       WHERE is_active AND code IN (
                           'academic_context.read.school',
                           'academic_year.read.school', 'academic_year.manage.school',
                           'academic_term.read.school', 'academic_term.manage.school',
                           'academic_catalog.read.school',
                           'academic_catalog.manage.organization_unit',
                           'academic_catalog.manage.organization_tree',
                           'academic_catalog.manage.school',
                           'academic_curriculum.read.organization_unit',
                           'academic_curriculum.read.organization_tree',
                           'academic_curriculum.read.school',
                           'academic_curriculum.manage.organization_unit',
                           'academic_curriculum.manage.organization_tree',
                           'academic_curriculum.manage.school',
                           'homeroom.read.school', 'homeroom.manage.school',
                           'student_academic_year.read.school',
                           'student_academic_year.manage.school',
                           'learning_offering.read.assigned',
                           'learning_offering.read.organization_unit',
                           'learning_offering.read.organization_tree',
                           'learning_offering.read.school',
                           'learning_offering.manage.assigned',
                           'learning_offering.manage.organization_unit',
                           'learning_offering.manage.organization_tree',
                           'learning_offering.manage.school'
                       )
                   ) AS active_target_count,
                   COUNT(*) FILTER (
                       WHERE is_active AND (
                              code LIKE 'academic_structure.%'
                           OR code LIKE 'academic_classroom.%'
                           OR code LIKE 'academic_enrollment.%'
                           OR code LIKE 'academic_course_plan.%'
                           OR code IN (
                               'academic_curriculum.read.all',
                               'academic_curriculum.create.all',
                               'academic_curriculum.update.all',
                               'academic_curriculum.delete.all',
                               'activity.read.all', 'activity.manage.all',
                               'activity.manage_members.all', 'activity.manage.own',
                               'academic_promotion.read.all',
                               'academic_promotion.execute.all'
                           )
                       )
                   ) AS active_legacy_count
            FROM permissions
        )
        SELECT (SELECT COUNT(*) FROM expected_grants),
               (SELECT COUNT(*)
                FROM expected_grants expected
                JOIN actual_grants actual USING (grant_key)),
               permission_state.active_target_count,
               permission_state.active_legacy_count
        FROM permission_state
        "#,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let (active_year_count, active_term_count, aligned_active_term_count): (i64, i64, i64) =
        sqlx::query_as(
            r#"
            SELECT (SELECT COUNT(*) FROM academic_years WHERE status = 'active'),
                   (SELECT COUNT(*) FROM academic_terms WHERE status = 'active'),
                   (SELECT COUNT(*)
                    FROM academic_terms term
                    JOIN academic_years year ON year.id = term.academic_year_id
                    WHERE term.status = 'active' AND year.status = 'active')
            "#,
        )
        .fetch_one(&mut *transaction)
        .await?;

    let (source_checksum_matches, target_checksum_matches): (bool, bool) = sqlx::query_as(
        r#"
        WITH source_checksum AS (
            SELECT encode(sha256(convert_to(
                       COALESCE(string_agg(source_table || ':' || source_id::text, ','
                                           ORDER BY source_table, source_id), ''),
                       'UTF8'
                   )), 'hex') AS checksum
            FROM academic_core_entity_map
            WHERE migration_version = 43
        ),
        physical_all(target_table, target_id) AS (
            SELECT 'course_assessment_plans', id FROM course_assessment_plans
            UNION ALL SELECT 'course_assessment_categories', id FROM course_assessment_categories
            UNION ALL SELECT 'course_assessment_items', id FROM course_assessment_items
            UNION ALL SELECT 'academic_timetable_entries', id FROM academic_timetable_entries
            UNION ALL SELECT 'timetable_entry_instructors', id FROM timetable_entry_instructors
            UNION ALL SELECT 'academic_exam_rounds', id FROM academic_exam_rounds
            UNION ALL SELECT 'academic_exam_days', id FROM academic_exam_days
            UNION ALL SELECT 'academic_exam_schedule_items', id FROM academic_exam_schedule_items
            UNION ALL SELECT 'academic_exam_sessions', id FROM academic_exam_sessions
            UNION ALL SELECT 'academic_exam_day_room_assignments', id
                FROM academic_exam_day_room_assignments
            UNION ALL SELECT 'supervision_cycles', id FROM supervision_cycles
            UNION ALL SELECT 'supervision_observations', id FROM supervision_observations
            UNION ALL SELECT 'academic_question_bank_questions', id
                FROM academic_question_bank_questions
            UNION ALL SELECT 'academic_question_bank_choices', id
                FROM academic_question_bank_choices
            UNION ALL SELECT 'admission_tracks', id FROM admission_tracks
            UNION ALL SELECT 'admission_applications', id FROM admission_applications
            UNION ALL SELECT 'admission_room_assignments', id FROM admission_room_assignments
            UNION ALL SELECT 'calendar_events', id FROM calendar_events
            UNION ALL SELECT 'calendar_event_targets', id FROM calendar_event_targets
            UNION ALL SELECT 'certificate_campaigns', id FROM certificate_campaigns
        ),
        physical_targets AS (
            SELECT physical.target_table, physical.target_id
            FROM physical_all physical
            JOIN academic_core_entity_map map
              ON map.target_table = physical.target_table
             AND map.target_id = physical.target_id
             AND map.migration_version = 43
        ),
        target_checksum AS (
            SELECT encode(sha256(convert_to(
                       COALESCE(string_agg(target_table || ':' || target_id::text, ','
                                           ORDER BY target_table, target_id), ''),
                       'UTF8'
                   )), 'hex') AS checksum
            FROM physical_targets
        )
        SELECT COALESCE(audit.source_checksum = source.checksum, false),
               COALESCE(audit.target_checksum = target.checksum, false)
        FROM source_checksum source
        CROSS JOIN target_checksum target
        LEFT JOIN academic_core_cutover_audits audit
          ON audit.migration_version = 43
         AND audit.mapping_algorithm_version = 'academic-core-v1'
        "#,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let checks = vec![
        check(
            "ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS",
            source_snapshot_count,
            physical_target_count,
            count_snapshot_matches,
        ),
        check(
            "ACADEMIC_CORE_RECON_ORPHAN_COUNTS",
            0,
            orphan_count,
            orphan_count == 0,
        ),
        check(
            "ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS",
            0,
            cross_context_count,
            cross_context_count == 0,
        ),
        check(
            "ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS",
            source_principal_count + TARGET_PERMISSION_COUNT,
            reconciled_principal_count + active_target_permission_count,
            source_principal_count == reconciled_principal_count
                && active_target_permission_count == TARGET_PERMISSION_COUNT
                && active_legacy_permission_count == 0,
        ),
        check(
            "ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS",
            2,
            active_year_count + active_term_count,
            active_year_count <= 1
                && active_term_count <= 1
                && active_term_count == aligned_active_term_count,
        ),
        check(
            "ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS",
            i64::from(source_checksum_matches),
            i64::from(target_checksum_matches),
            source_checksum_matches && target_checksum_matches,
        ),
    ];

    transaction.commit().await?;

    Ok(AcademicCutoverReconciliation {
        migration_version: 43,
        passed: checks.iter().all(|entry| entry.passed),
        checks,
    })
}

pub async fn reconcile_and_record_academic_core_cutover(
    pool: &PgPool,
) -> Result<AcademicCutoverReconciliation, AppError> {
    let current_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await?;
    if current_version != PHASE_A_MIGRATION_VERSION {
        return Err(AppError::BadRequest(
            "ACADEMIC_CORE_PHASE_A_VERSION_MISMATCH".to_string(),
        ));
    }

    let report = reconcile_academic_core_cutover(pool).await?;
    if !report.passed {
        return Ok(report);
    }

    let source_counts = report
        .checks
        .iter()
        .map(|check| (check.code.clone(), check.source_count))
        .collect::<BTreeMap<_, _>>();
    let target_counts = report
        .checks
        .iter()
        .map(|check| (check.code.clone(), check.target_count))
        .collect::<BTreeMap<_, _>>();
    let source_counts = serde_json::to_value(source_counts).map_err(|_| {
        AppError::InternalServerError("Failed to encode reconciliation counts".to_string())
    })?;
    let target_counts = serde_json::to_value(target_counts).map_err(|_| {
        AppError::InternalServerError("Failed to encode reconciliation counts".to_string())
    })?;
    let source_checksum = hex::encode(Sha256::digest(source_counts.to_string().as_bytes()));
    let target_checksum = hex::encode(Sha256::digest(target_counts.to_string().as_bytes()));

    let result = sqlx::query(
        r#"
        INSERT INTO academic_core_cutover_audits (
            migration_version, mapping_algorithm_version, source_counts, target_counts,
            source_checksum, target_checksum
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (migration_version) DO UPDATE
        SET source_counts = EXCLUDED.source_counts,
            target_counts = EXCLUDED.target_counts,
            source_checksum = EXCLUDED.source_checksum,
            target_checksum = EXCLUDED.target_checksum
        WHERE academic_core_cutover_audits.mapping_algorithm_version =
              EXCLUDED.mapping_algorithm_version
        "#,
    )
    .bind(PHASE_A_MIGRATION_VERSION)
    .bind(RECONCILIATION_MAPPING_VERSION)
    .bind(source_counts)
    .bind(target_counts)
    .bind(source_checksum)
    .bind(target_checksum)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::InternalServerError(
            "Academic Core reconciliation marker conflicts with an existing audit".to_string(),
        ));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };

    async fn migrated_pool(name: &str) -> PgPool {
        let pool = create_named_test_pool(name).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();
        pool
    }

    fn named_check<'a>(
        report: &'a AcademicCutoverReconciliation,
        code: &str,
    ) -> &'a ReconciliationCheck {
        report
            .checks
            .iter()
            .find(|check| check.code == code)
            .unwrap_or_else(|| panic!("missing reconciliation check {code}"))
    }

    #[tokio::test]
    async fn reports_all_cutover_checks_for_untampered_data() {
        let pool = migrated_pool("academic_reconciliation_043_passing").await;
        let passing = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert_eq!(passing.migration_version, 43);
        assert!(passing.passed);
        assert_eq!(
            passing
                .checks
                .iter()
                .map(|check| check.code.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS",
                "ACADEMIC_CORE_RECON_ORPHAN_COUNTS",
                "ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS",
                "ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS",
                "ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS",
                "ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS",
            ]
        );
        assert!(passing.checks.iter().all(|check| check.passed));
    }

    #[tokio::test]
    async fn ignores_valid_rows_created_after_the_cutover_snapshot() {
        let pool = migrated_pool("academic_reconciliation_043_later_rows").await;
        sqlx::query(
            r#"INSERT INTO academic_question_bank_choices (
                   id, question_id, label, content, is_correct, sort_order
               ) VALUES (
                   '93100000-0000-0000-0000-000000000002',
                   '93000000-0000-0000-0000-000000000001', 'B',
                   '{"blocks":[]}'::jsonb, false, 2
               )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let report = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert!(report.passed);
        assert!(report.checks.iter().all(|check| check.passed));
    }

    #[tokio::test]
    async fn detects_exact_permission_grant_tampering() {
        let pool = migrated_pool("academic_reconciliation_043_permission").await;

        let deleted = sqlx::query(
            r#"DELETE FROM role_permissions target_grant
               USING permissions target_permission
               WHERE target_permission.id = target_grant.permission_id
                 AND target_permission.code = 'academic_year.read.school'
                 AND EXISTS (
                     SELECT 1
                     FROM role_permissions source_grant
                     JOIN permissions source_permission
                       ON source_permission.id = source_grant.permission_id
                     WHERE source_grant.role_id = target_grant.role_id
                       AND source_permission.code = 'academic_structure.read.all'
                 )"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(deleted.rows_affected(), 1);

        let tampered = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS").passed);
    }

    #[tokio::test]
    async fn detects_active_academic_context_tampering() {
        let pool = migrated_pool("academic_reconciliation_043_active_context").await;

        sqlx::query(
            r#"UPDATE academic_years
               SET status = 'ready'
               WHERE status = 'active'"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let tampered = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS").passed);
    }

    #[tokio::test]
    async fn detects_cross_context_corruption_even_if_a_constraint_is_removed() {
        let pool = migrated_pool("academic_reconciliation_043_cross_context").await;

        sqlx::query(
            r#"ALTER TABLE academic_question_bank_questions
               DROP CONSTRAINT academic_question_bank_questions_version_subject_fkey"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"UPDATE academic_question_bank_questions
               SET subject_id = (
                   SELECT subject_id
                   FROM subject_versions
                   WHERE id = '20000000-0000-0000-0000-000000000026'
               )
               WHERE id = '93000000-0000-0000-0000-000000000001'"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let tampered = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS").passed);
    }

    #[tokio::test]
    async fn detects_physical_target_and_checksum_tampering() {
        let pool = migrated_pool("academic_reconciliation_043_target_checksum").await;

        sqlx::query(
            r#"DELETE FROM academic_question_bank_choices
               WHERE id = '93100000-0000-0000-0000-000000000001'"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let tampered = reconcile_academic_core_cutover(&pool).await.unwrap();
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS").passed);
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_ORPHAN_COUNTS").passed);
        assert!(!named_check(&tampered, "ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS").passed);
    }
}
