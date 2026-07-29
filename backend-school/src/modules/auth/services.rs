use super::models::{LoginUser, UpdateProfileRequest, User};
use crate::error::AppError;
use crate::utils::field_encryption;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ProfileUpdateResult {
    pub user: User,
    pub replaced_file_id: Option<Uuid>,
}

pub async fn find_active_login_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<LoginUser, AppError> {
    sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_file_id
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
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_file_id
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
            profile_image_file_id,
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
) -> Result<ProfileUpdateResult, AppError> {
    let date_of_birth = parse_profile_date(payload.date_of_birth.as_deref());
    let mut transaction = pool.begin().await?;

    if let Some(Some(file_id)) = payload.profile_image_file_id {
        let valid = sqlx::query_scalar::<_, bool>(
            r#"
SELECT EXISTS(
    SELECT 1
    FROM files
    WHERE id = $1
      AND owner_user_id = $2
      AND purpose_code = 'profile_image'
      AND lifecycle_status = 'ready'
      AND deleted_at IS NULL
)
"#,
        )
        .bind(file_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !valid {
            return Err(AppError::ValidationError(
                "ไฟล์รูปโปรไฟล์ไม่พร้อมใช้งาน".to_string(),
            ));
        }
        sqlx::query(
            "UPDATE files SET retention_class = 'standard', expires_at = NULL, updated_at = NOW() WHERE id = $1",
        )
        .bind(file_id)
        .execute(&mut *transaction)
        .await?;
    }

    let old_file_id = sqlx::query_as::<_, (Option<Uuid>,)>(
        "SELECT profile_image_file_id FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบผู้ใช้".to_string()))?
    .0;

    let profile_image_update_requested = payload.profile_image_file_id.is_some();
    let new_file_id = payload.profile_image_file_id.unwrap_or(old_file_id);

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
             profile_image_file_id = CASE WHEN $10 THEN $11 ELSE profile_image_file_id END,
             updated_at = NOW()
         WHERE id = $12",
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
    .bind(profile_image_update_requested)
    .bind(new_file_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(ProfileUpdateResult {
        user: find_user_by_id(pool, user_id).await?,
        replaced_file_id: (profile_image_update_requested && old_file_id != new_file_id)
            .then_some(old_file_id)
            .flatten(),
    })
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
    use crate::test_helpers::{create_test_pool, create_test_user, run_test_migrations};

    fn profile_update(profile_image_file_id: Option<Option<Uuid>>) -> UpdateProfileRequest {
        UpdateProfileRequest {
            title: None,
            nickname: None,
            email: None,
            phone: None,
            emergency_contact: None,
            line_id: None,
            date_of_birth: None,
            gender: None,
            address: None,
            profile_image_file_id,
        }
    }

    async fn insert_ready_profile_image(pool: &PgPool, user_id: Uuid, temporary: bool) -> Uuid {
        let file_id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let mut transaction = pool
            .begin()
            .await
            .expect("profile image fixture transaction should begin");
        sqlx::query(
            r#"
INSERT INTO files (
    id, owner_user_id, display_filename, created_by, purpose_code, visibility,
    lifecycle_status, retention_class, expires_at
) VALUES (
    $1, $2, 'profile.jpg', $2, 'profile_image', 'private', 'processing',
    CASE WHEN $3 THEN 'temporary' ELSE 'standard' END,
    CASE WHEN $3 THEN now() + INTERVAL '1 hour' ELSE NULL END
)
"#,
        )
        .bind(file_id)
        .bind(user_id)
        .bind(temporary)
        .execute(&mut *transaction)
        .await
        .expect("profile image fixture should insert");
        sqlx::query(
            r#"
INSERT INTO file_versions (
    id, file_id, version_number, provider_code, storage_class, storage_status,
    object_key, detected_mime_type, canonical_extension, byte_size, checksum,
    scan_status, scanner_result_code, scanned_at, created_by
) VALUES (
    $1, $2, 1, 'test', 'private', 'stored', $3, 'image/jpeg', 'jpg', 1,
    repeat('a', 64), 'clean', 'clean', now(), $4
)
"#,
        )
        .bind(version_id)
        .bind(file_id)
        .bind(format!("profile-test/{}", Uuid::new_v4()))
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .expect("profile image version fixture should insert");
        sqlx::query(
            "UPDATE files
             SET current_version_id = $1, lifecycle_status = 'ready'
             WHERE id = $2",
        )
        .bind(version_id)
        .bind(file_id)
        .execute(&mut *transaction)
        .await
        .expect("profile image fixture should become ready");
        transaction
            .commit()
            .await
            .expect("profile image fixture transaction should commit");
        file_id
    }

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
            profile_image_file_id: None,
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

    #[tokio::test]
    async fn update_profile_preserves_replaces_and_clears_profile_image() {
        let pool = create_test_pool().await;
        run_test_migrations(&pool).await;
        let email = format!("profile-update-{}@test.example", Uuid::new_v4());
        let user_id = create_test_user(&pool, &email, "TestPassword123!")
            .await
            .expect("test user should insert");
        let old_file_id = insert_ready_profile_image(&pool, user_id, false).await;
        let new_file_id = insert_ready_profile_image(&pool, user_id, true).await;
        sqlx::query("UPDATE users SET profile_image_file_id = $1 WHERE id = $2")
            .bind(old_file_id)
            .bind(user_id)
            .execute(&pool)
            .await
            .expect("old profile image should attach");

        let preserved = update_profile(&pool, user_id, profile_update(None))
            .await
            .expect("omitted image should preserve");
        assert_eq!(preserved.user.profile_image_file_id, Some(old_file_id));
        assert_eq!(preserved.replaced_file_id, None);

        let replaced = update_profile(&pool, user_id, profile_update(Some(Some(new_file_id))))
            .await
            .expect("new image should replace old image");
        assert_eq!(replaced.user.profile_image_file_id, Some(new_file_id));
        assert_eq!(replaced.replaced_file_id, Some(old_file_id));
        let promoted = sqlx::query_as::<_, (String, Option<chrono::DateTime<chrono::Utc>>)>(
            "SELECT retention_class, expires_at FROM files WHERE id = $1",
        )
        .bind(new_file_id)
        .fetch_one(&pool)
        .await
        .expect("new image should remain available");
        assert_eq!(promoted, ("standard".to_string(), None));

        let cleared = update_profile(&pool, user_id, profile_update(Some(None)))
            .await
            .expect("explicit null should clear image");
        assert_eq!(cleared.user.profile_image_file_id, None);
        assert_eq!(cleared.replaced_file_id, Some(new_file_id));
    }
}
