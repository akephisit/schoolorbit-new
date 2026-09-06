use super::*;
use std::collections::BTreeMap;

pub async fn save_scores_batch(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    mutations: Vec<ScoreCellMutation>,
) -> Result<ScoreBatchOutcome, AppError> {
    if mutations.is_empty() || mutations.len() > 500 {
        return Err(AppError::ValidationError(
            "Score batch must contain 1–500 cells".into(),
        ));
    }
    let cells = normalize(mutations)?;
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, true, false).await?;
    let workspace = workspace::load_workspace(&mut tx, actor, &scope).await?;
    // Validate the complete normalized batch against the same locked source before any write.
    for ((item_id, student_id), (value, version)) in &cells {
        let item = workspace
            .items
            .iter()
            .find(|i| i.id == *item_id && i.lifecycle == "active")
            .ok_or_else(|| {
                AppError::ValidationError("Score item is not active in this group phase".into())
            })?;
        if !workspace
            .students
            .iter()
            .any(|s| s.student_academic_year_id == *student_id)
        {
            return Err(AppError::ValidationError(
                "Student is not on the current group roster".into(),
            ));
        }
        if let Some(value) = value {
            if value > &decimal(&item.max_score)? {
                return Err(AppError::ValidationError(
                    "Score exceeds the item maximum".into(),
                ));
            }
        }
        let actual = workspace
            .scores
            .iter()
            .find(|s| s.score_item_id == *item_id && s.student_academic_year_id == *student_id)
            .and_then(|s| s.row_version);
        check_version(*version, actual)?;
    }
    // A group revision survives a cell being cleared and prevents version reuse on reinsertion.
    let revision:i64=sqlx::query_scalar("UPDATE learning_groups SET row_version=row_version+1,updated_at=now() WHERE id=$1 RETURNING row_version").bind(group).fetch_one(&mut *tx).await?;
    let mut set_items = Vec::new();
    let mut set_students = Vec::new();
    let mut values = Vec::new();
    let mut clear_items = Vec::new();
    let mut clear_students = Vec::new();
    let mut result = Vec::new();
    for ((item_id, student_id), (value, _)) in cells {
        if let Some(value) = value {
            set_items.push(item_id);
            set_students.push(student_id);
            values.push(value.clone());
            result.push(ScoreCell {
                score_item_id: item_id,
                student_academic_year_id: student_id,
                value: Some(format!("{value:.2}")),
                row_version: Some(revision),
            });
        } else {
            clear_items.push(item_id);
            clear_students.push(student_id);
            result.push(ScoreCell {
                score_item_id: item_id,
                student_academic_year_id: student_id,
                value: None,
                row_version: None,
            });
        }
    }
    if !set_items.is_empty() {
        sqlx::query("INSERT INTO learning_group_student_scores (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score,row_version,updated_by) SELECT $1,$2,$3,$4,c.item,c.student,c.score,$8,$9 FROM unnest($5::uuid[],$6::uuid[],$7::numeric[]) AS c(item,student,score) ON CONFLICT (score_item_id,student_academic_year_id) DO UPDATE SET score=EXCLUDED.score,row_version=EXCLUDED.row_version,updated_by=EXCLUDED.updated_by,updated_at=now()").bind(group).bind(scope.offering_id).bind(context.academic_term_id).bind(context.academic_year_id).bind(&set_items).bind(&set_students).bind(&values).bind(revision).bind(actor.user_id).execute(&mut *tx).await?;
    }
    if !clear_items.is_empty() {
        sqlx::query("DELETE FROM learning_group_student_scores s USING unnest($1::uuid[],$2::uuid[]) AS c(item,student) WHERE s.score_item_id=c.item AND s.student_academic_year_id=c.student").bind(clear_items).bind(clear_students).execute(&mut *tx).await?;
    }
    invalidate(&mut tx, &scope).await?;
    let updated = workspace::load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(ScoreBatchOutcome {
        cells: result,
        workspace_revision: updated.source_checksum,
    })
}
type NormalizedCells = BTreeMap<(Uuid, Uuid), (Option<BigDecimal>, Option<i64>)>;
fn normalize(mutations: Vec<ScoreCellMutation>) -> Result<NormalizedCells, AppError> {
    let mut cells = BTreeMap::new();
    for mutation in mutations {
        let (item, student, value, version) = match mutation {
            ScoreCellMutation::Set {
                score_item_id,
                student_academic_year_id,
                value,
                row_version,
            } => (
                score_item_id,
                student_academic_year_id,
                Some(decimal(&value)?),
                row_version,
            ),
            ScoreCellMutation::Clear {
                score_item_id,
                student_academic_year_id,
                row_version,
            } => (score_item_id, student_academic_year_id, None, row_version),
        };
        let entry = (value, version);
        if let Some(prior) = cells.insert((item, student), entry.clone()) {
            if prior != entry {
                return Err(AppError::ValidationError(
                    "Batch contains conflicting duplicate cells".into(),
                ));
            }
        }
    }
    Ok(cells)
}
