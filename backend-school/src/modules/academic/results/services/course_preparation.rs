use super::*;
use crate::{
    middleware::permission::ActorContext,
    modules::academic::gradebook::models::{GradebookStudent, ScoreCell, ScoreItem},
    policies::academic_result_access_policy as access_policy,
};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct CourseScope {
    group_id: Uuid,
    offering_id: Uuid,
    subject_id: Uuid,
    assessment_total_score: String,
    owning_organization_unit_id: Option<Uuid>,
    assigned: bool,
    primary_teacher_id: Option<Uuid>,
    locked: bool,
    locked_policy_id: Option<Uuid>,
    locked_policy_snapshot: Option<sqlx::types::Json<GradingPolicyVersion>>,
}

#[derive(sqlx::FromRow)]
struct PhaseRow {
    id: Uuid,
    phase_code: String,
    max_score: String,
    row_version: i64,
    confirmation_id: Option<Uuid>,
    confirmation_row_version: Option<i64>,
    confirmation_source_checksum: Option<String>,
    confirmation_roster_checksum: Option<String>,
    confirmation_invalidated: Option<bool>,
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    assessment_phase_id: Uuid,
    id: Uuid,
    name: String,
    max_score: String,
    display_order: i32,
    lifecycle: String,
    row_version: i64,
}

#[derive(sqlx::FromRow)]
struct ScoreRow {
    assessment_phase_id: Uuid,
    score_item_id: Uuid,
    student_academic_year_id: Uuid,
    value: Option<String>,
    row_version: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct OverrideRow {
    student_academic_year_id: Uuid,
    outcome: String,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RosterRevision {
    membership_id: Uuid,
    student_academic_year_id: Uuid,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseRevision {
    assessment_phase_id: Uuid,
    phase_code: String,
    confirmation_id: Option<Uuid>,
    row_version: Option<i64>,
    source_checksum: String,
    roster_checksum: String,
    current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionRevision {
    student_academic_year_id: Uuid,
    selection: CourseOutcomeSelection,
    row_version: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CourseConfirmationSnapshot<'a> {
    invalidated: bool,
    learning_group_id: Uuid,
    learning_offering_id: Uuid,
    source_checksum: &'a str,
    roster_checksum: &'a str,
    policy: &'a GradingPolicyVersion,
    phases: &'a [PhaseRevision],
    roster: Vec<RosterRevision>,
    selections: Vec<SelectionRevision>,
}

async fn load_scope(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    group: Uuid,
    context: &ResultContext,
) -> Result<CourseScope, AppError> {
    sqlx::query_as(
        r#"SELECT g.id AS group_id,g.learning_offering_id AS offering_id,d.subject_id,
                  d.assessment_total_score::text,o.owning_organization_unit_id,
                  EXISTS(SELECT 1 FROM learning_group_teachers teacher JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                         WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id
                           AND teacher.academic_year_id=g.academic_year_id AND teacher.teacher_id=$4
                           AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                           AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))) AS assigned,
                  (SELECT teacher.teacher_id FROM learning_group_teachers teacher
                     JOIN users u ON u.id=teacher.teacher_id AND u.status='active'
                    WHERE teacher.learning_group_id=g.id AND teacher.academic_term_id=g.academic_term_id
                      AND teacher.academic_year_id=g.academic_year_id AND teacher.role='primary'
                      AND teacher.starts_on<=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date)
                      AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(GREATEST(current_date,t.start_date),t.planned_end_date))
                    ORDER BY teacher.id LIMIT 1) AS primary_teacher_id,
                  lock.id IS NOT NULL AS locked,lock.policy_version_id AS locked_policy_id,
                  lock.policy_snapshot AS locked_policy_snapshot
           FROM learning_groups g
           JOIN learning_offerings o ON o.id=g.learning_offering_id AND o.kind='course'
           JOIN course_offering_details d ON d.learning_offering_id=o.id
           JOIN academic_terms t ON t.id=g.academic_term_id
           LEFT JOIN academic_course_result_locks lock ON lock.subject_id=d.subject_id
             AND lock.academic_term_id=g.academic_term_id
             AND lock.academic_year_id=g.academic_year_id
           WHERE g.id=$1 AND g.academic_term_id=$2 AND g.academic_year_id=$3 AND g.status<>'closed'"#,
    )
    .bind(group)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(actor.user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Course group not found in context".into()))
}

async fn begin_course<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    group: Uuid,
    context: &ResultContext,
    write: bool,
) -> Result<(Transaction<'a, Postgres>, CourseScope), AppError> {
    let access = access_policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    if !write {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    validate_context(&mut tx, context).await?;
    if write {
        let offering: Uuid = sqlx::query_scalar(
            "SELECT learning_offering_id FROM learning_groups WHERE id=$1 AND academic_term_id=$2 AND academic_year_id=$3",
        )
        .bind(group)
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Course group not found in context".into()))?;
        // Shared order for every result-source writer: offering, group, then sources.
        sqlx::query("SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE")
            .bind(offering)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM learning_groups WHERE id=$1 FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT phase.id FROM course_assessment_phases phase JOIN course_assessment_plans plan ON plan.id=phase.plan_id WHERE plan.learning_offering_id=$1 ORDER BY phase.id FOR SHARE OF phase")
            .bind(offering)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "SELECT id FROM academic_grading_policy_versions WHERE lifecycle='active' FOR SHARE",
        )
        .execute(&mut *tx)
        .await?;
    }
    let scope = load_scope(&mut tx, actor, group, context).await?;
    if !access_policy::can_read_group(&access, scope.owning_organization_unit_id, scope.assigned) {
        return Err(AppError::Forbidden("Course result access denied".into()));
    }
    if write {
        if !access_policy::can_confirm_course(
            actor,
            scope.primary_teacher_id == Some(actor.user_id),
        ) {
            return Err(AppError::Forbidden(
                "Current group primary teacher is required for course result preparation".into(),
            ));
        }
        require_course_offering_unlocked(
            &mut tx,
            scope.offering_id,
            context.academic_term_id,
            context.academic_year_id,
        )
        .await?;
        sqlx::query("SELECT id FROM learning_group_result_overrides WHERE learning_group_id=$1 ORDER BY id FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT id FROM learning_group_result_confirmations WHERE learning_group_id=$1 FOR UPDATE")
            .bind(group)
            .execute(&mut *tx)
            .await?;
    }
    Ok((tx, scope))
}

