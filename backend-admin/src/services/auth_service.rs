use crate::models::{AdminUser, CreateAdminUser, LoginRequest};
use crate::auth::{generate_token, hash_password, verify_password, Claims, UserRole};
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub struct AuthService {
    pool: PgPool,
}

impl AuthService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_admin(&self, data: CreateAdminUser) -> Result<AdminUser, AppError> {
        // Validate national ID (13 digits)
        if !Self::validate_national_id(&data.national_id) {
            return Err(AppError::ValidationError(
                "Invalid national ID format. Must be 13 digits.".to_string()
            ));
        }

        // Hash password
        let password_hash = hash_password(&data.password)?;

        // Create admin user
        let admin = sqlx::query_as::<_, AdminUser>(
            r#"
            INSERT INTO admin_users (national_id, password_hash, name, role)
            VALUES ($1, $2, $3, 'super_admin')
            RETURNING *
            "#
        )
        .bind(&data.national_id)
        .bind(&password_hash)
        .bind(&data.name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(admin)
    }

    pub async fn login(&self, data: LoginRequest) -> Result<(AdminUser, String), AppError> {
        // Validate national ID format
        if !Self::validate_national_id(&data.national_id) {
            return Err(AppError::ValidationError(
                "Invalid national ID format".to_string()
            ));
        }

        // Find user by national_id
        let admin = sqlx::query_as::<_, AdminUser>(
            "SELECT * FROM admin_users WHERE national_id = $1"
        )
        .bind(&data.national_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Invalid national ID or password".to_string()))?;

        // Verify password
        let is_valid = verify_password(&data.password, &admin.password_hash)?;
        if !is_valid {
            return Err(AppError::Unauthorized("Invalid national ID or password".to_string()));
        }

        // Generate JWT
        let claims = Claims {
            sub: admin.id.to_string(),
            email: admin.national_id.clone(), // Use national_id in email field for compatibility
            role: UserRole::SuperAdmin,
            school_id: None,
            subdomain: None,
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
        };

        let token = generate_token(claims)?;

        Ok((admin, token))
    }

    pub async fn get_admin_by_id(&self, id: Uuid) -> Result<AdminUser, AppError> {
        let admin = sqlx::query_as::<_, AdminUser>(
            "SELECT * FROM admin_users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Admin user not found".to_string()))?;

        Ok(admin)
    }

    // Validate National ID
    // Supports:
    // 1. Thai National ID: 13 digits (e.g., 1234567890123)
    // 2. Foreign ID with G prefix: G + 12 digits (e.g., G123456789012)
    fn validate_national_id(national_id: &str) -> bool {
        // Must be exactly 13 characters
        if national_id.len() != 13 {
            return false;
        }

        // Check if it's a G-prefixed ID (for foreigners)
        if national_id.starts_with('G') || national_id.starts_with('g') {
            // G followed by 12 digits
            let digits_part = &national_id[1..];
            return digits_part.len() == 12 && digits_part.chars().all(|c| c.is_ascii_digit());
        }

        // Regular Thai National ID: all 13 characters must be digits
        national_id.chars().all(|c| c.is_ascii_digit())
    }
}
