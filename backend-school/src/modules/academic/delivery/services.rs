pub use school_academic_delivery::services::{
    activities, opening, roster_memberships, teacher_handoff, workspaces,
};

pub mod offerings {
    pub use school_academic_delivery::services::offerings::*;

    use school_academic_delivery::models::{CreateLearningOfferingRequest, LearningOffering};
    use school_errors::AppError;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::super::adapters::TIMETABLE_MUTATIONS;

    pub async fn create(
        pool: &PgPool,
        actor_user_id: Uuid,
        request: CreateLearningOfferingRequest,
    ) -> Result<LearningOffering, AppError> {
        create_for_timetable(pool, actor_user_id, None, request).await
    }

    pub async fn create_for_timetable(
        pool: &PgPool,
        actor_user_id: Uuid,
        timetable_version_id: Option<Uuid>,
        request: CreateLearningOfferingRequest,
    ) -> Result<LearningOffering, AppError> {
        school_academic_delivery::services::offerings::create(
            &TIMETABLE_MUTATIONS,
            pool,
            actor_user_id,
            timetable_version_id,
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

    use super::super::adapters::TIMETABLE_MUTATIONS;

    pub async fn create(
        pool: &PgPool,
        actor_user_id: Uuid,
        offering_id: Uuid,
        request: CreateLearningGroupRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::create(
            &TIMETABLE_MUTATIONS,
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
        school_academic_delivery::services::groups::update(
            &TIMETABLE_MUTATIONS,
            pool,
            actor_user_id,
            id,
            request,
        )
        .await
    }

    pub async fn replace_teachers(
        pool: &PgPool,
        actor_user_id: Uuid,
        id: Uuid,
        request: ReplaceLearningGroupTeachersRequest,
    ) -> Result<LearningGroup, AppError> {
        school_academic_delivery::services::groups::replace_teachers(
            &TIMETABLE_MUTATIONS,
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
            &TIMETABLE_MUTATIONS,
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

    use super::super::adapters::TIMETABLE_MUTATIONS;

    pub async fn create_change_set(
        pool: &PgPool,
        actor_user_id: Uuid,
        request: CreateAcademicTermChangeSetRequest,
    ) -> Result<AcademicTermChangeSet, AppError> {
        school_academic_delivery::services::change_sets::create_change_set(
            &TIMETABLE_MUTATIONS,
            pool,
            actor_user_id,
            request,
        )
        .await
    }
}
