use super::*;
use school_database::DbSession;

pub(super) async fn initialize(
    tx: &mut Transaction<'_, Postgres>,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    offerings: &[Uuid],
) -> Result<(), AppError> {
    let inserted=DbSession::new(&mut **tx).query("academic_assessment.learner_evaluation.services.configuration.initialize", "INSERT INTO subject_term_evaluation_configurations (subject_id,academic_term_id,academic_year_id,domain) VALUES ($1,$2,$3,$4) ON CONFLICT (subject_id,academic_term_id,domain) DO NOTHING").bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).execute().await?.rows_affected()>0;
    if inserted {
        DbSession::new(&mut **tx).query("academic_assessment.learner_evaluation.services.configuration.initialize", r#"INSERT INTO subject_term_evaluation_criteria (subject_id,academic_term_id,academic_year_id,domain,school_criterion_id,name,display_order)
            SELECT $1,$2,$3,c.domain,c.id,c.name,c.display_order FROM academic_learner_evaluation_criteria c
            WHERE c.domain=$4 AND c.lifecycle='active' AND (c.applicability='all' OR EXISTS(
                SELECT 1 FROM learning_offering_targets target JOIN grade_levels level ON level.id=target.grade_level_id
                WHERE target.learning_offering_id=ANY($5) AND level.level_type=c.applicability)) ORDER BY c.id"#).bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).bind(offerings).execute().await?;
    }
    Ok(())
}
pub(super) async fn criteria(
    tx: &mut Transaction<'_, Postgres>,
    scope: &SubjectScope,
    ctx: &EvaluationContext,
) -> Result<Vec<EvaluationCriterion>, AppError> {
    let rows:Vec<EvaluationCriterion>=DbSession::new(&mut **tx).query_as("academic_assessment.learner_evaluation.services.configuration.criteria", "SELECT id,school_criterion_id,name,lifecycle,display_order,row_version FROM subject_term_evaluation_criteria WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3 ORDER BY display_order,id LIMIT 1001").bind(scope.subject_id).bind(ctx.academic_term_id).bind(scope.domain.as_str()).fetch_all().await?;
    if rows.len() > 1000 {
        return Err(AppError::ValidationError(
            "Configuration exceeds 1000 criteria".into(),
        ));
    }
    Ok(rows)
}
pub async fn get_configuration(
    pool: &PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
) -> Result<EvaluationConfiguration, AppError> {
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx, false).await?;
    let criteria = criteria(&mut tx, &scope, ctx).await?;
    let row_version=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.configuration.get_configuration", "SELECT row_version FROM subject_term_evaluation_configurations WHERE subject_id=$1 AND academic_term_id=$2 AND domain=$3").bind(subject).bind(ctx.academic_term_id).bind(domain.as_str()).fetch_optional().await?.ok_or_else(|| AppError::NotFound("ยังไม่ได้ตั้งหัวข้อประเมินของรายวิชานี้ในภาคเรียนที่ปิดแล้ว".into()))?;
    let can_manage = scope.academic_state.is_writable()
        && !scope.locked
        && policy::can_manage_configuration(actor, scope.coordinator);
    tx.commit().await?;
    Ok(EvaluationConfiguration {
        subject_id: subject,
        domain,
        row_version,
        locked: scope.locked,
        can_manage,
        criteria,
    })
}
pub(super) fn validate_criterion(input: &CriterionInput) -> Result<(), AppError> {
    if input.name.trim().is_empty() || input.name.chars().count() > 500 {
        return Err(AppError::ValidationError(
            "Criterion name must contain 1–500 characters".into(),
        ));
    }
    Ok(())
}
pub async fn save_criterion(
    pool: &PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    id: Option<Uuid>,
    input: CriterionInput,
) -> Result<EvaluationCriterion, AppError> {
    validate_criterion(&input)?;
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx, true).await?;
    require_unlocked(
        &scope,
        policy::can_manage_configuration(actor, scope.coordinator),
    )?;
    let rows = criteria(&mut tx, &scope, ctx).await?;
    let current =
        if let Some(id) = id {
            Some(rows.iter().find(|c| c.id == id).ok_or_else(|| {
                AppError::NotFound("Criterion not found in subject domain".into())
            })?)
        } else {
            None
        };
    check_version(input.row_version, current.map(|c| c.row_version))?;
    if current.is_none() && rows.len() >= 1000 {
        return Err(AppError::ValidationError(
            "Configuration exceeds 1000 criteria".into(),
        ));
    }
    let active_set_changed =
        current.map_or(input.active, |c| (c.lifecycle == "active") != input.active);
    let revision = next_revision(&mut tx, &scope, ctx).await?;
    let lifecycle = if input.active { "active" } else { "inactive" };
    let row = if let Some(id) = id {
        DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.configuration.save_criterion", "UPDATE subject_term_evaluation_criteria SET name=$2,lifecycle=$3,display_order=$4,row_version=$5,updated_at=now() WHERE id=$1 RETURNING id,school_criterion_id,name,lifecycle,display_order,row_version").bind(id).bind(input.name.trim()).bind(lifecycle).bind(input.display_order).bind(revision).fetch_one().await?
    } else {
        DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.configuration.save_criterion", "INSERT INTO subject_term_evaluation_criteria (subject_id,academic_term_id,academic_year_id,domain,name,lifecycle,display_order,row_version) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id,school_criterion_id,name,lifecycle,display_order,row_version").bind(subject).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(domain.as_str()).bind(input.name.trim()).bind(lifecycle).bind(input.display_order).bind(revision).fetch_one().await?
    };
    if active_set_changed {
        invalidate(&mut tx, &scope, ctx, None).await?;
    }
    tx.commit().await?;
    Ok(row)
}
pub async fn remove_criterion(
    pool: &PgPool,
    actor: &ActorContext,
    subject: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    id: Uuid,
    version: i64,
) -> Result<CriterionRemoval, AppError> {
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx, true).await?;
    require_unlocked(
        &scope,
        policy::can_manage_configuration(actor, scope.coordinator),
    )?;
    let rows = criteria(&mut tx, &scope, ctx).await?;
    let current = rows
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::NotFound("Criterion not found in subject domain".into()))?;
    check_version(Some(version), Some(current.row_version))?;
    let used:bool=DbSession::new(&mut *tx).scalar("academic_assessment.learner_evaluation.services.configuration.remove_criterion", "SELECT EXISTS(SELECT 1 FROM learning_group_student_evaluations WHERE subject_term_criterion_id=$1)").bind(id).fetch_one().await?;
    let revision = next_revision(&mut tx, &scope, ctx).await?;
    let criterion = if used {
        Some(DbSession::new(&mut *tx).query_as("academic_assessment.learner_evaluation.services.configuration.remove_criterion", "UPDATE subject_term_evaluation_criteria SET lifecycle='inactive',row_version=$2,updated_at=now() WHERE id=$1 RETURNING id,school_criterion_id,name,lifecycle,display_order,row_version").bind(id).bind(revision).fetch_one().await?)
    } else {
        DbSession::new(&mut *tx)
            .query(
                "academic_assessment.learner_evaluation.services.configuration.remove_criterion",
                "DELETE FROM subject_term_evaluation_criteria WHERE id=$1",
            )
            .bind(id)
            .execute()
            .await?;
        None
    };
    if current.lifecycle == "active" {
        invalidate(&mut tx, &scope, ctx, None).await?;
    }
    tx.commit().await?;
    Ok(CriterionRemoval {
        id,
        deleted: !used,
        criterion,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_names_are_rejected() {
        assert!(validate_criterion(&CriterionInput {
            name: " \n ".into(),
            display_order: 0,
            active: true,
            row_version: None
        })
        .is_err());
    }
}