fn blocker(code: ResultBlockerCode, phase: Option<Uuid>, student: Option<Uuid>) -> ResultBlocker {
    ResultBlocker {
        code,
        assessment_phase_id: phase,
        student_academic_year_id: student,
    }
}

fn preparation_sources_are_current(blockers: &[ResultBlocker]) -> bool {
    !blockers.iter().any(|item| {
        matches!(
            item.code,
            ResultBlockerCode::MissingPhaseConfirmation
                | ResultBlockerCode::StalePhaseConfirmation
                | ResultBlockerCode::InvalidAssessmentPlan
                | ResultBlockerCode::InvalidGradingPolicy
                | ResultBlockerCode::MissingPrimaryTeacher
        )
    })
}

async fn load_workspace(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    scope: &CourseScope,
) -> Result<(CoursePreparationWorkspace, Vec<PhaseRevision>), AppError> {
    let policy = match (&scope.locked_policy_id, &scope.locked_policy_snapshot) {
        (Some(id), Some(snapshot)) if snapshot.0.id == *id => snapshot.0.clone(),
        (Some(_), Some(_)) => {
            return Err(AppError::ValidationError(
                "Locked course policy snapshot does not match its version".into(),
            ));
        }
        (Some(_), None) => {
            return Err(AppError::ValidationError(
                "Locked course policy snapshot is missing".into(),
            ));
        }
        (None, _) => policies::active_policy(tx).await?,
    };
    let phases: Vec<PhaseRow> = sqlx::query_as(
        r#"SELECT phase.id,phase.phase_code,phase.max_score::text AS max_score,phase.row_version,
                  confirmation.id AS confirmation_id,confirmation.row_version AS confirmation_row_version,
                  confirmation.source_checksum AS confirmation_source_checksum,
                  confirmation.roster_checksum AS confirmation_roster_checksum,
                  COALESCE((confirmation.source_snapshot->>'invalidated')::boolean,false) AS confirmation_invalidated
           FROM course_assessment_plans plan
           JOIN course_assessment_phases phase ON phase.plan_id=plan.id
           LEFT JOIN learning_group_phase_confirmations confirmation
             ON confirmation.learning_group_id=$1 AND confirmation.assessment_phase_id=phase.id
           WHERE plan.learning_offering_id=$2
           ORDER BY CASE phase.phase_code WHEN 'before_midterm' THEN 1 WHEN 'midterm' THEN 2 WHEN 'after_midterm' THEN 3 ELSE 4 END,phase.id"#,
    )
    .bind(scope.group_id)
    .bind(scope.offering_id)
    .fetch_all(&mut **tx)
    .await?;
    let items: Vec<ItemRow> = sqlx::query_as(
        "SELECT assessment_phase_id,id,name,max_score::text AS max_score,display_order,lifecycle,row_version FROM learning_group_score_items WHERE learning_group_id=$1 ORDER BY assessment_phase_id,display_order,id LIMIT 4001",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;
    let students: Vec<GradebookStudent> = sqlx::query_as(
        "SELECT m.id AS membership_id,m.student_academic_year_id,concat_ws(' ',u.first_name,u.last_name) AS display_name,m.row_version FROM learning_group_students m JOIN users u ON u.id=m.student_id WHERE m.learning_group_id=$1 AND m.membership_status='active' ORDER BY m.student_academic_year_id LIMIT 2001",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;
    let scores: Vec<ScoreRow> = sqlx::query_as(
        "SELECT item.assessment_phase_id,score.score_item_id,score.student_academic_year_id,score.score::text AS value,score.row_version FROM learning_group_student_scores score JOIN learning_group_score_items item ON item.id=score.score_item_id WHERE score.learning_group_id=$1 ORDER BY item.assessment_phase_id,score.score_item_id,score.student_academic_year_id LIMIT 200001",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;
    if items.len() > 4000 || students.len() > 2000 || scores.len() > 200000 {
        return Err(AppError::ValidationError(
            "Course preparation workspace exceeds supported size".into(),
        ));
    }
    let overrides: Vec<OverrideRow> = sqlx::query_as(
        "SELECT student_academic_year_id,outcome,row_version FROM learning_group_result_overrides WHERE learning_group_id=$1 ORDER BY student_academic_year_id",
    )
    .bind(scope.group_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut blockers = Vec::new();
    let canonical = BTreeSet::from(["before_midterm", "midterm", "after_midterm", "final"]);
    let actual = phases
        .iter()
        .map(|phase| phase.phase_code.as_str())
        .collect::<BTreeSet<_>>();
    let phase_total = phases
        .iter()
        .try_fold(BigDecimal::from(0), |total, phase| {
            decimal(&phase.max_score).map(|maximum| total + maximum)
        })?;
    if phases.len() != 4
        || actual != canonical
        || phase_total != decimal(&scope.assessment_total_score)?
    {
        blockers.push(blocker(
            ResultBlockerCode::InvalidAssessmentPlan,
            None,
            None,
        ));
    }
    let policy_is_valid = validate_policy(&policy.bands, &scope.assessment_total_score).is_ok();
    if !policy_is_valid {
        blockers.push(blocker(ResultBlockerCode::InvalidGradingPolicy, None, None));
    }
    if scope.primary_teacher_id.is_none() {
        blockers.push(blocker(
            ResultBlockerCode::MissingPrimaryTeacher,
            None,
            None,
        ));
    }
    if scope.locked {
        blockers.push(blocker(ResultBlockerCode::AlreadyLocked, None, None));
    }

    let mut phase_revisions = Vec::with_capacity(phases.len());
    let mut roster_checksum = None;
    for phase in &phases {
        let phase_items = items
            .iter()
            .filter(|item| item.assessment_phase_id == phase.id)
            .map(|item| ScoreItem {
                id: item.id,
                name: item.name.clone(),
                max_score: item.max_score.clone(),
                display_order: item.display_order,
                lifecycle: item.lifecycle.clone(),
                row_version: item.row_version,
            })
            .collect::<Vec<_>>();
        let phase_scores = scores
            .iter()
            .filter(|score| score.assessment_phase_id == phase.id)
            .map(|score| ScoreCell {
                score_item_id: score.score_item_id,
                student_academic_year_id: score.student_academic_year_id,
                value: score.value.clone(),
                row_version: score.row_version,
            })
            .collect::<Vec<_>>();
        let (current_roster, current_source) =
            crate::modules::academic::gradebook::services::source_checksums(
                scope.group_id,
                phase.id,
                phase.row_version,
                &phase.max_score,
                &phase_items,
                &students,
                &phase_scores,
            )?;
        if roster_checksum.is_none() {
            roster_checksum = Some(current_roster.clone());
        }
        let current = phase.confirmation_id.is_some()
            && phase.confirmation_invalidated != Some(true)
            && phase.confirmation_source_checksum.as_deref() == Some(current_source.as_str())
            && phase.confirmation_roster_checksum.as_deref() == Some(current_roster.as_str());
        if phase.confirmation_id.is_none() {
            blockers.push(blocker(
                ResultBlockerCode::MissingPhaseConfirmation,
                Some(phase.id),
                None,
            ));
        } else if !current {
            blockers.push(blocker(
                ResultBlockerCode::StalePhaseConfirmation,
                Some(phase.id),
                None,
            ));
        }
        phase_revisions.push(PhaseRevision {
            assessment_phase_id: phase.id,
            phase_code: phase.phase_code.clone(),
            confirmation_id: phase.confirmation_id,
            row_version: phase.confirmation_row_version,
            source_checksum: current_source,
            roster_checksum: current_roster,
            current,
        });
    }
    let roster_checksum = match roster_checksum {
        Some(checksum) => checksum,
        None => hash(
            &students
                .iter()
                .map(|student| {
                    (
                        student.student_academic_year_id,
                        student.membership_id,
                        student.row_version,
                    )
                })
                .collect::<Vec<_>>(),
        )?,
    };

    let active_items = items
        .iter()
        .filter(|item| item.lifecycle == "active")
        .map(|item| item.id)
        .collect::<BTreeSet<_>>();
    let active_students = students
        .iter()
        .map(|student| student.student_academic_year_id)
        .collect::<BTreeSet<_>>();
    let mut totals: BTreeMap<Uuid, BigDecimal> = students
        .iter()
        .map(|student| (student.student_academic_year_id, BigDecimal::from(0)))
        .collect();
    for score in &scores {
        if active_items.contains(&score.score_item_id)
            && active_students.contains(&score.student_academic_year_id)
        {
            let value = score
                .value
                .as_deref()
                .ok_or_else(|| AppError::ValidationError("Stored score is invalid".into()))?;
            let total = totals
                .get_mut(&score.student_academic_year_id)
                .ok_or_else(|| AppError::InternalServerError("Result roster changed".into()))?;
            *total += decimal(value)?;
        }
    }
    let override_map = overrides
        .iter()
        .map(|value| (value.student_academic_year_id, value))
        .collect::<BTreeMap<_, _>>();
    let mut prepared = Vec::with_capacity(students.len());
    for student in &students {
        let score = totals
            .get(&student.student_academic_year_id)
            .cloned()
            .unwrap_or_else(|| BigDecimal::from(0));
        let calculated_grade = if policy_is_valid {
            Some(derive_grade(
                &policy.bands,
                &decimal_wire(&score),
                &scope.assessment_total_score,
            )?)
        } else {
            None
        };
        let selected = override_map.get(&student.student_academic_year_id);
        let selection = selected
            .map(|value| CourseOutcomeSelection::try_from(value.outcome.as_str()))
            .transpose()
            .map_err(|_| AppError::ValidationError("Stored course selection is invalid".into()))?
            .unwrap_or(CourseOutcomeSelection::Derived);
        let numeric_grade = match selection {
            CourseOutcomeSelection::Derived => calculated_grade.clone(),
            CourseOutcomeSelection::ExplicitZero => Some("0.00".into()),
            CourseOutcomeSelection::Incomplete | CourseOutcomeSelection::InsufficientAttendance => {
                None
            }
        };
        prepared.push(PreparedCourseStudent {
            student_academic_year_id: student.student_academic_year_id,
            display_name: student.display_name.clone(),
            calculated_score: decimal_wire(&score),
            calculated_grade,
            selection,
            selection_row_version: selected.map(|value| value.row_version),
            numeric_grade,
        });
    }

    let selection_revisions = prepared
        .iter()
        .map(|student| SelectionRevision {
            student_academic_year_id: student.student_academic_year_id,
            selection: student.selection,
            row_version: student.selection_row_version,
        })
        .collect::<Vec<_>>();
    let source_checksum = hash(&(
        scope.group_id,
        scope.subject_id,
        &roster_checksum,
        policy.id,
        &policy.bands,
        &phase_revisions,
        &selection_revisions,
    ))?;
    let confirmation: Option<ResultConfirmation> = sqlx::query_as(
        "SELECT id,row_version,source_checksum,roster_checksum,confirmed_by,COALESCE((source_snapshot->>'invalidated')::boolean,false) AS invalidated FROM learning_group_result_confirmations WHERE learning_group_id=$1",
    )
    .bind(scope.group_id)
    .fetch_optional(&mut **tx)
    .await?;
    let confirmation_is_current = confirmation.as_ref().is_some_and(|saved| {
        !saved.invalidated
            && saved.source_checksum == source_checksum
            && saved.roster_checksum == roster_checksum
            && Some(saved.confirmed_by) == scope.primary_teacher_id
            && blockers.iter().all(|item| {
                !matches!(
                    item.code,
                    ResultBlockerCode::MissingPhaseConfirmation
                        | ResultBlockerCode::StalePhaseConfirmation
                        | ResultBlockerCode::InvalidAssessmentPlan
                        | ResultBlockerCode::InvalidGradingPolicy
                        | ResultBlockerCode::MissingPrimaryTeacher
                )
            })
    });
    let sources_are_current = preparation_sources_are_current(&blockers);
    let can_prepare = !scope.locked
        && access_policy::can_confirm_course(
            actor,
            scope.primary_teacher_id == Some(actor.user_id),
        );
    Ok((
        CoursePreparationWorkspace {
            learning_group_id: scope.group_id,
            subject_id: scope.subject_id,
            policy,
            students: prepared,
            source_checksum,
            roster_checksum,
            confirmation,
            confirmation_is_current,
            blockers,
            locked: scope.locked,
            can_manage: can_prepare && sources_are_current,
            can_confirm: can_prepare && sources_are_current,
        },
        phase_revisions,
    ))
}

pub(super) async fn workspace_for_lock(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
) -> Result<CoursePreparationWorkspace, AppError> {
    let scope = load_scope(tx, actor, group, context).await?;
    let (workspace, _) = load_workspace(tx, actor, &scope).await?;
    Ok(workspace)
}

async fn invalidate_confirmation(
    tx: &mut Transaction<'_, Postgres>,
    group: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE learning_group_result_confirmations SET row_version=row_version+1,source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true'::jsonb) WHERE learning_group_id=$1",
    )
    .bind(group)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn get_course_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
) -> Result<CoursePreparationWorkspace, AppError> {
    let (mut tx, scope) = begin_course(pool, actor, group, context, false).await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub async fn save_selection(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
    input: SelectionInput,
) -> Result<CoursePreparationWorkspace, AppError> {
    let (mut tx, scope) = begin_course(pool, actor, group, context, true).await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    if !preparation_sources_are_current(&workspace.blockers) {
        tx.commit().await?;
        return Err(AppError::Conflict(
            "All four current Gradebook phase confirmations and a valid grading policy are required before selecting a course outcome".into(),
        ));
    }
    let student = workspace
        .students
        .iter()
        .find(|student| student.student_academic_year_id == input.student_academic_year_id)
        .ok_or_else(|| {
            AppError::ValidationError("Student is not on the current group roster".into())
        })?;
    check_version(input.row_version, student.selection_row_version)?;
    if input.selection == CourseOutcomeSelection::Derived && student.selection_row_version.is_none()
    {
        tx.commit().await?;
        return Ok(workspace);
    }
    let revision: i64 = sqlx::query_scalar(
        "UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=$1 RETURNING row_version",
    )
    .bind(group)
    .fetch_one(&mut *tx)
    .await?;
    if input.selection == CourseOutcomeSelection::Derived {
        sqlx::query("DELETE FROM learning_group_result_overrides WHERE learning_group_id=$1 AND student_academic_year_id=$2")
            .bind(group)
            .bind(input.student_academic_year_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "INSERT INTO learning_group_result_overrides (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,student_academic_year_id,outcome,row_version,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (learning_group_id,student_academic_year_id) DO UPDATE SET outcome=EXCLUDED.outcome,row_version=EXCLUDED.row_version,updated_by=EXCLUDED.updated_by,updated_at=now()",
        )
        .bind(group)
        .bind(scope.offering_id)
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .bind(scope.subject_id)
        .bind(input.student_academic_year_id)
        .bind(input.selection.as_str())
        .bind(revision)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
    }
    invalidate_confirmation(&mut tx, group).await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub async fn confirm_group_results(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    group: Uuid,
    input: ResultConfirmationInput,
) -> Result<CoursePreparationWorkspace, AppError> {
    let (mut tx, scope) = begin_course(pool, actor, group, context, true).await?;
    let (workspace, phases) = load_workspace(&mut tx, actor, &scope).await?;
    if input.source_checksum != workspace.source_checksum
        || input.roster_checksum != workspace.roster_checksum
    {
        return Err(conflict());
    }
    check_version(
        input.row_version,
        workspace
            .confirmation
            .as_ref()
            .map(|saved| saved.row_version),
    )?;
    if workspace
        .blockers
        .iter()
        .any(|item| !matches!(item.code, ResultBlockerCode::MissingGroupConfirmation))
    {
        tx.commit().await?;
        return Ok(workspace);
    }
    let snapshot = CourseConfirmationSnapshot {
        invalidated: false,
        learning_group_id: group,
        learning_offering_id: scope.offering_id,
        source_checksum: &workspace.source_checksum,
        roster_checksum: &workspace.roster_checksum,
        policy: &workspace.policy,
        phases: &phases,
        roster: sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "SELECT id,student_academic_year_id,row_version FROM learning_group_students WHERE learning_group_id=$1 AND membership_status='active' ORDER BY student_academic_year_id",
        )
        .bind(group)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(membership_id, student_academic_year_id, row_version)| RosterRevision {
            membership_id,
            student_academic_year_id,
            row_version,
        })
        .collect(),
        selections: workspace
            .students
            .iter()
            .map(|student| SelectionRevision {
                student_academic_year_id: student.student_academic_year_id,
                selection: student.selection,
                row_version: student.selection_row_version,
            })
            .collect(),
    };
    sqlx::query(
        "INSERT INTO learning_group_result_confirmations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,policy_version_id,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (learning_group_id) DO UPDATE SET policy_version_id=EXCLUDED.policy_version_id,roster_checksum=EXCLUDED.roster_checksum,source_checksum=EXCLUDED.source_checksum,source_snapshot=EXCLUDED.source_snapshot,confirmed_by=EXCLUDED.confirmed_by,confirmed_at=now(),row_version=learning_group_result_confirmations.row_version+1",
    )
    .bind(group)
    .bind(scope.offering_id)
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .bind(scope.subject_id)
    .bind(workspace.policy.id)
    .bind(&workspace.roster_checksum)
    .bind(&workspace.source_checksum)
    .bind(sqlx::types::Json(snapshot))
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await?;
    let (workspace, _) = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_snapshots_do_not_include_display_names() {
        let value = serde_json::to_value(RosterRevision {
            membership_id: Uuid::new_v4(),
            student_academic_year_id: Uuid::new_v4(),
            row_version: 1,
        })
        .unwrap();
        assert_eq!(value.as_object().unwrap().len(), 3);
    }
}
