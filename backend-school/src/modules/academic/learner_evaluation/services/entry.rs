use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub async fn get_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
) -> Result<EvaluationWorkspace, AppError> {
    let subject = subject_for_group(pool, group, ctx).await?;
    let access = policy::list_access(pool, actor).await?;
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx).await?;
    let group_scope = scope
        .groups
        .iter()
        .find(|g| g.group_id == group)
        .ok_or_else(|| AppError::NotFound("Active group not found".into()))?;
    if !policy::can_read_group(&access, group_scope.owner, group_scope.assigned) {
        return Err(AppError::Forbidden("Group access denied".into()));
    }
    let ws = load_workspace(&mut tx, actor, &scope, group_scope, ctx).await?;
    tx.commit().await?;
    Ok(ws)
}

pub(super) async fn load_workspace(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    scope: &SubjectScope,
    group: &GroupScope,
    ctx: &EvaluationContext,
) -> Result<EvaluationWorkspace, AppError> {
    let criteria = configuration::criteria(tx, scope, ctx).await?;
    let students:Vec<EvaluationStudent>=sqlx::query_as("SELECT m.id AS membership_id,m.student_academic_year_id,concat_ws(' ',u.first_name,u.last_name) AS display_name,m.row_version FROM learning_group_students m JOIN users u ON u.id=m.student_id WHERE m.learning_group_id=$1 AND m.membership_status='active' ORDER BY m.student_academic_year_id LIMIT 2001").bind(group.group_id).fetch_all(&mut **tx).await?;
    let responses:Vec<EvaluationResponse>=sqlx::query_as("SELECT subject_term_criterion_id,student_academic_year_id,quality_level,row_version FROM learning_group_student_evaluations WHERE learning_group_id=$1 AND domain=$2 ORDER BY subject_term_criterion_id,student_academic_year_id LIMIT 200001").bind(group.group_id).bind(scope.domain.as_str()).fetch_all(&mut **tx).await?;
    if students.len() > 2000 || responses.len() > 200000 {
        return Err(AppError::ValidationError(
            "Learner evaluation workspace exceeds supported size".into(),
        ));
    }
    let (roster_checksum, source_checksum) = checksums(
        group.group_id,
        scope.domain,
        &criteria,
        &students,
        &responses,
    )?;
    let mut confirmation:Option<EvaluationConfirmation>=sqlx::query_as("SELECT id,source_checksum,roster_checksum,row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) AS invalidated,confirmed_by FROM learning_group_evaluation_confirmations WHERE learning_group_id=$1 AND domain=$2").bind(group.group_id).bind(scope.domain.as_str()).fetch_optional(&mut **tx).await?;
    // Roster/teacher mutations belong to Delivery. Detect their retained revisions here,
    // persist explicit staleness once, and never reset the confirmation's row version.
    if let Some(c) = confirmation.as_mut() {
        if !scope.locked
            && !c.invalidated
            && (c.source_checksum != source_checksum
                || c.roster_checksum != roster_checksum
                || Some(c.confirmed_by) != group.primary_teacher_id)
        {
            invalidate(tx, scope, ctx, Some(group.group_id)).await?;
            c.invalidated = true;
            c.row_version += 1;
        }
    }
    let confirmation_is_current = confirmation.as_ref().is_some_and(|c| {
        !c.invalidated
            && c.source_checksum == source_checksum
            && c.roster_checksum == roster_checksum
            && Some(c.confirmed_by) == group.primary_teacher_id
    });
    let editable = !scope.locked && (scope.entry_enabled || policy::can_manage_school(actor));
    Ok(EvaluationWorkspace {
        learning_group_id: group.group_id,
        subject_id: scope.subject_id,
        domain: scope.domain,
        criteria,
        students,
        responses,
        entry_enabled: scope.entry_enabled,
        locked: scope.locked,
        can_manage: editable && policy::can_manage_group(actor, group.assigned),
        can_confirm: editable
            && policy::can_confirm(actor, group.primary_teacher_id == Some(actor.user_id)),
        source_checksum,
        roster_checksum,
        confirmation,
        confirmation_is_current,
    })
}
pub(super) fn hash(value: &impl serde::Serialize) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        AppError::InternalServerError("Could not encode evaluation revision".into())
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub(super) fn checksums(
    group: Uuid,
    domain: LearnerEvaluationDomain,
    criteria: &[EvaluationCriterion],
    students: &[EvaluationStudent],
    responses: &[EvaluationResponse],
) -> Result<(String, String), AppError> {
    let mut roster = students
        .iter()
        .map(|s| (s.student_academic_year_id, s.membership_id, s.row_version))
        .collect::<Vec<_>>();
    roster.sort();
    let mut active = criteria
        .iter()
        .filter(|c| c.lifecycle == "active")
        .map(|c| c.id)
        .collect::<Vec<_>>();
    active.sort();
    let mut values = responses
        .iter()
        .filter(|r| {
            active.contains(&r.subject_term_criterion_id)
                && roster.iter().any(|s| s.0 == r.student_academic_year_id)
        })
        .map(|r| {
            (
                r.subject_term_criterion_id,
                r.student_academic_year_id,
                r.quality_level,
                r.row_version,
            )
        })
        .collect::<Vec<_>>();
    values.sort();
    let roster_checksum = hash(&roster)?;
    let source_checksum = hash(&(group, domain, &roster_checksum, active, values))?;
    Ok((roster_checksum, source_checksum))
}
type NormalizedCells = BTreeMap<(Uuid, Uuid), (Option<i16>, Option<i64>)>;
fn normalize(cells: Vec<ResponseInput>) -> Result<NormalizedCells, AppError> {
    if cells.is_empty() || cells.len() > 500 {
        return Err(AppError::ValidationError(
            "Response batch must contain 1–500 cells".into(),
        ));
    }
    let mut result = BTreeMap::new();
    for cell in cells {
        let key = (
            cell.subject_term_criterion_id,
            cell.student_academic_year_id,
        );
        let value = (cell.quality_level.map(i16::from), cell.row_version);
        if let Some(prior) = result.insert(key, value) {
            if prior != value {
                return Err(AppError::ValidationError(
                    "Conflicting duplicate response cells".into(),
                ));
            }
        }
    }
    Ok(result)
}
pub async fn save_responses(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    domain: LearnerEvaluationDomain,
    ctx: &EvaluationContext,
    cells: Vec<ResponseInput>,
) -> Result<EvaluationWorkspace, AppError> {
    let cells = normalize(cells)?;
    let subject = subject_for_group(pool, group, ctx).await?;
    let (mut tx, scope) = begin_subject(pool, actor, subject, domain, ctx).await?;
    let group_scope = scope
        .groups
        .iter()
        .find(|g| g.group_id == group)
        .ok_or_else(|| AppError::NotFound("Active group not found".into()))?;
    require_edit(
        actor,
        &scope,
        policy::can_manage_group(actor, group_scope.assigned),
    )?;
    let ws = load_workspace(&mut tx, actor, &scope, group_scope, ctx).await?;
    for ((criterion, student), (_, version)) in &cells {
        if !ws
            .criteria
            .iter()
            .any(|c| c.id == *criterion && c.lifecycle == "active")
        {
            return Err(AppError::ValidationError(
                "Criterion is not active in this subject domain".into(),
            ));
        }
        if !ws
            .students
            .iter()
            .any(|s| s.student_academic_year_id == *student)
        {
            return Err(AppError::ValidationError(
                "Student is not on the current group roster".into(),
            ));
        }
        let current = ws
            .responses
            .iter()
            .find(|r| {
                r.subject_term_criterion_id == *criterion && r.student_academic_year_id == *student
            })
            .map(|r| r.row_version);
        check_version(*version, current)?;
    }
    let revision = next_revision(&mut tx, &scope, ctx).await?;
    let (mut ids, mut students, mut values, mut clear_ids, mut clear_students) =
        (vec![], vec![], vec![], vec![], vec![]);
    for ((criterion, student), (value, _)) in cells {
        if let Some(value) = value {
            ids.push(criterion);
            students.push(student);
            values.push(value);
        } else {
            clear_ids.push(criterion);
            clear_students.push(student);
        }
    }
    if !ids.is_empty() {
        sqlx::query("INSERT INTO learning_group_student_evaluations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,subject_id,domain,subject_term_criterion_id,student_academic_year_id,quality_level,row_version,updated_by) SELECT $1,$2,$3,$4,$5,$6,c.criterion,c.student,c.level,$10,$11 FROM unnest($7::uuid[],$8::uuid[],$9::smallint[]) c(criterion,student,level) ON CONFLICT (learning_group_id,student_academic_year_id,subject_term_criterion_id) DO UPDATE SET quality_level=EXCLUDED.quality_level,row_version=EXCLUDED.row_version,updated_by=EXCLUDED.updated_by,updated_at=now()").bind(group).bind(group_scope.offering_id).bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(subject).bind(domain.as_str()).bind(ids).bind(students).bind(values).bind(revision).bind(actor.user_id).execute(&mut *tx).await?;
    }
    if !clear_ids.is_empty() {
        sqlx::query("DELETE FROM learning_group_student_evaluations r USING unnest($1::uuid[],$2::uuid[]) c(criterion,student) WHERE r.learning_group_id=$3 AND r.subject_term_criterion_id=c.criterion AND r.student_academic_year_id=c.student").bind(clear_ids).bind(clear_students).bind(group).execute(&mut *tx).await?;
    }
    invalidate(&mut tx, &scope, ctx, Some(group)).await?;
    let ws = load_workspace(&mut tx, actor, &scope, group_scope, ctx).await?;
    tx.commit().await?;
    Ok(ws)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_duplicate_cells_reject_whole_batch() {
        let a = ResponseInput {
            subject_term_criterion_id: Uuid::new_v4(),
            student_academic_year_id: Uuid::new_v4(),
            quality_level: Some(0.try_into().unwrap()),
            row_version: None,
        };
        assert!(normalize(vec![a.clone(), a.clone()]).is_ok());
        assert!(normalize(vec![
            a.clone(),
            ResponseInput {
                quality_level: None,
                ..a
            }
        ])
        .is_err());
    }
}
