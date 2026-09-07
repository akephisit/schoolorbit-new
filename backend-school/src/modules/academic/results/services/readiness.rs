use super::*;
use crate::{
    middleware::permission::ActorContext, policies::academic_result_access_policy as access_policy,
};
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct CourseReadinessRow {
    subject_id: Uuid,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    group_name: String,
    offering_name: String,
    assigned: bool,
    locked: bool,
    plan_current: bool,
    phase_confirmations_current: bool,
    result_confirmation_exists: bool,
    result_confirmation_current: bool,
    primary_exists: bool,
    active_policy_valid: bool,
}

#[derive(sqlx::FromRow)]
struct ActivityReadinessRow {
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    group_name: String,
    offering_name: String,
    assigned: bool,
    locked: bool,
    outcomes_complete: bool,
    confirmation_exists: bool,
    confirmation_current: bool,
    primary_exists: bool,
}

fn blocker(code: ResultBlockerCode) -> ResultBlocker {
    ResultBlocker {
        code,
        assessment_phase_id: None,
        student_academic_year_id: None,
    }
}

pub async fn readiness(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
) -> Result<ResultReadiness, AppError> {
    access_policy::require_school_readiness(pool, actor).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, context).await?;
    // Both queues are bounded set queries. There is no per-group service/query loop.
    let course_rows: Vec<CourseReadinessRow> = sqlx::query_as(
        r#"WITH active_policy AS (
               SELECT id FROM academic_grading_policy_versions WHERE lifecycle='active'
           )
           SELECT d.subject_id,g.id AS learning_group_id,o.id AS learning_offering_id,
                  g.name AS group_name,o.name_snapshot AS offering_name,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.teacher_id=$3
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
                  lock.id IS NOT NULL AS locked,
                  (SELECT count(*)=4 AND count(DISTINCT phase.phase_code)=4 AND COALESCE(sum(phase.max_score),0)=d.assessment_total_score
                     FROM course_assessment_plans plan JOIN course_assessment_phases phase ON phase.plan_id=plan.id
                    WHERE plan.learning_offering_id=o.id) AS plan_current,
                  (SELECT count(*)=4 AND count(confirmation.id)=4
                              AND bool_and(NOT COALESCE((confirmation.source_snapshot->>'invalidated')::boolean,false)
                                           AND confirmation.phase_row_version=phase.row_version
                                           AND confirmation.source_snapshot->'roster'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                                               'membershipId',member.id,'studentAcademicYearId',member.student_academic_year_id,'rowVersion',member.row_version)
                                               ORDER BY member.student_academic_year_id) FROM learning_group_students member
                                               WHERE member.learning_group_id=g.id AND member.membership_status='active'),'[]'::jsonb)
                                           AND confirmation.source_snapshot->'items'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                                               'id',item.id,'maxScore',item.max_score::text) ORDER BY item.id)
                                               FROM learning_group_score_items item
                                               WHERE item.learning_group_id=g.id AND item.assessment_phase_id=phase.id
                                                 AND item.lifecycle='active'),'[]'::jsonb)
                                           AND confirmation.source_snapshot->'scores'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                                               'scoreItemId',score.score_item_id,'studentAcademicYearId',score.student_academic_year_id,
                                               'value',score.score::text,'rowVersion',score.row_version)
                                               ORDER BY score.score_item_id,score.student_academic_year_id)
                                               FROM learning_group_student_scores score
                                               JOIN learning_group_score_items item ON item.id=score.score_item_id
                                                 AND item.learning_group_id=g.id AND item.assessment_phase_id=phase.id
                                                 AND item.lifecycle='active'
                                               JOIN learning_group_students member ON member.learning_group_id=g.id
                                                 AND member.student_academic_year_id=score.student_academic_year_id
                                                 AND member.membership_status='active'
                                               WHERE score.learning_group_id=g.id),'[]'::jsonb))
                     FROM course_assessment_plans plan JOIN course_assessment_phases phase ON phase.plan_id=plan.id
                     LEFT JOIN learning_group_phase_confirmations confirmation
                       ON confirmation.learning_group_id=g.id AND confirmation.assessment_phase_id=phase.id
                    WHERE plan.learning_offering_id=o.id) AS phase_confirmations_current,
                  result_confirmation.id IS NOT NULL AS result_confirmation_exists,
                  (result_confirmation.id IS NOT NULL
                   AND NOT COALESCE((result_confirmation.source_snapshot->>'invalidated')::boolean,false)
                   AND result_confirmation.policy_version_id=(SELECT id FROM active_policy)
                   AND COALESCE(result_confirmation.confirmed_by=(SELECT teacher.teacher_id FROM learning_group_teachers teacher
                         JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                        WHERE teacher.learning_group_id=g.id AND teacher.role='primary'
                          AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                          AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))
                        ORDER BY teacher.id LIMIT 1), false)
                   AND result_confirmation.source_snapshot->'roster'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                         'membershipId',member.id,'studentAcademicYearId',member.student_academic_year_id,'rowVersion',member.row_version)
                         ORDER BY member.student_academic_year_id) FROM learning_group_students member
                         WHERE member.learning_group_id=g.id AND member.membership_status='active'),'[]'::jsonb)
                   AND result_confirmation.source_snapshot->'selections'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                         'studentAcademicYearId',member.student_academic_year_id,
                         'selection',COALESCE(override.outcome,'derived'),
                         'rowVersion',override.row_version) ORDER BY member.student_academic_year_id)
                         FROM learning_group_students member
                         LEFT JOIN learning_group_result_overrides override
                           ON override.learning_group_id=g.id
                          AND override.student_academic_year_id=member.student_academic_year_id
                         WHERE member.learning_group_id=g.id AND member.membership_status='active'),'[]'::jsonb)
                   AND NOT EXISTS (
                       SELECT 1 FROM learning_group_phase_confirmations current_phase
                       JOIN course_assessment_phases phase ON phase.id=current_phase.assessment_phase_id
                       JOIN course_assessment_plans plan ON plan.id=phase.plan_id AND plan.learning_offering_id=o.id
                       WHERE current_phase.learning_group_id=g.id AND NOT EXISTS (
                           SELECT 1 FROM jsonb_array_elements(COALESCE(result_confirmation.source_snapshot->'phases','[]'::jsonb)) snapshot
                           WHERE snapshot->>'assessmentPhaseId'=current_phase.assessment_phase_id::text
                             AND (snapshot->>'rowVersion')::bigint=current_phase.row_version))) AS result_confirmation_current,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.role='primary'
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS primary_exists,
                  (EXISTS(SELECT 1 FROM active_policy)
                   AND (SELECT count(*)=8 AND min(lower_bound) FILTER (WHERE grade=0)=0
                               AND max(lower_bound)<=d.assessment_total_score
                          FROM academic_grading_policy_bands WHERE policy_version_id=(SELECT id FROM active_policy))
                   AND NOT EXISTS(SELECT 1 FROM academic_grading_policy_bands lower_band
                                  JOIN academic_grading_policy_bands upper_band
                                    ON upper_band.policy_version_id=lower_band.policy_version_id
                                   AND upper_band.grade>lower_band.grade
                                 WHERE lower_band.policy_version_id=(SELECT id FROM active_policy)
                                   AND upper_band.lower_bound<=lower_band.lower_bound)) AS active_policy_valid
           FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.kind='course'
           JOIN course_offering_details d ON d.learning_offering_id=o.id
           JOIN academic_terms t ON t.id=g.academic_term_id
           LEFT JOIN learning_group_result_confirmations result_confirmation ON result_confirmation.learning_group_id=g.id
           LEFT JOIN academic_course_result_locks lock ON lock.subject_id=d.subject_id
             AND lock.academic_term_id=g.academic_term_id
             AND lock.academic_year_id=g.academic_year_id
           WHERE g.academic_term_id=$1 AND g.academic_year_id=$2 AND g.status<>'closed'
           ORDER BY d.subject_id,g.id LIMIT 5001"#,
    )
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(actor.user_id)
    .fetch_all(&mut *tx)
    .await?;
    if course_rows.len() > 5000 {
        return Err(AppError::ValidationError(
            "Course readiness exceeds 5000 groups".into(),
        ));
    }
    let activity_rows: Vec<ActivityReadinessRow> = sqlx::query_as(
        r#"SELECT g.id AS learning_group_id,o.id AS learning_offering_id,g.name AS group_name,
                  o.name_snapshot AS offering_name,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.teacher_id=$3
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
                  lock.id IS NOT NULL AS locked,
                  (SELECT count(*) FROM learning_group_students member WHERE member.learning_group_id=g.id AND member.membership_status='active')
                    =(SELECT count(*) FROM academic_activity_evaluations value JOIN learning_group_students member
                        ON member.learning_group_id=value.learning_group_id AND member.student_academic_year_id=value.student_academic_year_id
                       WHERE value.learning_group_id=g.id AND member.membership_status='active') AS outcomes_complete,
                  confirmation.id IS NOT NULL AS confirmation_exists,
                  (confirmation.id IS NOT NULL
                   AND NOT COALESCE((confirmation.source_snapshot->>'invalidated')::boolean,false)
                   AND COALESCE(confirmation.confirmed_by=(SELECT teacher.teacher_id FROM learning_group_teachers teacher
                         JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                        WHERE teacher.learning_group_id=g.id AND teacher.role='primary'
                          AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                          AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))
                        ORDER BY teacher.id LIMIT 1), false)
                   AND confirmation.source_snapshot->'roster'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                         'membershipId',member.id,'studentAcademicYearId',member.student_academic_year_id,'rowVersion',member.row_version)
                         ORDER BY member.student_academic_year_id) FROM learning_group_students member
                         WHERE member.learning_group_id=g.id AND member.membership_status='active'),'[]'::jsonb)
                   AND confirmation.source_snapshot->'outcomes'=COALESCE((SELECT jsonb_agg(jsonb_build_object(
                         'studentAcademicYearId',value.student_academic_year_id,'outcome',value.outcome,'rowVersion',value.row_version)
                         ORDER BY value.student_academic_year_id) FROM academic_activity_evaluations value
                         JOIN learning_group_students member ON member.learning_group_id=value.learning_group_id
                           AND member.student_academic_year_id=value.student_academic_year_id AND member.membership_status='active'
                         WHERE value.learning_group_id=g.id),'[]'::jsonb)) AS confirmation_current,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.role='primary'
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS primary_exists
           FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.kind='activity'
           JOIN activity_offering_details detail ON detail.learning_offering_id=o.id
           JOIN academic_terms t ON t.id=g.academic_term_id
           LEFT JOIN academic_activity_result_confirmations confirmation ON confirmation.learning_group_id=g.id
           LEFT JOIN academic_activity_result_locks lock ON lock.learning_group_id=g.id
           WHERE g.academic_term_id=$1 AND g.academic_year_id=$2 AND g.status<>'closed'
           ORDER BY o.code_snapshot,g.id LIMIT 5001"#,
    )
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(actor.user_id)
    .fetch_all(&mut *tx)
    .await?;
    if activity_rows.len() > 5000 {
        return Err(AppError::ValidationError(
            "Activity readiness exceeds 5000 groups".into(),
        ));
    }
    tx.commit().await?;

    let mut course_groups: BTreeMap<Uuid, Vec<GroupResultReadiness>> = BTreeMap::new();
    for row in course_rows {
        let mut blockers = Vec::new();
        if row.locked {
            blockers.push(blocker(ResultBlockerCode::AlreadyLocked));
        }
        if !row.plan_current {
            blockers.push(blocker(ResultBlockerCode::InvalidAssessmentPlan));
        }
        if !row.phase_confirmations_current {
            blockers.push(blocker(ResultBlockerCode::StalePhaseConfirmation));
        }
        if !row.primary_exists {
            blockers.push(blocker(ResultBlockerCode::MissingPrimaryTeacher));
        }
        if !row.active_policy_valid {
            blockers.push(blocker(ResultBlockerCode::InvalidGradingPolicy));
        }
        if !row.result_confirmation_exists {
            blockers.push(blocker(ResultBlockerCode::MissingGroupConfirmation));
        } else if !row.result_confirmation_current {
            blockers.push(blocker(ResultBlockerCode::StaleGroupConfirmation));
        }
        course_groups
            .entry(row.subject_id)
            .or_default()
            .push(GroupResultReadiness {
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                subject_id: Some(row.subject_id),
                group_name: row.group_name,
                offering_name: row.offering_name,
                assigned: row.assigned,
                ready: blockers.is_empty(),
                locked: row.locked,
                blockers,
            });
    }
    let courses = course_groups
        .into_iter()
        .map(|(subject_id, groups)| SubjectResultReadiness {
            subject_id,
            ready: !groups.is_empty() && groups.iter().all(|group| group.ready),
            groups,
        })
        .collect();
    let activities = activity_rows
        .into_iter()
        .map(|row| {
            let mut blockers = Vec::new();
            if row.locked {
                blockers.push(blocker(ResultBlockerCode::AlreadyLocked));
            }
            if !row.outcomes_complete {
                blockers.push(blocker(ResultBlockerCode::MissingActivityOutcome));
            }
            if !row.primary_exists {
                blockers.push(blocker(ResultBlockerCode::MissingPrimaryTeacher));
            }
            if !row.confirmation_exists {
                blockers.push(blocker(ResultBlockerCode::MissingGroupConfirmation));
            } else if !row.confirmation_current {
                blockers.push(blocker(ResultBlockerCode::StaleGroupConfirmation));
            }
            GroupResultReadiness {
                learning_group_id: row.learning_group_id,
                learning_offering_id: row.learning_offering_id,
                subject_id: None,
                group_name: row.group_name,
                offering_name: row.offering_name,
                assigned: row.assigned,
                ready: blockers.is_empty(),
                locked: row.locked,
                blockers,
            }
        })
        .collect();
    Ok(ResultReadiness {
        courses,
        activities,
    })
}
