use crate::models::{AdminUser, CreateAdminUser, LoginRequest};
use shared::auth::{generate_token, validate_token, hash_password, verify_password, Claims, UserRole};
use shared::error::AppError;
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
        // Hash password
        let password_hash = hash_password(&data.password)?;

        // Create admin user
        let admin = sqlx::query_as::<_, AdminUser>(
            r#"
            INSERT INTO admin_users (email, password_hash, name, role)
            VALUES ($1, $2, $3, 'super_admin')
            RETURNING *
            "#
        )
        .bind(&data.email)
        .bind(&password_hash)
        .bind(&data.name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(admin)
    }

    pub async fn login(&self, data: LoginRequest) -> Result<(AdminUser, String), AppError> {
        // Find user by email
        let admin = sqlx::query_as::<_, AdminUser>(
            "SELECT * FROM admin_users WHERE email = $1"
        )
        .bind(&data.email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

        // Verify password
        let is_valid = verify_password(&data.password, &admin.password_hash)?;
        if !is_valid {
            return Err(AppError::Unauthorized("Invalid email or password".to_string()));
        }

        // Generate JWT
        let claims = Claims {
            sub: admin.id.to_string(),
            email: admin.email.clone(),
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
}
