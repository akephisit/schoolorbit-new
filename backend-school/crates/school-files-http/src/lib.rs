pub mod access_policy;

pub mod http;
#[cfg(test)]
mod test_support;

pub struct DomainFileAuthorization;
pub static FILE_AUTHORIZATION: DomainFileAuthorization = DomainFileAuthorization;
#[async_trait::async_trait]
impl school_file_platform::access::FileAuthorizationPort for DomainFileAuthorization {
    async fn authorize_create(
        &self,
        pool: &sqlx::PgPool,
        actor: &school_authorization::ActorContext,
        purpose: school_file_platform::platform_types::FilePurpose,
        resource_id: Option<uuid::Uuid>,
    ) -> Result<uuid::Uuid, school_errors::AppError> {
        access_policy::authorize_create(pool, actor, purpose, resource_id).await
    }
    async fn authorize_existing(
        &self,
        pool: &sqlx::PgPool,
        actor: &school_authorization::ActorContext,
        file: &school_file_platform::repository::PlatformFile,
        action: school_file_platform::access::FilePolicyAction,
        resource_id: Option<uuid::Uuid>,
    ) -> Result<(), school_errors::AppError> {
        access_policy::authorize_existing(pool, actor, file, action, resource_id).await
    }
    async fn authorize_portal_application(
        &self,
        pool: &sqlx::PgPool,
        authenticated_application_id: uuid::Uuid,
        requested_application_id: uuid::Uuid,
        file_id: Option<uuid::Uuid>,
    ) -> Result<(), school_errors::AppError> {
        access_policy::authorize_portal_application(
            pool,
            authenticated_application_id,
            requested_application_id,
            file_id,
        )
        .await
    }
}
