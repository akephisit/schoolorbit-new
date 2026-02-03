mod db;
mod middleware;
mod permissions;
mod services;
mod utils;
mod modules;
pub mod error;

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
use tokio::sync::broadcast;
use crate::modules::notification::handlers::Notification;
use std::env;
use std::sync::Arc;
use uuid::Uuid;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_cookies::CookieManagerLayer;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub admin_pool: sqlx::PgPool,  // Backend-admin database (for school mapping)
    pub pool_manager: Arc<PoolManager>,
    pub websocket_manager: Arc<modules::academic::websockets::WebSocketManager>,
    pub notification_channel: broadcast::Sender<(Uuid, Notification)>, // (User ID, Notification)
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
    let websocket_manager = Arc::new(modules::academic::websockets::WebSocketManager::new());
    
    // Notification broadcast channel (capacity 100)
    let (notification_tx, _) = broadcast::channel(100);

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
        websocket_manager,
        notification_channel: notification_tx,
    };

    // Build application
    let app = Router::new()
        // Public routes
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        
        // Auth routes (public)
        .route("/api/auth/login", post(modules::auth::handlers::login))
        .route("/api/auth/logout", post(modules::auth::handlers::logout))
        
        // Protected auth routes
        .route("/api/auth/me", get(modules::auth::handlers::me)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/auth/me/profile", get(modules::auth::handlers::get_profile)
            .put(modules::auth::handlers::update_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/auth/me/change-password", post(modules::auth::handlers::change_password)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Staff Management routes (protected)
        .route("/api/staff", get(modules::staff::handlers::staff::list_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", get(modules::staff::handlers::staff::get_staff_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}/public-profile", get(modules::staff::handlers::staff::get_public_staff_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff", post(modules::staff::handlers::staff::create_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", axum::routing::put(modules::staff::handlers::staff::update_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/staff/{id}", axum::routing::delete(modules::staff::handlers::staff::delete_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Staff Achievements routes (protected)
        .route("/api/achievements", get(modules::achievement::handlers::list_achievements)
            .post(modules::achievement::handlers::create_achievement)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/achievements/{id}", axum::routing::put(modules::achievement::handlers::update_achievement)
            .delete(modules::achievement::handlers::delete_achievement)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Student Self-Service routes (protected)
        .route("/api/student/profile", get(modules::students::handlers::get_own_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/student/profile", axum::routing::put(modules::students::handlers::update_own_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Parent Self-Service routes (protected)
        .route("/api/parent/profile", get(modules::parents::handlers::get_own_parent_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/parent/students/{student_id}", get(modules::parents::handlers::get_child_profile)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Student Management routes (protected - for admin/staff)
        .route("/api/students", get(modules::students::handlers::list_students)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students", post(modules::students::handlers::create_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", get(modules::students::handlers::get_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", axum::routing::put(modules::students::handlers::update_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}", axum::routing::delete(modules::students::handlers::delete_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        // Parent Management (Nested for simplicity)
        .route("/api/students/{id}/parents", post(modules::students::handlers_parents::add_parent_to_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/students/{id}/parents/{parent_id}", axum::routing::delete(modules::students::handlers_parents::remove_parent_from_student)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Role Management routes (protected)
        .route("/api/roles", get(modules::staff::handlers::roles::list_roles)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles/{id}", get(modules::staff::handlers::roles::get_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles", post(modules::staff::handlers::roles::create_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/roles/{id}", axum::routing::put(modules::staff::handlers::roles::update_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Department Management routes (protected)
        .route("/api/departments", get(modules::staff::handlers::roles::list_departments)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments/{id}", get(modules::staff::handlers::roles::get_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments", post(modules::staff::handlers::roles::create_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments/{id}", axum::routing::put(modules::staff::handlers::roles::update_department)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/departments/{id}/permissions", get(modules::staff::handlers::department_permissions::get_department_permissions)
            .put(modules::staff::handlers::department_permissions::update_department_permissions)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // User Role Assignment routes (protected)
        .route("/api/users/{id}/roles", get(modules::staff::handlers::user_roles::get_user_roles)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/roles", post(modules::staff::handlers::user_roles::assign_user_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/roles/{role_id}", axum::routing::delete(modules::staff::handlers::user_roles::remove_user_role)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/users/{id}/permissions", get(modules::staff::handlers::user_roles::get_user_permissions)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Permissions Master Data routes (protected)
        .route("/api/permissions", get(modules::staff::handlers::permissions::list_permissions)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/permissions/modules", get(modules::staff::handlers::permissions::list_permissions_by_module)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Menu routes (protected)
        .route("/api/menu/user", get(modules::menu::handlers::public::get_user_menu)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Admin - Feature Toggles (protected)
        .route("/api/admin/features", get(modules::system::handlers::feature_toggles::list_features)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}", get(modules::system::handlers::feature_toggles::get_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}", axum::routing::put(modules::system::handlers::feature_toggles::update_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/features/{id}/toggle", post(modules::system::handlers::feature_toggles::toggle_feature)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        
        // Admin - Menu Management (protected, module-based permissions)
        .route("/api/admin/menu/groups", get(modules::menu::handlers::admin::list_menu_groups)
            .post(modules::menu::handlers::admin::create_menu_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/groups/{id}", axum::routing::put(modules::menu::handlers::admin::update_menu_group)
            .delete(modules::menu::handlers::admin::delete_menu_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/groups/reorder", post(modules::menu::handlers::admin::reorder_menu_groups)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items", get(modules::menu::handlers::admin::list_menu_items)
            .post(modules::menu::handlers::admin::create_menu_item)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/{id}", axum::routing::put(modules::menu::handlers::admin::update_menu_item)
            .delete(modules::menu::handlers::admin::delete_menu_item)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/{id}/group", axum::routing::put(modules::menu::handlers::admin::move_item_to_group)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/admin/menu/items/reorder", post(modules::menu::handlers::admin::reorder_menu_items)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Consent Management routes (PDPA Compliance)
        .route("/api/consent/types", get(modules::consent::handlers::get_consent_types))
        .route("/api/consent/my-status", get(modules::consent::handlers::get_my_consent_status)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent", post(modules::consent::handlers::create_consent)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent/{id}/withdraw", post(modules::consent::handlers::withdraw_consent)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/consent/summary", get(modules::consent::handlers::get_consent_summary)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // File Management routes (protected)
        .route("/api/files/upload", post(modules::files::handlers::upload_file)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/files", get(modules::files::handlers::list_user_files)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/files/{id}", axum::routing::delete(modules::files::handlers::delete_file)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Academic Management routes (Protected)
        .nest("/api/academic", modules::academic::academic_routes()
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Facility Management routes (Protected)
        .nest("/api/facilities", modules::facility::facility_routes()
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Notification routes (Protected)
        .route("/api/notifications/stream", get(modules::notification::handlers::stream_notifications)) // Auth handled inside to get user ID, or use middleware?
                                                                                                        // Standard middleware might buffer or cause issues with SSE if not careful?
                                                                                                        // Actually `auth_middleware` just parses token.
            // .layer(axum_middleware::from_fn(middleware::auth::auth_middleware))) // Let's keep it consistent.
        .route("/api/notifications", 
            get(modules::notification::handlers::list_notifications)
            .post(modules::notification::handlers::create_notification)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/notifications/read-all", post(modules::notification::handlers::mark_all_as_read)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/notifications/{id}/read", post(modules::notification::handlers::mark_as_read)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/notifications/subscribe", post(modules::notification::handlers::subscribe_push)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Lookup endpoints (Protected - only requires authentication, no specific permission)
        // These return minimal data for dropdowns (id, name only)
        .route("/api/lookup/staff", get(modules::lookup::handlers::lookup_staff)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/students", get(modules::lookup::handlers::lookup_students)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/rooms", get(modules::lookup::handlers::lookup_rooms)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/roles", get(modules::lookup::handlers::lookup_roles)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/departments", get(modules::lookup::handlers::lookup_departments)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/grade-levels", get(modules::lookup::handlers::lookup_grade_levels)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/classrooms", get(modules::lookup::handlers::lookup_classrooms)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/academic-years", get(modules::lookup::handlers::lookup_academic_years)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        .route("/api/lookup/subjects", get(modules::lookup::handlers::lookup_subjects)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))

        // Route registration (no auth - uses deploy key)
        .route("/api/admin/routes/register", post(modules::system::handlers::register_routes::register_routes))
        
        // WebSocket Route (No standard middleware auth, uses Query Params)
        .route("/ws/timetable", get(modules::academic::websockets::timetable_websocket_handler))
        
        
        
        // Internal routes (protected by internal auth middleware)
        .route(
            "/internal/provision",
            post(modules::system::handlers::provision::provision_tenant)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        .route(
            "/internal/migrate-all",
            post(modules::system::handlers::migration::migrate_all_schools)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        .route(
            "/internal/migration-status",
            get(modules::system::handlers::migration::migration_status)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
    // Add cookie middleware
        .layer(CookieManagerLayer::new())
        // Increase request body limit to 20MB for file uploads
        .layer(axum::extract::DefaultBodyLimit::max(20 * 1024 * 1024))
        // Add state
        .with_state(state.clone());

    let addr = format!("{}:{}", host, port);
    tracing::info!("🌐 Server starting on http://{}", addr);
    tracing::info!("\n✅ Available endpoints:");
    tracing::info!("  GET  /                          - API info");
    tracing::info!("  GET  /health                    - Health check");
    tracing::info!("  POST /api/auth/login            - Login");
    tracing::info!("  POST /api/auth/logout           - Logout");
    tracing::info!("  GET  /api/auth/me               - Get current user (protected)\n");
    tracing::info!("  Staff & Student Management:");
    tracing::info!("  /api/staff/*                    - Staff, Roles, Departments");
    tracing::info!("  /api/students/*                 - Student Management\n");
    tracing::info!("  Internal Admin APIs (Protected by Secret):");
    tracing::info!("  POST /internal/provision        - Provision tenant database");
    tracing::info!("  POST /internal/migrate-all      - Migrate all school databases");
    tracing::info!("  POST /internal/migrate-all      - Migrate all school databases");
    tracing::info!("  GET  /internal/migration-status - Get migration status");
    tracing::info!("  GET  /ws/timetable              - Real-time Timetable Collaboration");


    // Run server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect(&format!("Failed to bind to {}", addr));

    // Initialize Job Scheduler for background tasks
    // Run daily cleaning at 3:00 AM
    let sched = JobScheduler::new().await.unwrap();
    
    // Clone shared resources for the job
    let admin_pool_for_job = state.admin_pool.clone();
    let pool_manager_for_job = Arc::clone(&state.pool_manager);

    let cleaner_job = Job::new_async("0 0 3 * * *", move |_uuid, _l| {
        let admin_pool = admin_pool_for_job.clone();
        let pool_manager = pool_manager_for_job.clone();
        
        Box::pin(async move {
            tracing::info!("⏰ Starting scheduled file cleanup job (Garbage Collection)...");
            
            // 1. Get List of all schools to clean
            // We need database_url to establish connection via pool_manager
            #[derive(sqlx::FromRow)]
            struct SchoolInfo {
                subdomain: String,
                db_connection_string: String,
            }

            let schools = match sqlx::query_as::<_, SchoolInfo>(
                "SELECT subdomain, db_connection_string FROM schools WHERE status = 'active' AND db_connection_string IS NOT NULL"
            )
            .fetch_all(&admin_pool)
            .await 
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to fetch schools list for cleanup: {}", e);
                    return;
                }
            };

            tracing::info!("Found {} active schools to clean.", schools.len());

            for school in schools {
                tracing::info!("🧹 Cleaning school tenant: {}", school.subdomain);
                
                // 2. Get Connection Pool (Reuse existing logic)
                match pool_manager.get_pool(&school.db_connection_string, &school.subdomain).await {
                    Ok(pool) => {
                        // 3. Run Cleaner Service
                        match services::cleaner::FileCleaner::new(pool).await {
                            Ok(cleaner) => {
                                cleaner.clean_orphaned_files().await;
                            },
                            Err(e) => {
                                tracing::error!("Failed to initialize FileCleaner for {}: {}", school.subdomain, e);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to get database connection for {}: {}", school.subdomain, e);
                    }
                }
            }
            tracing::info!("✅ Scheduled cleanup job completed for all tenants.");
        })
    }).expect("Failed to create cleaner job");

    sched.add(cleaner_job).await.expect("Failed to add job to scheduler");
    sched.start().await.expect("Failed to start scheduler");

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
