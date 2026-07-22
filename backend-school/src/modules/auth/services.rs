use super::models::{LoginUser, UpdateProfileRequest, User};
use crate::error::AppError;
use crate::utils::field_encryption;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn find_active_login_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<LoginUser, AppError> {
    sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_url
        FROM users
        WHERE username = $1 AND status = 'active'
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::AuthError(
        "ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string(),
    ))
}

pub async fn find_active_login_user_by_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<LoginUser, AppError> {
    sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_url
        FROM users
        WHERE id = $1 AND status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound(
        "ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string(),
    ))
}

pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> Result<User, AppError> {
    let mut user = sqlx::query_as::<_, User>(
        "SELECT 
            id,
            username,
            national_id,
            email,
            password_hash,
            first_name,
            last_name,
            user_type,
            phone,
            date_of_birth,
            address,
            status,
            metadata,
            created_at,
            updated_at,
            title,
            nickname,
            emergency_contact,
            line_id,
            gender,
            profile_image_url,
            hired_date,
            resigned_date
         FROM users 
         WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("ไม่พบผู้ใช้".to_string()))?;

    decrypt_national_id(&mut user);
    Ok(user)
}

pub fn ensure_active_user_status(status: &str) -> Result<(), AppError> {
    if status == "active" {
        return Ok(());
    }

    Err(AppError::AuthError("บัญชีผู้ใช้ถูกระงับ".to_string()))
}

pub async fn get_primary_role_name(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<String>, AppError> {
    sqlx::query_scalar::<_, String>(
        "SELECT r.name 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.is_primary = true 
           AND ur.ended_at IS NULL
           AND r.is_active = true
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_profile(
    pool: &PgPool,
    user_id: Uuid,
    payload: UpdateProfileRequest,
) -> Result<User, AppError> {
    let date_of_birth = parse_profile_date(payload.date_of_birth.as_deref());

    sqlx::query(
        "UPDATE users 
         SET title = COALESCE($1, title),
             nickname = COALESCE($2, nickname),
             email = COALESCE($3, email),
             phone = COALESCE($4, phone),
             emergency_contact = COALESCE($5, emergency_contact),
             line_id = COALESCE($6, line_id),
             date_of_birth = COALESCE($7, date_of_birth),
             gender = COALESCE($8, gender),
             address = COALESCE($9, address),
             profile_image_url = COALESCE($10, profile_image_url),
             updated_at = NOW()
         WHERE id = $11",
    )
    .bind(&payload.title)
    .bind(&payload.nickname)
    .bind(&payload.email)
    .bind(&payload.phone)
    .bind(&payload.emergency_contact)
    .bind(&payload.line_id)
    .bind(date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.address)
    .bind(&payload.profile_image_url)
    .bind(user_id)
    .execute(pool)
    .await?;

    find_user_by_id(pool, user_id).await
}

pub async fn update_password_hash(
    pool: &PgPool,
    user_id: Uuid,
    password_hash: String,
) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

fn decrypt_national_id(user: &mut User) {
    if let Some(national_id) = &user.national_id {
        if let Ok(decrypted) = field_encryption::decrypt(national_id) {
            user.national_id = Some(decrypted);
        }
    }
}

fn parse_profile_date(value: Option<&str>) -> Option<chrono::NaiveDate> {
    value.and_then(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_with_national_id(national_id: Option<&str>) -> User {
        User {
            id: Uuid::new_v4(),
            username: "test-user".to_string(),
            national_id: national_id.map(str::to_string),
            email: Some("test@example.com".to_string()),
            password_hash: "hash".to_string(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            user_type: "staff".to_string(),
            phone: None,
            date_of_birth: None,
            address: None,
            status: "active".to_string(),
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            title: None,
            nickname: None,
            emergency_contact: None,
            line_id: None,
            gender: None,
            profile_image_url: None,
            hired_date: None,
            resigned_date: None,
        }
    }

    #[test]
    fn parse_profile_date_accepts_iso_date() {
        let parsed = parse_profile_date(Some("2026-06-06"));

        assert_eq!(parsed, chrono::NaiveDate::from_ymd_opt(2026, 6, 6));
    }

    #[test]
    fn active_user_status_is_accepted_for_current_session() {
        assert!(ensure_active_user_status("active").is_ok());
    }

    #[test]
    fn inactive_user_status_is_rejected_for_current_session() {
        let result = ensure_active_user_status("inactive");

        assert!(matches!(result, Err(AppError::AuthError(_))));
    }

    #[test]
    fn parse_profile_date_ignores_invalid_date() {
        let parsed = parse_profile_date(Some("06/06/2026"));

        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_profile_date_ignores_missing_or_empty_date() {
        assert_eq!(parse_profile_date(None), None);
        assert_eq!(parse_profile_date(Some("")), None);
    }

    #[test]
    fn decrypt_national_id_keeps_invalid_ciphertext_unchanged() {
        let mut user = user_with_national_id(Some("not-ciphertext"));

        decrypt_national_id(&mut user);

        assert_eq!(user.national_id.as_deref(), Some("not-ciphertext"));
    }

    #[test]
    fn decrypt_national_id_decrypts_valid_ciphertext() {
        let _guard = field_encryption::test_env_lock();
        std::env::set_var("ENCRYPTION_KEY", "auth-service-test-key");
        let encrypted = field_encryption::encrypt("1234567890123").expect("encrypt national id");
        let mut user = user_with_national_id(Some(&encrypted));

        decrypt_national_id(&mut user);

        assert_eq!(user.national_id.as_deref(), Some("1234567890123"));
    }
}
