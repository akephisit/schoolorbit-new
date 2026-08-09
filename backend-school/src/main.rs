pub mod api_contract;
pub mod api_response;
mod app;
mod db;
pub mod error;
mod middleware;
mod modules;
mod permissions;
mod policies;
mod scheduling;
mod services;
mod utils;

#[cfg(test)]
mod test_helpers;

use crate::modules::notification::events::{
    PermissionChangeEvent, TenantNotificationEvent, WorkChangeEvent,
};
use db::admin_client::{AdminClient, AdminClientConfig};
use db::permission_cache::PermissionCache;
use db::pool_manager::PoolManager;
use dotenvy::dotenv;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio_cron_scheduler::JobScheduler;
use uuid::Uuid;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
    pub websocket_manager: Arc<modules::academic::websockets::WebSocketManager>,
    pub notification_channel: broadcast::Sender<TenantNotificationEvent>,
    pub permission_event_channel: broadcast::Sender<PermissionChangeEvent>,
    pub work_event_channel: broadcast::Sender<WorkChangeEvent>,
    pub permission_cache: Arc<PermissionCache>,
    pub file_platform: Arc<modules::files::platform_service::FilePlatform>,
    pub auth_runtime: modules::auth::runtime::AuthRuntime,
}

impl AppState {
    pub fn notify_permission_changed(&self, tenant: &str, target_user_id: Uuid) {
        let _ = self
            .permission_event_channel
            .send(PermissionChangeEvent::for_user(tenant, target_user_id));
    }

    pub fn notify_all_permissions_changed(&self, tenant: &str) {
        let _ = self
            .permission_event_channel
            .send(PermissionChangeEvent::for_all_users(tenant));
    }

    pub fn notify_work_items_changed(&self, tenant: &str) {
        let _ = self
            .work_event_channel
            .send(WorkChangeEvent::work_items_changed(tenant));
    }

    pub fn notify_workflow_window_changed(&self, tenant: &str) {
        let _ = self
            .work_event_channel
            .send(WorkChangeEvent::workflow_window_changed(tenant));
    }
}

