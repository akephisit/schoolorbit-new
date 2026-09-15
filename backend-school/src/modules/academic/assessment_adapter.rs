use school_academic_assessment::assessment::models::{
    AssessmentPlanDetail, SaveAssessmentPlanRequest,
};
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::academic::result_lock_adapter::RESULT_LOCKS;

pub async fn save_plan(
    pool: &PgPool,
    offering_id: Uuid,
    actor_user_id: Uuid,
    can_manage_school: bool,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    school_academic_assessment::assessment::services::save_plan(
        &RESULT_LOCKS,
        pool,
        offering_id,
        actor_user_id,
        can_manage_school,
        payload,
    )
    .await
}
