use school_academic_assessment::{gradebook::models::*, ports::ResultLockPort};
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::academic::result_lock_adapter::RESULT_LOCKS;

fn result_locks() -> &'static impl ResultLockPort {
    &RESULT_LOCKS
}

pub async fn create_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    input: ItemInput,
) -> Result<ScoreItem, AppError> {
    school_academic_assessment::gradebook::services::create_item(
        result_locks(),
        pool,
        actor,
        group,
        phase,
        context,
        input,
    )
    .await
}

pub async fn update_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    item_id: Uuid,
    context: &GradebookContext,
    input: ItemInput,
) -> Result<ScoreItem, AppError> {
    school_academic_assessment::gradebook::services::update_item(
        result_locks(),
        pool,
        actor,
        group,
        phase,
        item_id,
        context,
        input,
    )
    .await
}

pub async fn remove_item(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    item_id: Uuid,
    context: &GradebookContext,
    row_version: i64,
) -> Result<ScoreItemRemovalOutcome, AppError> {
    school_academic_assessment::gradebook::services::remove_item(
        result_locks(),
        pool,
        actor,
        group,
        phase,
        item_id,
        context,
        row_version,
    )
    .await
}

pub async fn save_scores_batch(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    mutations: Vec<ScoreCellMutation>,
) -> Result<ScoreBatchOutcome, AppError> {
    school_academic_assessment::gradebook::services::save_scores_batch(
        result_locks(),
        pool,
        actor,
        group,
        phase,
        context,
        mutations,
    )
    .await
}

pub async fn confirm_phase(
    pool: &PgPool,
    actor: &ActorContext,
    group: Uuid,
    phase: Uuid,
    context: &GradebookContext,
    input: ConfirmInput,
) -> Result<PhaseConfirmation, AppError> {
    school_academic_assessment::gradebook::services::confirm_phase(
        result_locks(),
        pool,
        actor,
        group,
        phase,
        context,
        input,
    )
    .await
}
