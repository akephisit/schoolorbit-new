use crate::error::AppError;
use crate::modules::auth::models::User;
use crate::utils::field_encryption;
use sqlx::PgPool;
use uuid::Uuid;

fn decrypt_national_id_if_possible(national_id: Option<String>) -> Option<String> {
    national_id.map(|value| field_encryption::decrypt(&value).unwrap_or(value))
}

pub async fn get_user(pool: &PgPool, user_id: Uuid) -> Result<User, AppError> {
    let mut user: User = sqlx::query_as(
        "SELECT id, username, national_id, email, password_hash, first_name, last_name,
                user_type, phone, date_of_birth, address, status, metadata, created_at, updated_at,
                title, nickname, emergency_contact, line_id, gender, profile_image_url,
                hired_date, resigned_date
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get user: {}", e);
        AppError::InternalServerError(format!("Database error: {}", e))
    })?;

    user.national_id = decrypt_national_id_if_possible(user.national_id);
    Ok(user)
}

pub type MenuRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    i32,
    i32,
);

pub async fn fetch_menu_items(pool: &PgPool, user_type: &str) -> Result<Vec<MenuRow>, AppError> {
    sqlx::query_as(
        r#"SELECT mi.id, mi.code, mi.name, mi.path, mi.icon, mi.required_permission,
                  mg.code as group_code, mg.name as group_name, mg.icon as group_icon,
                  mg.display_order as group_order, mi.display_order
           FROM menu_items mi
           JOIN menu_groups mg ON mi.group_id = mg.id
           WHERE mi.is_active = true AND mg.is_active = true AND mi.user_type = $1
           ORDER BY mg.display_order, mi.display_order"#,
    )
    .bind(user_type)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch menu items: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลเมนูได้".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_national_id_if_possible_preserves_none() {
        assert_eq!(decrypt_national_id_if_possible(None), None);
    }

    #[test]
    fn decrypt_national_id_if_possible_keeps_plaintext_when_decryption_fails() {
        assert_eq!(
            decrypt_national_id_if_possible(Some("not-encrypted".to_string())),
            Some("not-encrypted".to_string())
        );
    }
}
