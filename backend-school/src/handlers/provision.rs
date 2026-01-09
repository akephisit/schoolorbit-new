use crate::models::provision::{ProvisionRequest, ProvisionResponse};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sqlx::postgres::PgPoolOptions;

/// Handler for provisioning a new school tenant database
/// 
/// This endpoint:
/// 1. Connects to the provided database URL
/// 2. Runs all migrations
/// 3. Creates initial admin user with provided credentials
/// 4. Returns success/failure
pub async fn provision_tenant(
    Json(payload): Json<ProvisionRequest>,
) -> Response {
    println!("📦 Provisioning tenant for school: {}", payload.school_id);
    println!("   Subdomain: {}", payload.subdomain);
    println!("   Admin National ID: {}", payload.admin_national_id);

    // Connect to the tenant database
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&payload.db_connection_string)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("❌ Failed to connect to tenant database: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Database connection failed: {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };


    println!("✅ Connected to tenant database");

    // Run migrations
    println!("📦 Running migrations...");
    match sqlx::migrate!("./migrations")
        .run(&pool)
        .await
    {
        Ok(_) => {
            println!("✅ Migrations completed successfully");
        }
        Err(e) => {
            eprintln!("❌ Migration failed: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Migration failed: {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    }

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
    
    let admin_role_id = match sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        SELECT id FROM roles WHERE code = 'ADMIN'
        "#
    )
    .fetch_one(&pool)
    .await
    {
        Ok(id) => {
            println!("✅ Admin role found with ID: {}", id);
            id
        }
        Err(e) => {
            eprintln!("❌ Failed to find admin role: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Failed to find admin role (migrations may not have run): {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

    // Create admin user (must be in transaction for encryption to work)
    println!("👤 Creating admin user...");
    
    // Start transaction
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to start transaction: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": "Failed to start transaction"
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

    // Setup encryption key for encrypted columns (inside transaction)
    println!("🔐 Setting up encryption...");
    
    // Get encryption key from environment
    let encryption_key = match std::env::var("ENCRYPTION_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("❌ ENCRYPTION_KEY not set");
            let error = serde_json::json!({
                "success": false,
                "error": "ENCRYPTION_KEY environment variable not set"
            });
            let _ = tx.rollback().await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };
    
    // SET LOCAL cannot use parameter binding, must use literal value
    // Note: encryption_key should only come from trusted source (environment)
    if let Err(e) = sqlx::query(&format!("SET LOCAL app.encryption_key = '{}'", encryption_key))
        .execute(&mut *tx)
        .await
    {
        eprintln!("❌ Encryption setup failed: {}", e);
        let error = serde_json::json!({
            "success": false,
            "error": format!("Encryption setup failed: {}", e)
        });
        let _ = tx.rollback().await;
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
    }
    
    // Hash the password using bcrypt
    let password_hash = match bcrypt::hash(&payload.admin_password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            eprintln!("❌ Password hashing failed: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": "Password hashing failed"
            });
            let _ = tx.rollback().await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

    // Insert admin user into the database with encrypted national_id
    let user_id = match sqlx::query_scalar::<_, uuid::Uuid>(
        r#"
        INSERT INTO users (national_id, password_hash, first_name, last_name, user_type, status)
        VALUES (
            pgp_sym_encrypt($1, current_setting('app.encryption_key')),
            $2, $3, $4, $5, $6
        )
        ON CONFLICT (national_id) DO UPDATE SET national_id = EXCLUDED.national_id
        RETURNING id
        "#
    )
    .bind(&payload.admin_national_id)
    .bind(&password_hash)
    .bind("ผู้ดูแลระบบ") // Default first name
    .bind(&payload.subdomain) // Use subdomain as last name initially
    .bind("staff") // user_type is 'staff' (admin is determined by role assignment)
    .bind("active")
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => {
            println!("✅ Admin user created successfully");
            println!("   User ID: {}", id);
            println!("   National ID: {} (encrypted)", payload.admin_national_id);
            id
        }
        Err(e) => {
            eprintln!("❌ Failed to create admin user: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Failed to create admin user: {}", e)
            });
            let _ = tx.rollback().await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

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
            let error = serde_json::json!({
                "success": false,
                "error": format!("Failed to assign admin role: {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    }

    // Commit transaction
    if let Err(e) = tx.commit().await {
        eprintln!("❌ Failed to commit transaction: {}", e);
        let error = serde_json::json!({
            "success": false,
            "error": "Failed to commit transaction"
        });
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
    }

    println!("🎉 Tenant provisioning completed for school: {}", payload.school_id);

    let response = ProvisionResponse {
        success: true,
        message: "Tenant database provisioned successfully with admin user".to_string(),
        school_id: payload.school_id,
    };

    (StatusCode::OK, Json(response)).into_response()
}
