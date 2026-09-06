use super::*;

pub async fn confirm_phase(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    input: ConfirmInput,
) -> Result<PhaseConfirmation, AppError> {
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, true, true).await?;
    let workspace = workspace::load_workspace(&mut tx, actor, &scope).await?;
    if workspace.source_checksum != input.source_checksum
        || workspace.roster_checksum != input.roster_checksum
    {
        return Err(conflict());
    }
    let current:Option<i64>=sqlx::query_scalar("SELECT row_version FROM learning_group_phase_confirmations WHERE learning_group_id=$1 AND assessment_phase_id=$2 FOR UPDATE").bind(group).bind(phase).fetch_optional(&mut *tx).await?;
    check_version(input.row_version, current)?;
    let active = workspace
        .items
        .iter()
        .filter(|i| i.lifecycle == "active")
        .collect::<Vec<_>>();
    let total = active.iter().try_fold(BigDecimal::from(0), |sum, i| {
        decimal(&i.max_score).map(|v| sum + v)
    })?;
    if total != decimal(&scope.phase_max_score)? {
        return Err(AppError::ValidationError(format!(
            "Active item maxima total {} but phase requires {}",
            total, scope.phase_max_score
        )));
    }
    let mut entered = 0i64;
    for score in &workspace.scores {
        if let Some(item) = active.iter().find(|i| i.id == score.score_item_id) {
            if workspace
                .students
                .iter()
                .any(|s| s.student_academic_year_id == score.student_academic_year_id)
            {
                let value = score
                    .value
                    .as_deref()
                    .ok_or_else(|| AppError::ValidationError("Stored score is invalid".into()))?;
                if decimal(value)? > decimal(&item.max_score)? {
                    return Err(AppError::ValidationError(
                        "A retained score exceeds its maximum".into(),
                    ));
                }
                entered += 1;
            }
        }
    }
    let blank_score_count = (active.len() * workspace.students.len()) as i64 - entered;
    let snapshot = ConfirmationSnapshot {
        phase_row_version: scope.phase_row_version,
        phase_max_score: &scope.phase_max_score,
        roster_checksum: &workspace.roster_checksum,
        source_checksum: &workspace.source_checksum,
        roster: workspace
            .students
            .iter()
            .map(|s| RosterRevision {
                membership_id: s.membership_id,
                student_academic_year_id: s.student_academic_year_id,
                row_version: s.row_version,
            })
            .collect(),
        items: {
            let mut items = active
                .iter()
                .map(|i| ItemMaximum {
                    id: i.id,
                    max_score: i.max_score.clone(),
                })
                .collect::<Vec<_>>();
            items.sort_by_key(|i| i.id);
            items
        },
        scores: workspace
            .scores
            .iter()
            .filter(|s| {
                active.iter().any(|i| i.id == s.score_item_id)
                    && workspace.students.iter().any(|student| {
                        student.student_academic_year_id == s.student_academic_year_id
                    })
            })
            .cloned()
            .collect(),
    };
    let confirmation=sqlx::query_as("INSERT INTO learning_group_phase_confirmations (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,course_assessment_plan_id,assessment_phase_id,phase_row_version,blank_score_count,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT (learning_group_id,assessment_phase_id) DO UPDATE SET phase_row_version=EXCLUDED.phase_row_version,blank_score_count=EXCLUDED.blank_score_count,roster_checksum=EXCLUDED.roster_checksum,source_checksum=EXCLUDED.source_checksum,source_snapshot=EXCLUDED.source_snapshot,confirmed_by=EXCLUDED.confirmed_by,confirmed_at=now(),row_version=learning_group_phase_confirmations.row_version+1 RETURNING id,blank_score_count,source_checksum,roster_checksum,row_version").bind(group).bind(scope.offering_id).bind(context.academic_term_id).bind(context.academic_year_id).bind(scope.plan_id).bind(phase).bind(scope.phase_row_version).bind(blank_score_count).bind(&workspace.roster_checksum).bind(&workspace.source_checksum).bind(sqlx::types::Json(snapshot)).bind(actor.user_id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(confirmation)
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmationSnapshot<'a> {
    phase_row_version: i64,
    phase_max_score: &'a str,
    roster_checksum: &'a str,
    source_checksum: &'a str,
    roster: Vec<RosterRevision>,
    items: Vec<ItemMaximum>,
    scores: Vec<ScoreCell>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RosterRevision {
    membership_id: Uuid,
    student_academic_year_id: Uuid,
    row_version: i64,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemMaximum {
    id: Uuid,
    max_score: String,
}
