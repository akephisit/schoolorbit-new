mod db;
mod handlers;
mod middleware;
mod models;
mod permissions;
mod utils;

#[cfg(test)]
mod test_helpers;


use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
    Json,
};
use db::pool_manager::PoolManager;
use dotenv::dotenv;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tower_cookies::CookieManagerLayer;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub admin_pool: sqlx::PgPool,  // Backend-admin database (for school mapping)
    pub pool_manager: Arc<PoolManager>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    // Initialize structured logging
    utils::logging::init_pretty();
    
    tracing::info!("🚀 Starting SchoolOrbit Backend School Service...");

    // Get environment variables
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    
    // Connect to backend-admin database for school mapping
    let admin_database_url = env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must be set (backend-admin database for school mapping)");

    // Verify internal secret is set
    env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set for internal API authentication");

    tracing::info!("📦 Connecting to admin database for school mapping...");
    let admin_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&admin_database_url)
        .await
        .expect("Failed to connect to admin database");

    tracing::info!("✅ Admin database connected");

    // Create pool manager for tenant databases
    let pool_manager = Arc::new(PoolManager::new());
    
    // Start cleanup task
    let pool_manager_cleanup = Arc::clone(&pool_manager);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            pool_manager_cleanup.cleanup_expired().await;
        }
    });

    tracing::info!("✅ Pool manager initialized");
    tracing::info!("ℹ️  Multi-tenant architecture ready");
    tracing::info!("ℹ️  Each school has its own database connection pool (cached)");

    // Create shared state
    let state = AppState {
        admin_pool,
        pool_manager,
    };

    // Build application
    let app = Router::new()
        // Public routes
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        
        // Auth routes (public)
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        
        // Protected auth routes
        .route("/api/auth/me", get(handlers::auth::me)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/auth/me/profile", get(handlers::auth::get_profile)
            .put(handlers::auth::update_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/auth/me/change-password", post(handlers::auth::change_password)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Staff Management routes (protected)
        .route("/api/staff", get(handlers::staff::list_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", get(handlers::staff::get_staff_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff", post(handlers::staff::create_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", axum::routing::put(handlers::staff::update_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", axum::routing::delete(handlers::staff::delete_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Student Self-Service routes (protected)
        .route("/api/student/profile", get(handlers::students::get_own_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/student/profile", axum::routing::put(handlers::students::update_own_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Student Management routes (protected - for admin/staff)
        .route("/api/students", get(handlers::students::list_students)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students", post(handlers::students::create_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", get(handlers::students::get_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", axum::routing::put(handlers::students::update_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", axum::routing::delete(handlers::students::delete_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Role Management routes (protected)
        .route("/api/roles", get(handlers::roles::list_roles)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles/{id}", get(handlers::roles::get_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles", post(handlers::roles::create_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles/{id}", axum::routing::put(handlers::roles::update_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Department Management routes (protected)
        .route("/api/departments", get(handlers::roles::list_departments)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments/{id}", get(handlers::roles::get_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments", post(handlers::roles::create_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments/{id}", axum::routing::put(handlers::roles::update_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // User Role Assignment routes (protected)
        .route("/api/users/{id}/roles", get(handlers::user_roles::get_user_roles)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/roles", post(handlers::user_roles::assign_user_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/roles/{role_id}", axum::routing::delete(handlers::user_roles::remove_user_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/permissions", get(handlers::user_roles::get_user_permissions)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Permissions Master Data routes (protected)
        .route("/api/permissions", get(handlers::permissions::list_permissions)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/permissions/modules", get(handlers::permissions::list_permissions_by_module)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Menu routes (protected)
        .route("/api/menu/user", get(handlers::menu::get_user_menu)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Admin - Feature Toggles (protected)
        .route("/api/admin/features", get(handlers::feature_toggles::list_features)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}", get(handlers::feature_toggles::get_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}", axum::routing::put(handlers::feature_toggles::update_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}/toggle", post(handlers::feature_toggles::toggle_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        
        // Admin - Menu Management (protected, module-based permissions)
        .route("/api/admin/menu/groups", get(handlers::menu_admin::list_menu_groups)
            .post(handlers::menu_admin::create_menu_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/groups/{id}", axum::routing::put(handlers::menu_admin::update_menu_group)
            .delete(handlers::menu_admin::delete_menu_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/groups/reorder", post(handlers::menu_admin::reorder_menu_groups)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items", get(handlers::menu_admin::list_menu_items)
            .post(handlers::menu_admin::create_menu_item)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/{id}", axum::routing::put(handlers::menu_admin::update_menu_item)
            .delete(handlers::menu_admin::delete_menu_item)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/{id}/group", axum::routing::put(handlers::menu_admin::move_item_to_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/reorder", post(handlers::menu_admin::reorder_menu_items)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Consent Management routes (PDPA Compliance)
        .route("/api/consent/types", get(handlers::consent::get_consent_types))
        .route("/api/consent/my-status", get(handlers::consent::get_my_consent_status)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent", post(handlers::consent::create_consent)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent/:id/withdraw", post(handlers::consent::withdraw_consent)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent/summary", get(handlers::consent::get_consent_summary)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Route registration (no auth - uses deploy key)
        .route("/api/admin/routes/register", post(handlers::register_routes::register_routes))
        
        
        // Internal routes (protected by internal auth middleware)
        .route(
            "/internal/provision",
            post(handlers::provision::provision_tenant)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        .route(
            "/internal/migrate-all",
            post(handlers::migration::migrate_all_schools)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        .route(
            "/internal/migration-status",
            get(handlers::migration::migration_status)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        // Add cookie middleware
        .layer(CookieManagerLayer::new())
        // Add state
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("🌐 Server starting on http://{}", addr);
    tracing::info!("\nAvailable endpoints:");
    tracing::info!("  GET  /                    - API info");
    tracing::info!("  GET  /health              - Health check");
    tracing::info!("  POST /api/auth/login      - Login");
    tracing::info!("  POST /api/auth/logout     - Logout");
    tracing::info!("  GET  /api/auth/me         - Get current user (protected)");
    tracing::info!("\n  Staff Management:");
    tracing::info!("  GET    /api/staff         - List all staff (protected)");
    tracing::info!("  GET    /api/staff/{{id}}    - Get staff profile (protected)");
    tracing::info!("  POST   /api/staff         - Create staff (protected)");
    tracing::info!("  PUT    /api/staff/{{id}}    - Update staff (protected)");
    tracing::info!("  DELETE /api/staff/{{id}}    - Delete staff (protected)");
    tracing::info!("\n  Internal APIs:");
    tracing::info!("  POST /internal/provision  - Provision tenant database (internal only)");
    tracing::info!("  POST /internal/migrate-all - Migrate all school databases (internal only)");
    tracing::info!("  GET  /internal/migration-status - Get migration status (internal only)");


    // Run server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect(&format!("Failed to bind to {}", addr));

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

// Handler functions
async fn root_handler() -> Json<serde_json::Value> {
    Json(json!({
        "service": "SchoolOrbit Backend School",
        "version": "0.2.0",
        "status": "running",
        "architecture": "multi-tenant with dynamic connection pools"
    }))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
