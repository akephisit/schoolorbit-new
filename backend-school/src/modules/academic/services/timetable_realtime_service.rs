use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
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
    username: String,
    title: Option<String>,
    first_name: String,
    last_name: String,
}

fn socket_permission(actor: &ActorContext) -> Result<bool, AppError> {
    let can_manage = actor.has_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL);
    if actor.has_any_permission(&[
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
    ]) {
        Ok(can_manage)
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์ดูตารางเรียน".to_string()))
    }
}

fn display_name(title: Option<&str>, first_name: &str, last_name: &str, username: &str) -> String {
    let given_name = format!("{}{}", title.unwrap_or_default().trim(), first_name.trim());
    let full_name = [given_name.as_str(), last_name.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if full_name.is_empty() {
        let username = username.trim();
        if username.is_empty() {
            "ผู้ใช้งาน".to_string()
        } else {
            username.to_string()
        }
    } else {
        full_name
    }
}

pub async fn authorize_socket(
    pool: &PgPool,
    actor: &ActorContext,
    semester_id: Uuid,
) -> Result<TimetableSocketAccess, AppError> {
    let can_manage = socket_permission(actor)?;
    let user = sqlx::query_as::<_, RealtimeUser>(
        "SELECT COALESCE(username, '') AS username, title, first_name, last_name FROM users WHERE id = $1 AND status = 'active'",
    )
    .bind(actor.user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::AuthError("ไม่พบผู้ใช้งานที่เปิดใช้งาน".to_string()))?;

    let semester_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM academic_semesters WHERE id = $1)",
    )
    .bind(semester_id)
    .fetch_one(pool)
    .await?;
    if !semester_exists {
        return Err(AppError::NotFound("ไม่พบภาคเรียน".to_string()));
    }

    Ok(TimetableSocketAccess {
        user_id: actor.user_id,
        display_name: display_name(
            user.title.as_deref(),
            &user.first_name,
            &user.last_name,
            &user.username,
        ),
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
        assert!(!socket_permission(&actor(&[codes::ACADEMIC_COURSE_PLAN_READ_ALL])).unwrap());
    }

    #[test]
    fn manager_and_wildcard_can_manage() {
        assert!(socket_permission(&actor(&[codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL])).unwrap());
        assert!(socket_permission(&actor(&[codes::WILDCARD])).unwrap());
    }

    #[test]
    fn unrelated_permission_is_forbidden() {
        assert!(matches!(
            socket_permission(&actor(&["calendar.read.all"])),
            Err(AppError::Forbidden(_))
        ));
    }

    #[test]
    fn display_name_prefers_person_name_and_falls_back_to_username() {
        assert_eq!(
            display_name(Some("นาย"), "สมชาย", "ใจดี", "staff1"),
            "นายสมชาย ใจดี"
        );
        assert_eq!(display_name(None, "", "", "staff1"), "staff1");
    }

    #[test]
    fn display_name_uses_non_empty_generic_fallback_when_all_identity_fields_are_blank() {
        assert_eq!(display_name(Some(" \t"), " ", "\n", " \t"), "ผู้ใช้งาน");
    }
}
