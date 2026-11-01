pub use school_academic_delivery::services::{activities, opening, roster_memberships, workspaces};

pub mod offerings {
    pub use school_academic_delivery::services::offerings::*;

    use school_academic_delivery::models::{CreateLearningOfferingRequest, LearningOffering};
    use school_errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    pub async fn publish(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: school_academic_delivery::models::PublishLearningOfferingRequest,
    ) -> Result<LearningOffering, AppError> {
        school_academic_delivery::services::offerings::publish(
            &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
            pool,
            actor_user_id,
            id,
            request,
        )
        .await
    }

    pub async fn create(
        pool: &PgPool,
        actor_user_id: Uuid,
        request: CreateLearningOfferingRequest,
    ) -> Result<LearningOffering, AppError> {
        create_for_delivery(pool, actor_user_id, None, request).await
    }

    pub async fn create_for_delivery(
        pool: &PgPool,
        actor_user_id: Uuid,
        delivery_version_id: Option<Uuid>,
        request: CreateLearningOfferingRequest,
    ) -> Result<LearningOffering, AppError> {
        school_academic_delivery::services::offerings::create(
            pool,
            actor_user_id,
            delivery_version_id,
            request,
        )
        .await
    }
}

pub mod groups {
    pub use school_academic_delivery::services::groups::*;

    use school_academic_delivery::models::{
        CreateLearningGroupRequest, LearningGroup, ReplaceLearningGroupHomeroomsRequest,
        ReplaceLearningGroupTeachersRequest, UpdateLearningGroupRequest,
    };
    use school_errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    pub async fn create(
        pool: &PgPool,
        actor_user_id: Uuid,
        offering_id: Uuid,
        request: CreateLearningGroupRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::create(
            pool,
            actor_user_id,
            offering_id,
            request,
        )
        .await
    }

    pub async fn update(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: UpdateLearningGroupRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::update(pool, actor_user_id, id, request).await
    }

    pub async fn replace_teachers(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: ReplaceLearningGroupTeachersRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::replace_teachers(
            pool,
            actor_user_id,
            id,
            request,
        )
        .await
    }

    pub async fn replace_homerooms(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: ReplaceLearningGroupHomeroomsRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::replace_homerooms(
            pool,
            actor_user_id,
            id,
            request,
        )
        .await
    }
}

pub mod change_sets {
    pub use school_academic_delivery::services::change_sets::*;

    use school_academic_delivery::models::{
        AcademicTermChangeSet, CreateAcademicTermChangeSetRequest,
    };
    use school_errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    pub async fn preview_change_set(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<school_academic_delivery::models::AcademicTermChangeSetPreview, AppError> {
        school_academic_delivery::services::change_sets::preview_change_set(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            pool,
            id,
        )
        .await
    }
    pub async fn preview_change_set_at(
        pool: &PgPool,
        id: Uuid,
        date: Option<chrono::NaiveDate>,
    ) -> Result<school_academic_delivery::models::AcademicTermChangeSetPreview, AppError> {
        school_academic_delivery::services::change_sets::preview_change_set_at(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            pool,
            id,
            date,
        )
        .await
    }

    pub async fn publish_change_set(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: school_academic_delivery::models::PublishAcademicTermChangeSetRequest,
    ) -> Result<AcademicTermChangeSet, AppError> {
        school_academic_delivery::services::change_sets::publish_change_set(
            &crate::adapters::publication_history::PUBLICATION_HISTORY,
            &crate::adapters::assessment_publication::ASSESSMENT_PUBLICATION,
            pool,
            actor_user_id,
            id,
            request,
        )
        .await
    }

    pub async fn create_change_set(
        pool: &PgPool,
        actor_user_id: Uuid,
        request: CreateAcademicTermChangeSetRequest,
    ) -> Result<AcademicTermChangeSet, AppError> {
        school_academic_delivery::services::change_sets::create_change_set(
            pool,
            actor_user_id,
            request,
        )
        .await
    }
}

pub use school_academic_timetable::services::teacher_handoff;
