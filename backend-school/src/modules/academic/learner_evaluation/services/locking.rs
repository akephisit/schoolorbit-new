use super::*;
use std::collections::BTreeSet;

#[derive(sqlx::FromRow)]
struct LockReadinessRow {
    subject_id: Uuid,
    code: String,
    name: String,
    domain: LearnerEvaluationDomain,
    learning_group_id: Uuid,
    group_name: String,
    locked: bool,
    blocker_reason: Option<String>,
    student_in_multiple_subject_groups: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LockSnapshot {
    criteria: Vec<EvaluationCriterion>,
    groups: Vec<confirmation::GroupSnapshot>,
}

pub async fn read_lock_readiness(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
) -> Result<Vec<LearnerEvaluationSubjectLockReadiness>, AppError> {
    if !policy::can_lock(actor) {
        return Err(AppError::Forbidden(
            "School learner evaluation lock permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, ctx).await?;
    let rows: Vec<LockReadinessRow> = sqlx::query_as(
        r#"WITH domains(domain) AS (
               VALUES ('desirable_characteristic'::text), ('reading_thinking_writing'::text)
           ), subject_groups AS (
               SELECT detail.subject_id, offering.code_snapshot AS code,
                      offering.name_snapshot AS name, group_row.id AS learning_group_id,
                      group_row.name AS group_name, group_row.academic_term_id,
                      group_row.academic_year_id
               FROM learning_groups group_row
               JOIN learning_offerings offering ON offering.id=group_row.learning_offering_id
                                                AND offering.kind='course'
               JOIN course_offering_details detail
                 ON detail.learning_offering_id=offering.id
               WHERE group_row.academic_term_id=$1 AND group_row.academic_year_id=$2
                 AND group_row.status<>'closed'
           )
           SELECT subject_groups.subject_id,subject_groups.code,subject_groups.name,
                  domains.domain,subject_groups.learning_group_id,subject_groups.group_name,
                  evaluation_lock.id IS NOT NULL AS locked,
                  CASE
                    WHEN evaluation_lock.id IS NOT NULL THEN NULL
                    WHEN NOT EXISTS (
                        SELECT 1 FROM subject_term_evaluation_criteria criterion
                        WHERE criterion.subject_id=subject_groups.subject_id
                          AND criterion.academic_term_id=$1 AND criterion.domain=domains.domain
                          AND criterion.lifecycle='active'
                    ) THEN 'no_active_criteria'
                    WHEN EXISTS (
                        SELECT 1
                        FROM learning_group_students member
                        CROSS JOIN subject_term_evaluation_criteria criterion
                        WHERE member.learning_group_id=subject_groups.learning_group_id
                          AND member.membership_status='active'
                          AND criterion.subject_id=subject_groups.subject_id
                          AND criterion.academic_term_id=$1 AND criterion.domain=domains.domain
                          AND criterion.lifecycle='active'
                          AND NOT EXISTS (
                              SELECT 1 FROM learning_group_student_evaluations response
                              WHERE response.learning_group_id=subject_groups.learning_group_id
                                AND response.student_academic_year_id=member.student_academic_year_id
                                AND response.subject_term_criterion_id=criterion.id
                          )
                    ) THEN 'missing_responses'
                    WHEN confirmation.id IS NULL THEN 'group_not_confirmed'
                    WHEN NOT (
                        NOT COALESCE((confirmation.source_snapshot->>'invalidated')::boolean,false)
                        AND COALESCE(confirmation.confirmed_by=(
                            SELECT teacher.teacher_id
                            FROM learning_group_teachers teacher
                            JOIN users account ON account.id=teacher.teacher_id
                                              AND account.status='active'
                            JOIN academic_terms term
                              ON term.id=teacher.academic_term_id
                            WHERE teacher.learning_group_id=subject_groups.learning_group_id
                              AND teacher.role='primary'
                              AND teacher.starts_on<=LEAST(
                                  GREATEST(current_date,term.start_date),term.planned_end_date
                              )
                              AND (teacher.ends_on IS NULL OR teacher.ends_on>=LEAST(
                                  GREATEST(current_date,term.start_date),term.planned_end_date
                              ))
                            ORDER BY teacher.id LIMIT 1
                        ),false)
                        AND confirmation.source_snapshot->'roster'=COALESCE((
                            SELECT jsonb_agg(jsonb_build_object(
                                'membershipId',member.id,
                                'studentAcademicYearId',member.student_academic_year_id,
                                'rowVersion',member.row_version
                            ) ORDER BY member.student_academic_year_id)
                            FROM learning_group_students member
                            WHERE member.learning_group_id=subject_groups.learning_group_id
                              AND member.membership_status='active'
                        ),'[]'::jsonb)
                    ) THEN 'stale_group_confirmation'
                    ELSE NULL
                  END AS blocker_reason,
                  evaluation_lock.id IS NULL AND EXISTS (
                      SELECT 1
                      FROM learning_group_students member
                      JOIN learning_group_students earlier_member
                        ON earlier_member.student_academic_year_id=member.student_academic_year_id
                       AND earlier_member.membership_status='active'
                       AND earlier_member.learning_group_id<subject_groups.learning_group_id
                      JOIN learning_groups earlier_group
                        ON earlier_group.id=earlier_member.learning_group_id
                       AND earlier_group.academic_term_id=$1
                       AND earlier_group.academic_year_id=$2
                       AND earlier_group.status<>'closed'
                      JOIN course_offering_details earlier_detail
                        ON earlier_detail.learning_offering_id=earlier_group.learning_offering_id
                       AND earlier_detail.subject_id=subject_groups.subject_id
                      WHERE member.learning_group_id=subject_groups.learning_group_id
                        AND member.membership_status='active'
                  ) AS student_in_multiple_subject_groups
           FROM subject_groups CROSS JOIN domains
           LEFT JOIN learning_group_evaluation_confirmations confirmation
             ON confirmation.learning_group_id=subject_groups.learning_group_id
            AND confirmation.domain=domains.domain
           LEFT JOIN subject_term_evaluation_locks evaluation_lock
             ON evaluation_lock.subject_id=subject_groups.subject_id
            AND evaluation_lock.academic_term_id=$1
            AND evaluation_lock.academic_year_id=$2
            AND evaluation_lock.domain=domains.domain
           ORDER BY subject_groups.subject_id,domains.domain,
                    subject_groups.learning_group_id
           LIMIT 5001"#,
    )
    .bind(ctx.academic_term_id)
    .bind(ctx.academic_year_id)
    .fetch_all(&mut *tx)
    .await?;
    if rows.len() > 5000 {
        return Err(AppError::ValidationError(
            "Learner evaluation lock readiness exceeds 5000 groups".into(),
        ));
    }

    let mut readiness: Vec<LearnerEvaluationSubjectLockReadiness> = Vec::new();
    for row in rows {
        if !readiness.last().is_some_and(|subject| {
            subject.subject_id == row.subject_id && subject.domain == row.domain
        }) {
            readiness.push(LearnerEvaluationSubjectLockReadiness {
                subject_id: row.subject_id,
                code: row.code.clone(),
                name: row.name.clone(),
                domain: row.domain,
                locked: row.locked,
                ready: true,
                groups: Vec::new(),
            });
        }
        let subject = readiness.last_mut().ok_or_else(|| {
            AppError::InternalServerError("Could not group learner evaluation readiness".into())
        })?;
        let mut blockers = row.blocker_reason.into_iter().collect::<Vec<_>>();
        if row.student_in_multiple_subject_groups {
            blockers.push("student_in_multiple_subject_groups".into());
        }
        let group_ready = row.locked || blockers.is_empty();
        subject.ready &= group_ready;
        subject.groups.push(LearnerEvaluationGroupLockReadiness {
            learning_group_id: row.learning_group_id,
            group_name: row.group_name,
            ready: group_ready,
            blockers,
        });
    }
    readiness.sort_by(|left, right| {
        (&left.code, &left.name, left.domain, left.subject_id).cmp(&(
            &right.code,
            &right.name,
            right.domain,
            right.subject_id,
        ))
    });
    tx.commit().await?;
    Ok(readiness)
}

pub async fn lock_subject(
    pool: &PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
) -> Result<LockOutcome, AppError> {
    if !policy::can_lock(actor) {
        return Err(AppError::Forbidden(
            "School learner evaluation lock permission is required".into(),
        ));
    }
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx).await?;
    if scope.locked {
        return Err(AppError::Conflict(
            "Subject evaluation domain is already locked".into(),
        ));
    }
    let mut snapshots = vec![];
    let mut blockers = vec![];
    let mut students = BTreeSet::new();
    for group in &scope.groups {
        let ws = entry::load_workspace(&mut tx, actor, &scope, group, ctx).await?;
        if let Some(reason) = blocker(&ws) {
            blockers.push(LockBlocker {
                learning_group_id: group.group_id,
                reason: reason.into(),
            });
        }
        for student in &ws.students {
            if !students.insert(student.student_academic_year_id) {
                blockers.push(LockBlocker {
                    learning_group_id: group.group_id,
                    reason: "student_in_multiple_subject_groups".into(),
                });
            }
        }
        snapshots.push(confirmation::snapshot(&ws, group.offering_id));
    }
    if !blockers.is_empty() {
        tx.commit().await?;
        return Ok(LockOutcome {
            lock: None,
            blockers,
        });
    }
    let criteria = configuration::criteria(&mut tx, &scope, ctx)
        .await?
        .into_iter()
        .filter(|c| c.lifecycle == "active")
        .collect();
    let roster_checksum = entry::hash(
        &snapshots
            .iter()
            .map(|s| (s.learning_group_id, &s.roster_checksum))
            .collect::<Vec<_>>(),
    )?;
    let source_checksum = entry::hash(
        &snapshots
            .iter()
            .map(|s| {
                (
                    s.learning_group_id,
                    &s.source_checksum,
                    s.confirmation.as_ref().map(|c| (c.id, c.row_version)),
                )
            })
            .collect::<Vec<_>>(),
    )?;
    let snapshot = LockSnapshot {
        criteria,
        groups: snapshots,
    };
    let lock:EvaluationLock=sqlx::query_as("INSERT INTO subject_term_evaluation_locks (subject_id,academic_term_id,academic_year_id,domain,roster_checksum,source_checksum,source_snapshot,locked_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id,subject_id,domain,source_checksum,roster_checksum,row_version").bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).bind(roster_checksum).bind(source_checksum).bind(sqlx::types::Json(&snapshot)).bind(actor.user_id).fetch_one(&mut *tx).await?;
    // Snapshot inputs were fully validated above; one statement captures every room.
    sqlx::query("INSERT INTO subject_term_student_evaluations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,subject_term_criterion_id,evaluation_lock_id,student_academic_year_id,quality_level) SELECT r.learning_group_id,r.learning_offering_id,r.academic_term_id,r.academic_year_id,r.subject_id,r.domain,r.subject_term_criterion_id,$4,r.student_academic_year_id,r.quality_level FROM learning_group_student_evaluations r JOIN subject_term_evaluation_criteria c ON c.id=r.subject_term_criterion_id AND c.lifecycle='active' JOIN learning_group_students m ON m.learning_group_id=r.learning_group_id AND m.student_academic_year_id=r.student_academic_year_id AND m.membership_status='active' WHERE r.subject_id=$1 AND r.academic_term_id=$2 AND r.domain=$3 AND r.learning_group_id=ANY($5)").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).bind(lock.id).bind(scope.groups.iter().map(|g|g.group_id).collect::<Vec<_>>()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(LockOutcome {
        lock: Some(lock),
        blockers,
    })
}
fn blocker(ws: &EvaluationWorkspace) -> Option<&'static str> {
    if !ws.criteria.iter().any(|c| c.lifecycle == "active") {
        Some("no_active_criteria")
    } else if !confirmation::missing(ws).is_empty() {
        Some("missing_responses")
    } else if ws.confirmation.is_none() {
        Some("group_not_confirmed")
    } else if !ws.confirmation_is_current {
        Some("stale_group_confirmation")
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_permission_is_not_school_management() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![
                crate::permissions::registry::codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL
                    .into(),
            ],
        };
        assert!(!policy::can_lock(&actor));
    }
}
