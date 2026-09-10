use super::*;
use sha2::{Digest, Sha256};

pub async fn get_group_phase_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
) -> Result<GroupPhaseWorkspace, AppError> {
    let (mut tx, scope) = begin_scope(pool, actor, group, phase, context, false, false).await?;
    let workspace = load_workspace(&mut tx, actor, &scope).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(super) async fn load_workspace(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    scope: &Scope,
) -> Result<GroupPhaseWorkspace, AppError> {
    let items:Vec<ScoreItem>=sqlx::query_as("SELECT id,name,max_score::text AS max_score,display_order,lifecycle,row_version FROM learning_group_score_items WHERE learning_group_id=$1 AND assessment_phase_id=$2 ORDER BY display_order,id LIMIT 1001").bind(scope.group_id).bind(scope.phase_id).fetch_all(&mut **tx).await?;
    let students:Vec<GradebookStudent>=sqlx::query_as("SELECT m.id AS membership_id,m.student_academic_year_id,concat_ws(' ',u.first_name,u.last_name) AS display_name,m.row_version FROM learning_group_students m JOIN users u ON u.id=m.student_id WHERE m.learning_group_id=$1 AND m.membership_status='active' ORDER BY m.student_academic_year_id LIMIT 2001").bind(scope.group_id).fetch_all(&mut **tx).await?;
    let scores:Vec<ScoreCell>=sqlx::query_as("SELECT s.score_item_id,s.student_academic_year_id,s.score::text AS value,s.row_version FROM learning_group_student_scores s JOIN learning_group_score_items i ON i.id=s.score_item_id WHERE s.learning_group_id=$1 AND i.assessment_phase_id=$2 ORDER BY s.score_item_id,s.student_academic_year_id LIMIT 200001").bind(scope.group_id).bind(scope.phase_id).fetch_all(&mut **tx).await?;
    if items.len() > 1000 || students.len() > 2000 || scores.len() > 200000 {
        return Err(AppError::ValidationError(
            "Gradebook workspace is too large".into(),
        ));
    }
    let (roster_checksum, source_checksum) = super::source_checksums(
        scope.group_id,
        scope.phase_id,
        scope.phase_row_version,
        &scope.phase_max_score,
        &items,
        &students,
        &scores,
    )?;
    let confirmation:Option<PhaseConfirmation>=sqlx::query_as("SELECT id,blank_score_count,source_checksum,roster_checksum,row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) AS invalidated FROM learning_group_phase_confirmations WHERE learning_group_id=$1 AND assessment_phase_id=$2").bind(scope.group_id).bind(scope.phase_id).fetch_optional(&mut **tx).await?;
    let confirmation_is_current = confirmation.as_ref().is_some_and(|c| {
        !c.invalidated
            && c.source_checksum == source_checksum
            && c.roster_checksum == roster_checksum
    });
    Ok(GroupPhaseWorkspace {
        learning_group_id: scope.group_id,
        learning_offering_id: scope.offering_id,
        assessment_phase_id: scope.phase_id,
        phase_code: scope.phase_code.clone(),
        phase_max_score: scope.phase_max_score.clone(),
        phase_row_version: scope.phase_row_version,
        score_entry_enabled: scope.score_entry_enabled,
        locked: scope.locked,
        can_manage: scope.academic_state.is_writable()
            && !scope.locked
            && policy::can_manage_group(actor, scope.assigned)
            && (scope.score_entry_enabled || policy::can_manage_school(actor)),
        can_confirm: scope.academic_state.is_writable()
            && !scope.locked
            && policy::can_confirm_group_phase(actor, scope.primary_teacher)
            && (scope.score_entry_enabled || policy::can_manage_school(actor)),
        items,
        students,
        scores,
        source_checksum,
        roster_checksum,
        confirmation,
        confirmation_is_current,
    })
}

// Cosmetic item revisions deliberately do not enter this calculation identity.
pub fn source_checksums(
    group_id: Uuid,
    phase_id: Uuid,
    phase_row_version: i64,
    phase_max_score: &str,
    items: &[ScoreItem],
    students: &[GradebookStudent],
    scores: &[ScoreCell],
) -> Result<(String, String), AppError> {
    let mut roster = students
        .iter()
        .map(|s| (s.student_academic_year_id, s.membership_id, s.row_version))
        .collect::<Vec<_>>();
    roster.sort();
    let mut active_items = items
        .iter()
        .filter(|i| i.lifecycle == "active")
        .map(|i| (i.id, i.max_score.clone()))
        .collect::<Vec<_>>();
    active_items.sort();
    let mut active_scores = scores
        .iter()
        .filter(|s| {
            active_items.iter().any(|i| i.0 == s.score_item_id)
                && roster.iter().any(|r| r.0 == s.student_academic_year_id)
        })
        .map(|s| {
            (
                s.score_item_id,
                s.student_academic_year_id,
                s.value.clone(),
                s.row_version,
            )
        })
        .collect::<Vec<_>>();
    active_scores.sort();
    let roster_checksum = hash(&roster)?;
    let source_checksum = hash(&(
        group_id,
        phase_id,
        phase_row_version,
        phase_max_score,
        &roster_checksum,
        active_items,
        active_scores,
    ))?;
    Ok((roster_checksum, source_checksum))
}
fn hash(value: &impl serde::Serialize) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("Could not encode Gradebook revision".into()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
