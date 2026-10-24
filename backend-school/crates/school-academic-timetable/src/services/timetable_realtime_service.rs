use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TimetableSocketAccess {
    pub user_id: Uuid,
    pub display_name: String,
    pub can_manage: bool,
}

#[derive(sqlx::FromRow)]
struct RealtimeUser {
    display_name: String,
}

fn socket_permission(actor: &ActorContext) -> Result<bool, AppError> {
    let can_manage = actor.has_permission(codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL);
    if actor.has_any_permission(&[
        codes::ACADEMIC_TIMETABLE_READ_SCHOOL,
        codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
    ]) {
        Ok(can_manage)
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์ดูตารางเรียน".to_string()))
    }
}

pub async fn authorize_socket(
    pool: &PgPool,
    actor: &ActorContext,
    academic_term_id: Uuid,
) -> Result<TimetableSocketAccess, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.services.timetable_realtime_service.authorize_socket",
    )
    .await?;
    authorize_socket_on_connection(&mut connection, actor, academic_term_id).await
}

pub async fn authorize_socket_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    academic_term_id: Uuid,
) -> Result<TimetableSocketAccess, AppError> {
    let user = DbSession::new(connection)
        .query_as::<RealtimeUser>(
            "academic_timetable.services.timetable_realtime_service.authorize_socket_on_connection",
            "SELECT display_name FROM user_display_names WHERE id = $1 AND status = 'active'",
        )
        .bind(actor.user_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::AuthError("ไม่พบผู้ใช้งานที่เปิดใช้งาน".to_string()))?;

    let can_manage = socket_permission(actor)?;
    let term_exists = DbSession::new(connection)
        .scalar::<bool>(
            "academic_timetable.services.timetable_realtime_service.authorize_socket_on_connection",
            "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id = $1)",
        )
        .bind(academic_term_id)
        .fetch_one()
        .await?;
    if !term_exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".to_string()));
    }

    Ok(TimetableSocketAccess {
        user_id: actor.user_id,
        display_name: user.display_name,
        can_manage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[test]
    fn reader_connects_without_manage_capability() {
        assert!(!socket_permission(&actor(&[codes::ACADEMIC_TIMETABLE_READ_SCHOOL])).unwrap());
    }

    #[test]
    fn manager_and_wildcard_can_manage() {
        assert!(socket_permission(&actor(&[codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL])).unwrap());
        assert!(socket_permission(&actor(&[codes::WILDCARD])).unwrap());
    }

    #[test]
    fn unrelated_permission_is_forbidden() {
        assert!(matches!(
            socket_permission(&actor(&["calendar.read.all"])),
            Err(AppError::Forbidden(_))
        ));
    }
}
