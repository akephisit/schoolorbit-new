//! Test composition uses the same current providers and Assessment adapter as the application.
pub use school_academic_delivery::services::{
    activities, current, opening, roster_memberships, workspaces,
};
pub mod offerings {
    pub use school_academic_delivery::services::offerings::*;
    pub async fn create(
        pool: &sqlx::PgPool,
        actor: uuid::Uuid,
        request: school_academic_delivery::models::CreateLearningOfferingRequest,
    ) -> Result<school_academic_delivery::models::LearningOffering, school_errors::AppError> {
        school_academic_delivery::services::offerings::create(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            pool,
            actor,
            request,
        )
        .await
    }
}
pub mod groups {
    pub use school_academic_delivery::services::groups::*;
    pub async fn replace_teachers(
        pool: &sqlx::PgPool,
        actor: uuid::Uuid,
        id: uuid::Uuid,
        request: school_academic_delivery::models::ReplaceLearningGroupTeachersRequest,
    ) -> Result<school_academic_delivery::models::LearningGroup, school_errors::AppError> {
        school_academic_delivery::services::groups::replace_teachers(
            &crate::adapters::assessment_current::ASSESSMENT_CURRENT,
            pool,
            actor,
            id,
            request,
        )
        .await
    }
}