#[tokio::main]
async fn main() {
    let command_args = env::args().skip(1).collect::<Vec<_>>();
    if command_args.first().map(String::as_str) == Some("export-openapi") {
        if command_args.len() != 1 {
            eprintln!("usage: backend-school export-openapi");
            std::process::exit(2);
        }

        match api_contract::render_school_api() {
            Ok(document) => {
                use std::io::Write;
                if let Err(error) = std::io::stdout().write_all(document.as_bytes()) {
                    eprintln!("failed to write OpenAPI document: {error}");
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("failed to render OpenAPI document: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    dotenv().ok();

    if env::var("LOG_FORMAT").as_deref() == Ok("json") {
        utils::logging::init();
    } else {
        utils::logging::init_pretty();
    }

    tracing::info!("🚀 Starting SchoolOrbit Backend School Service...");

    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let backend_admin_url = env::var("BACKEND_ADMIN_URL")
        .expect("BACKEND_ADMIN_URL must be set (e.g. http://backend-admin:8080)");
    let internal_secret = env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set for internal API authentication");

    let admin_client_config = match AdminClientConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(error = %error, "Invalid backend-admin client configuration");
            std::process::exit(1);
        }
    };
    let admin_client = Arc::new(AdminClient::new(
        backend_admin_url,
        internal_secret,
        admin_client_config,
    ));
    tracing::info!("✅ Admin client initialized (HTTP-based school mapping)");

    let pool_manager = Arc::new(PoolManager::new());
    let websocket_manager = Arc::new(modules::academic::websockets::WebSocketManager::new());
    websocket_manager.clone().spawn_cleanup_task();

    let (notification_tx, _) = broadcast::channel(100);
    let (permission_event_tx, _) = broadcast::channel(100);
    let (work_event_tx, _) = broadcast::channel(100);
    let (session_event_tx, _) = broadcast::channel(100);

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

    let storage_provider = match modules::files::r2_storage_provider::R2StorageProvider::new().await
    {
        Ok(provider) => Arc::new(provider),
        Err(error) => {
            tracing::error!(
                error_code = error.log_safe_code(),
                "File Platform storage configuration is invalid"
            );
            std::process::exit(1);
        }
    };
    let scanner_config = match modules::files::malware_scanner::ClamdConfig::from_env() {
        Ok(config) => config,
        Err(_) => {
            tracing::error!("File Platform malware scanner configuration is invalid");
            std::process::exit(1);
        }
    };
    let file_runtime_config =
        match modules::files::runtime_config::FilePlatformRuntimeConfig::from_env() {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(
                    error_code = error.log_safe_code(),
                    "File Platform runtime configuration is invalid"
                );
                std::process::exit(1);
            }
        };
    let file_platform = Arc::new(modules::files::platform_service::FilePlatform::with_config(
        storage_provider,
        Arc::new(modules::files::malware_scanner::ClamdScanner::new(
            scanner_config,
        )),
        file_runtime_config,
    ));

    let session_config = match modules::auth::config::SessionConfig::from_env() {
        Ok(config) => Arc::new(config),
        Err(_) => {
            tracing::error!(reason = "session_config_invalid");
            std::process::exit(1);
        }
    };
    let permission_cache = Arc::new(PermissionCache::new());
    let auth_runtime = modules::auth::runtime::AuthRuntime {
        admin_client: Arc::clone(&admin_client),
        pool_manager: Arc::clone(&pool_manager),
        permission_cache: Arc::clone(&permission_cache),
        config: session_config,
        session_events: session_event_tx,
    };

    let state = AppState {
        admin_client,
        pool_manager,
        websocket_manager,
        notification_channel: notification_tx,
        permission_event_channel: permission_event_tx,
        work_event_channel: work_event_tx,
        permission_cache,
        file_platform,
        auth_runtime,
    };
    let app = app::build_app(state.clone());

    let addr = format!("{}:{}", host, port);
    tracing::info!("🌐 Server starting on http://{}", addr);
    tracing::info!("\n✅ Available endpoints:");
    tracing::info!("  GET  /                          - API info");
    tracing::info!("  GET  /health                    - Health check");
    tracing::info!("  GET  /ready                     - Control-plane readiness");
    tracing::info!("  POST /api/auth/login            - Login");
    tracing::info!("  POST /api/auth/logout           - Logout");
    tracing::info!("  GET  /api/auth/me               - Get current user (protected)\n");
    tracing::info!("  Staff & Student Management:");
    tracing::info!("  /api/staff/*                    - Staff, Roles, Organization");
    tracing::info!("  /api/students/*                 - Student Management\n");
    tracing::info!("  Internal Admin APIs (Protected by Secret):");
    tracing::info!("  POST /internal/provision        - Provision tenant database");
    tracing::info!("  POST /internal/migrate-all      - Migrate all school databases");
    tracing::info!("  GET  /internal/migration-status - Get migration status");
    tracing::info!("  GET  /ws/timetable              - Real-time Timetable Collaboration");

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(address = %addr, error = %error, "Failed to bind server listener");
            std::process::exit(1);
        }
    };

    let mut sched = JobScheduler::new()
        .await
        .expect("Failed to initialize job scheduler");
    let admin_client_for_job = Arc::clone(&state.admin_client);
    let pool_manager_for_job = Arc::clone(&state.pool_manager);
    let file_platform_for_job = Arc::clone(&state.file_platform);

    let cleaner_job = scheduling::new_school_cron_job(
        scheduling::FILE_PLATFORM_RECONCILIATION_CRON,
        move |_uuid, _l| {
            let admin_client = Arc::clone(&admin_client_for_job);
            let pool_manager = pool_manager_for_job.clone();
            let file_platform = Arc::clone(&file_platform_for_job);

            Box::pin(async move {
                tracing::info!("Starting scheduled File Platform reconciliation");
                let schools = match admin_client.list_active_schools().await {
                    Ok(schools) => schools,
                    Err(error) => {
                        tracing::error!("Failed to fetch schools list for cleanup: {}", error);
                        return;
                    }
                };

                tracing::info!("Found {} active schools to reconcile.", schools.len());
                for school in schools {
                    let db_url = match school.db_connection_string {
                        Some(ref url) if !url.is_empty() => url.clone(),
                        _ => {
                            tracing::warn!(
                                "Skipping school '{}': no database URL",
                                school.subdomain
                            );
                            continue;
                        }
                    };

                    tracing::info!("Reconciling File Platform operations for tenant");
                    match pool_manager.get_pool(&db_url, &school.subdomain).await {
                        Ok(pool) => {
                            let cleaner = services::cleaner::FileCleaner::new(
                                pool,
                                Arc::clone(&file_platform),
                            );
                            cleaner.reconcile_file_operations().await;
                        }
                        Err(error) => {
                            tracing::error!(
                                "Failed to get database connection for {}: {}",
                                school.subdomain,
                                error
                            );
                        }
                    }
                }
                tracing::info!("Scheduled File Platform reconciliation completed");
            })
        },
    )
    .expect("Failed to create cleaner job");

    let admin_client_for_calendar_job = Arc::clone(&state.admin_client);
    let pool_manager_for_calendar_job = Arc::clone(&state.pool_manager);
    let notification_channel_for_calendar_job = state.notification_channel.clone();
    let calendar_reminder_job =
        scheduling::new_school_cron_job(scheduling::CALENDAR_REMINDER_CRON, move |_uuid, _l| {
            let admin_client = Arc::clone(&admin_client_for_calendar_job);
            let pool_manager = Arc::clone(&pool_manager_for_calendar_job);
            let notification_channel = notification_channel_for_calendar_job.clone();

            Box::pin(async move {
                modules::calendar::services::process_due_calendar_reminders_for_all_tenants(
                    admin_client,
                    pool_manager,
                    notification_channel,
                )
                .await;
            })
        })
        .expect("Failed to create calendar reminder job");

    let cleaner_job_id = cleaner_job.guid();
    let calendar_reminder_job_id = calendar_reminder_job.guid();
    sched
        .add(cleaner_job)
        .await
        .expect("Failed to add job to scheduler");
    sched
        .add(calendar_reminder_job)
        .await
        .expect("Failed to add calendar reminder job");

    let cleaner_next_run = scheduling::next_run_for_job(&mut sched, cleaner_job_id)
        .await
        .expect("Failed to resolve next File Platform reconciliation");
    scheduling::log_next_run(
        "file_platform_reconciliation",
        scheduling::FILE_PLATFORM_RECONCILIATION_CRON,
        cleaner_next_run,
    );
    let calendar_next_run = scheduling::next_run_for_job(&mut sched, calendar_reminder_job_id)
        .await
        .expect("Failed to resolve next calendar reminder");
    scheduling::log_next_run(
        "calendar_reminder",
        scheduling::CALENDAR_REMINDER_CRON,
        calendar_next_run,
    );
    sched.start().await.expect("Failed to start scheduler");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server failed");
}
