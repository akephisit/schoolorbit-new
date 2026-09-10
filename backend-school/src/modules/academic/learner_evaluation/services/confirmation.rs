use super::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RosterRevision {
    pub membership_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub row_version: i64,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GroupSnapshot {
    pub invalidated: bool,
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub criteria: Vec<EvaluationCriterion>,
    pub roster: Vec<RosterRevision>,
    pub responses: Vec<EvaluationResponse>,
    pub confirmation: Option<EvaluationConfirmation>,
}
pub(super) fn snapshot(ws: &EvaluationWorkspace, offering: Uuid) -> GroupSnapshot {
    let criteria = ws
        .criteria
        .iter()
        .filter(|c| c.lifecycle == "active")
        .cloned()
        .collect::<Vec<_>>();
    let responses = ws
        .responses
        .iter()
        .filter(|r| {
            criteria.iter().any(|c| c.id == r.subject_term_criterion_id)
                && ws
                    .students
                    .iter()
                    .any(|s| s.student_academic_year_id == r.student_academic_year_id)
        })
        .cloned()
        .collect();
    GroupSnapshot {
        invalidated: false,
        learning_group_id: ws.learning_group_id,
        learning_offering_id: offering,
        source_checksum: ws.source_checksum.clone(),
        roster_checksum: ws.roster_checksum.clone(),
        criteria,
        roster: ws
            .students
            .iter()
            .map(|s| RosterRevision {
                membership_id: s.membership_id,
                student_academic_year_id: s.student_academic_year_id,
                row_version: s.row_version,
            })
            .collect(),
        responses,
        confirmation: ws.confirmation.clone(),
    }
}
pub(super) fn missing(ws: &EvaluationWorkspace) -> Vec<MissingEvaluation> {
    ws.students
        .iter()
        .flat_map(|s| {
            ws.criteria
                .iter()
                .filter(|c| {
                    c.lifecycle == "active"
                        && !ws.responses.iter().any(|r| {
                            r.student_academic_year_id == s.student_academic_year_id
                                && r.subject_term_criterion_id == c.id
                        })
                })
                .map(move |c| MissingEvaluation {
                    student_academic_year_id: s.student_academic_year_id,
                    subject_term_criterion_id: c.id,
                })
        })
        .collect()
}
pub async fn confirm_group(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    input: ConfirmationInput,
) -> Result<ConfirmationOutcome, AppError> {
    let subject = subject_for_group(pool, group, ctx).await?;
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx, true).await?;
    let group_scope = scope
        .groups
        .iter()
        .find(|g| g.group_id == group)
        .ok_or_else(|| AppError::NotFound("Active group not found".into()))?;
    require_edit(
        actor,
        &scope,
        policy::can_confirm(actor, group_scope.primary_teacher_id == Some(actor.user_id)),
    )?;
    let ws = entry::load_workspace(&mut tx, actor, &scope, group_scope, ctx).await?;
    if input.source_checksum != ws.source_checksum || input.roster_checksum != ws.roster_checksum {
        return Err(conflict());
    }
    check_version(
        input.row_version,
        ws.confirmation.as_ref().map(|c| c.row_version),
    )?;
    let missing = missing(&ws);
    if !missing.is_empty() {
        tx.commit().await?;
        return Ok(ConfirmationOutcome {
            confirmation: None,
            missing,
        });
    }
    if !ws.criteria.iter().any(|c| c.lifecycle == "active") {
        return Err(AppError::ValidationError(
            "At least one active evaluation criterion is required".into(),
        ));
    }
    let snapshot = sqlx::types::Json(snapshot(&ws, group_scope.offering_id));
    let confirmation=sqlx::query_as("INSERT INTO learning_group_evaluation_confirmations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (learning_group_id,domain) DO UPDATE SET roster_checksum=EXCLUDED.roster_checksum,source_checksum=EXCLUDED.source_checksum,source_snapshot=EXCLUDED.source_snapshot,confirmed_by=EXCLUDED.confirmed_by,confirmed_at=now(),row_version=learning_group_evaluation_confirmations.row_version+1 RETURNING id,source_checksum,roster_checksum,row_version,false AS invalidated,confirmed_by").bind(group).bind(group_scope.offering_id).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(subject).bind(domain.as_str()).bind(&ws.roster_checksum).bind(&ws.source_checksum).bind(snapshot).bind(actor.user_id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(ConfirmationOutcome {
        confirmation: Some(confirmation),
        missing,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roster_snapshot_has_no_display_names() {
        let row = RosterRevision {
            membership_id: Uuid::new_v4(),
            student_academic_year_id: Uuid::new_v4(),
            row_version: 1,
        };
        let value = serde_json::to_value(row).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 3);
    }
}
