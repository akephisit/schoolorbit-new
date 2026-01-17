use crate::modules::system::models::{ProvisionRequest, ProvisionResponse};
use crate::error::AppError;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::postgres::PgPoolOptions;
use chrono::Datelike;

/// Handler for provisioning a new school tenant database
/// 
/// This endpoint:
/// 1. Connects to the provided database URL
/// 2. Runs all migrations
/// 3. Creates initial admin user with provided credentials
/// 4. Returns success/failure
pub async fn provision_tenant(
    Json(payload): Json<ProvisionRequest>,
) -> Result<impl IntoResponse, AppError> {
    println!("📦 Provisioning tenant for school: {}", payload.school_id);
    println!("   Subdomain: {}", payload.subdomain);
    println!("   Admin Username: {:?}", payload.admin_username);

    // Connect to the tenant database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&payload.db_connection_string)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to connect to tenant database: {}", e);
             AppError::InternalServerError(format!("Database connection failed: {}", e))
        })?;

    println!("✅ Connected to tenant database");

    // Run migrations
    println!("📦 Running migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Migration failed: {}", e);
            AppError::InternalServerError(format!("Migration failed: {}", e))
        })?;

    println!("✅ Migrations completed successfully");

    // Sync permissions immediately after migrations
    println!("🔄 Syncing permissions...");
    match crate::utils::permission_sync::sync_permissions(&pool).await {
        Ok(_) => {
            println!("✅ Permissions synced successfully");
        }
        Err(e) => {
            eprintln!("⚠️  Failed to sync permissions: {}", e);
            // Continue anyway - permissions will sync on first request
        }
    }

    // Get admin role (created by migration)
    println!("🔍 Getting admin role...");
    
    let admin_role_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        SELECT id FROM roles WHERE code = 'ADMIN'
        "#
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to find admin role: {}", e);
        AppError::InternalServerError(format!("Failed to find admin role (migrations may not have run): {}", e))
    })?;

    // Create admin user (must be in transaction for encryption to work)
    println!("👤 Creating admin user...");
    
    // Start transaction
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("Failed to start transaction".to_string())
    })?;

    // Hash the password using bcrypt
    let password_hash = bcrypt::hash(&payload.admin_password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            eprintln!("❌ Password hashing failed: {}", e);
            AppError::InternalServerError("Password hashing failed".to_string())
        })?;

    // Generate running number for staff code (Admin is the first staff)
    // Pattern: T + Year(2) + Running(4) e.g., T670001
    // Since this is provisioning, it SHOULD be the first user, but we'll count to be safe/consistent
    let thai_year = (chrono::Utc::now().year() + 543) % 100;
    
    // We can't query count yet inside the transaction easily if we want to be atomic with insert in the same way,
    // but here we are in a special "provisioning" state where we expect to be the first.
    // However, to reuse the logic exactly, let's query count.
    
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE user_type = 'staff'")
        .fetch_one(&pool).await.unwrap_or(0);
        
    let username = format!("T{}{:04}", thai_year, count + 1);
    println!("   Generated Admin Username: {}", username);

    // Insert admin user into the database
    // Use username for uniqueness check (unique index on username should exist)
    let user_id = sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        INSERT INTO users (username, national_id, national_id_hash, password_hash, title, first_name, last_name, user_type, status)
        VALUES ($1, NULL, NULL, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (username) DO UPDATE SET 
            password_hash = EXCLUDED.password_hash
        RETURNING id
        "#
    )
    .bind(&username)
    .bind(&password_hash)
    .bind(&payload.admin_title)
    .bind(&payload.admin_first_name)
    .bind(&payload.admin_last_name)
    .bind("staff") // user_type is 'staff'
    .bind("active")
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create admin user: {}", e);
        AppError::InternalServerError(format!("Failed to create admin user: {}", e))
    })?;

    // Assign admin role to the user
    println!("🔑 Assigning admin role to user...");
    match sqlx::query(
        r#"
        INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
        VALUES ($1, $2, $3, CURRENT_DATE)
        ON CONFLICT (user_id, role_id, started_at) DO NOTHING
        "#
    )
    .bind(user_id)
    .bind(admin_role_id)
    .bind(true) // is_primary = true for admin role
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {
            println!("✅ Admin role assigned successfully");
        }
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to assign admin role: {}", e);
            let _ = tx.rollback().await;
            return Err(AppError::InternalServerError(format!("Failed to assign admin role: {}", e)));
        }
    }

    // Commit transaction
    if let Err(e) = tx.commit().await {
        eprintln!("❌ Failed to commit transaction: {}", e);
        return Err(AppError::InternalServerError("Failed to commit transaction".to_string()));
    }

    println!("🎉 Tenant provisioning completed for school: {}", payload.school_id);

    let response = ProvisionResponse {
        success: true,
        message: format!("Tenant database provisioned successfully. Admin Username: {}", username),
        school_id: payload.school_id,
    };

    Ok((StatusCode::OK, Json(response)))
}
