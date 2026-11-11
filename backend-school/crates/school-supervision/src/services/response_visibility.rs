use crate::access_policy as supervision_access_policy;
use crate::models::{SupervisionObservation, SupervisionTeacherStatusRow};
use school_authorization::ActorContext;
use school_errors::AppError;
use uuid::Uuid;

use super::shared::can_view_observation_results;

fn redact_observation_results_for_actor(
    actor: &ActorContext,
    managed_user_ids: &std::collections::HashSet<Uuid>,
    observation: &mut SupervisionObservation,
) {
    observation.can_manage = managed_user_ids.contains(&observation.observed_user_id);
    let can_view_unreleased =
        observation.can_manage || supervision_access_policy::can_approve_school(actor);
    observation.can_review = can_view_observation_results(observation.status, can_view_unreleased);
    if !observation.can_review {
        observation.average_rating = None;
    }
}

pub async fn redacted_observation_for_actor(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    mut observation: SupervisionObservation,
) -> Result<SupervisionObservation, AppError> {
    redact_observations_results_for_actor(pool, actor, std::slice::from_mut(&mut observation))
        .await?;
    Ok(observation)
}

pub async fn redact_observations_results_for_actor(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    observations: &mut [SupervisionObservation],
) -> Result<(), AppError> {
    let user_ids: Vec<_> = observations
        .iter()
        .map(|observation| observation.observed_user_id)
        .collect();
    let managed_user_ids =
        supervision_access_policy::observation_management_user_ids(pool, actor, &user_ids).await?;
    for observation in observations {
        redact_observation_results_for_actor(actor, &managed_user_ids, observation);
    }
    Ok(())
}

pub async fn redact_teacher_status_results_for_actor(
    pool: &sqlx::PgPool,
    actor: &ActorContext,
    rows: &mut [SupervisionTeacherStatusRow],
) -> Result<(), AppError> {
    let user_ids: Vec<_> = rows.iter().map(|row| row.teacher_id).collect();
    let managed_user_ids =
        supervision_access_policy::observation_management_user_ids(pool, actor, &user_ids).await?;
    for row in rows {
        if let Some(status) = row.status {
            let can_view_unreleased = managed_user_ids.contains(&row.teacher_id)
                || supervision_access_policy::can_approve_school(actor);
            if !can_view_observation_results(status, can_view_unreleased) {
                row.average_rating = None;
            }
        }
    }
    Ok(())
}
