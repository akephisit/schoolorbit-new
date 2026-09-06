use super::*;
use std::collections::BTreeSet;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LockSnapshot {
    criteria: Vec<EvaluationCriterion>,
    groups: Vec<confirmation::GroupSnapshot>,
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
