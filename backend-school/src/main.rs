#![deny(dead_code, unused_imports)]

mod adapters;
#[cfg(feature = "api-docs")]
pub mod api_contract;
mod app;
#[cfg(test)]
mod db;
mod http_state;
#[cfg(test)]
mod integration_tests;
mod middleware;
mod system;

mod scheduling;
mod services;
mod startup;
mod utils;

use dotenvy::dotenv;
use school_authorization::PermissionCache;
use school_file_platform::{
    malware_scanner::{ClamdConfig, ClamdScanner},
    platform_service::FilePlatform,
    r2_storage_provider::R2StorageProvider,
    runtime_config::FilePlatformRuntimeConfig,
};
use school_notifications::events::{TenantNotificationEvent, WorkChangeEvent};
use school_tenancy::{AdminClient, AdminClientConfig, PoolManager, PoolManagerConfig};
use startup::{required_environment, StartupError};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio_cron_scheduler::JobScheduler;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
    pub websocket_manager: Arc<school_academic_http::websockets::WebSocketManager>,
    pub notification_channel: broadcast::Sender<TenantNotificationEvent>,

    pub work_event_channel: broadcast::Sender<WorkChangeEvent>,

    pub file_platform: Arc<FilePlatform>,
    pub certificate_verification_limiter:
        Arc<school_certificates::verification_limiter::CertificateVerificationLimiter>,
    pub auth_runtime: school_auth::runtime::AuthRuntime,
}

impl AppState {
    pub fn invalidate_permission_tenant(&self, tenant: &str) {
        self.auth_runtime.invalidate_permission_tenant(tenant);
    }
    pub fn notify_all_permissions_changed(&self, tenant: &str) {
        self.auth_runtime.notify_all_permissions_changed(tenant);
    }
}

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    let command_args = env::args_os()
        .skip(1)
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| StartupError::new("arguments", "invalid_unicode"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if command_args.first().map(String::as_str) == Some("export-openapi") {
        if command_args.len() != 1 {
            eprintln!("usage: backend-school export-openapi");
            std::process::exit(2);
        }

        #[cfg(not(feature = "api-docs"))]
        {
            eprintln!("OpenAPI export requires building backend-school with --features api-docs");
            std::process::exit(2);
        }

        #[cfg(feature = "api-docs")]
        {
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
            return Ok(());
        }
    }
    let result = run().await;
    if let Err(error) = &result {
        tracing::error!(
            stage = error.stage(),
            reason = error.reason(),
            "School startup or server failed"
        );
    }
    result
}

async fn run() -> Result<(), StartupError> {
    dotenv().ok();

    if env::var("LOG_FORMAT").as_deref() == Ok("json") {
        utils::logging::init()
            .map_err(|_| StartupError::new("logging", "subscriber_install_failed"))?;
    } else {
        utils::logging::init_pretty()
            .map_err(|_| StartupError::new("logging", "subscriber_install_failed"))?;
    }

    tracing::info!("🚀 Starting SchoolOrbit Backend School Service...");

    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let backend_admin_url = required_environment("BACKEND_ADMIN_URL")?;
    let internal_secret = required_environment("INTERNAL_API_SECRET")?;
    let admin_client_config = AdminClientConfig::from_env().map_err(|error| {
        StartupError::tenancy_configuration("admin_client_configuration", &error)
    })?;
    let admin_client = Arc::new(AdminClient::new(
        backend_admin_url,
        internal_secret,
        admin_client_config,
    ));
    tracing::info!("✅ Admin client initialized (HTTP-based school mapping)");

    let pool_manager_config = PoolManagerConfig::from_env()
        .map_err(|error| StartupError::tenancy_configuration("tenant_connection_budget", &error))?;
    let pool_manager = Arc::new(PoolManager::with_config(pool_manager_config));
    let background_turns = pool_manager.background_turns();
    let websocket_manager = Arc::new(school_academic_http::websockets::WebSocketManager::new());
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

    let storage_provider =
        Arc::new(R2StorageProvider::new().await.map_err(|error| {
            StartupError::new("file_storage_configuration", error.log_safe_code())
        })?);
    let scanner_config = ClamdConfig::from_env().map_err(|_| {
        StartupError::new("malware_scanner_configuration", "invalid_CLAMD_settings")
    })?;
    let file_runtime_config = FilePlatformRuntimeConfig::from_env()
        .map_err(|error| StartupError::new("file_runtime_configuration", error.log_safe_code()))?;
    let file_platform = Arc::new(FilePlatform::with_config(
        storage_provider,
        Arc::new(ClamdScanner::new(scanner_config)),
        file_runtime_config,
    ));

    let session_config = Arc::new(
        school_auth::config::SessionConfig::from_env()
            .map_err(|_| StartupError::new("session_configuration", "session_config_invalid"))?,
    );
    let permission_cache = Arc::new(PermissionCache::new());
    let identity_cache = Arc::new(school_auth::session_cache::SessionCache::new());
    let auth_runtime = school_auth::runtime::AuthRuntime {
        admin_client: Arc::clone(&admin_client),
        pool_manager: Arc::clone(&pool_manager),
        identity_cache,
        permission_cache: Arc::clone(&permission_cache),
        config: session_config,
        session_events: session_event_tx,
        permission_events: permission_event_tx.clone(),
    };

    let state = AppState {
        admin_client,
        pool_manager,
        websocket_manager,
        notification_channel: notification_tx,

        work_event_channel: work_event_tx,

        file_platform,
        certificate_verification_limiter: Arc::new(
            school_certificates::verification_limiter::CertificateVerificationLimiter::new(),
        ),
        auth_runtime,
    };
    let app = app::build_app(state.clone());

    let addr = format!("{}:{}", host, port);
    tracing::info!("🌐 Server starting on configured listener");
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

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|error| StartupError::io("listener_bind", error))?;

    let mut sched = JobScheduler::new()
        .await
        .map_err(|error| StartupError::scheduler("scheduler_initialization", error))?;
    let admin_client_for_job = Arc::clone(&state.admin_client);
    let pool_manager_for_job = Arc::clone(&state.pool_manager);
    let file_platform_for_job = Arc::clone(&state.file_platform);

    let cleaner_running = Arc::new(tokio::sync::Mutex::new(()));
    let cleaner_job = scheduling::new_school_cron_job(
        scheduling::FILE_PLATFORM_RECONCILIATION_CRON,
        move |_uuid, _l| {
            let admin_client = Arc::clone(&admin_client_for_job);
            let pool_manager = pool_manager_for_job.clone();
            let file_platform = Arc::clone(&file_platform_for_job);
            let running = cleaner_running.clone();

            Box::pin(async move {
                let Ok(_running) = running.try_lock() else {
                    return;
                };
                services::cleaner::reconcile_all_tenants(admin_client, pool_manager, file_platform)
                    .await;
            })
        },
    )
    .map_err(|error| StartupError::scheduler("file_reconciliation_job", error))?;

    let cleanup_state = state.clone();
    let auth_retention = Arc::new(school_auth::retention::AuthRetentionRuntime::new(
        background_turns.clone(),
    ));
    let auth_cleanup_job = scheduling::new_school_cron_job(
        scheduling::FILE_PLATFORM_RECONCILIATION_CRON,
        move |_id, _scheduler| {
            let state = cleanup_state.clone();
            let retention = auth_retention.clone();
            Box::pin(async move {
                state.auth_runtime.permission_cache.cleanup_expired();
                retention
                    .sweep(&state.admin_client, &state.pool_manager)
                    .await;
            })
        },
    )
    .map_err(|error| StartupError::scheduler("auth_cleanup_job", error))?;
    sched
        .add(auth_cleanup_job)
        .await
        .map_err(|error| StartupError::scheduler("auth_cleanup_registration", error))?;

    let admin_client_for_calendar_job = Arc::clone(&state.admin_client);
    let pool_manager_for_calendar_job = Arc::clone(&state.pool_manager);
    let notification_channel_for_calendar_job = state.notification_channel.clone();
    let calendar_turns = background_turns.clone();
    let calendar_reminder_running = Arc::new(tokio::sync::Mutex::new(()));
    let calendar_reminder_job =
        scheduling::new_school_cron_job(scheduling::CALENDAR_REMINDER_CRON, move |_uuid, _l| {
            let admin_client = Arc::clone(&admin_client_for_calendar_job);
            let pool_manager = Arc::clone(&pool_manager_for_calendar_job);
            let notification_channel = notification_channel_for_calendar_job.clone();
            let turns = calendar_turns.clone();
            let running = calendar_reminder_running.clone();

            Box::pin(async move {
                // A stalled daily sweep must not accumulate another bounded
                // queue on each cron tick. This guard owns no tenant pool.
                let Ok(_running) = running.try_lock() else {
                    return;
                };
                school_calendar::delivery::process_due_calendar_reminders_for_all_tenants(
                    admin_client,
                    pool_manager,
                    notification_channel,
                    turns,
                )
                .await;
            })
        })
        .map_err(|error| StartupError::scheduler("calendar_reminder_job", error))?;

    let cleaner_job_id = cleaner_job.guid();
    let calendar_reminder_job_id = calendar_reminder_job.guid();
    sched
        .add(cleaner_job)
        .await
        .map_err(|error| StartupError::scheduler("file_reconciliation_registration", error))?;
    sched
        .add(calendar_reminder_job)
        .await
        .map_err(|error| StartupError::scheduler("calendar_reminder_registration", error))?;

    let cleaner_next_run = scheduling::next_run_for_job(&mut sched, cleaner_job_id)
        .await
        .map_err(|error| StartupError::scheduler("file_reconciliation_next_run", error))?;
    scheduling::log_next_run(
        "file_platform_reconciliation",
        scheduling::FILE_PLATFORM_RECONCILIATION_CRON,
        cleaner_next_run,
    );
    let calendar_next_run = scheduling::next_run_for_job(&mut sched, calendar_reminder_job_id)
        .await
        .map_err(|error| StartupError::scheduler("calendar_reminder_next_run", error))?;
    scheduling::log_next_run(
        "calendar_reminder",
        scheduling::CALENDAR_REMINDER_CRON,
        calendar_next_run,
    );
    sched
        .start()
        .await
        .map_err(|error| StartupError::scheduler("scheduler_start", error))?;
    tokio::spawn(school_notifications::runtime::start(
        school_notifications::runtime::NotificationRuntimeState {
            admin_client: state.admin_client.clone(),
            pool_manager: state.pool_manager.clone(),
            notification_channel: state.notification_channel.clone(),
            eligibility: &adapters::notification_delivery::APPLICATION_NOTIFICATION_ELIGIBILITY,
        },
    ));
    tokio::spawn(school_attendance::runtime::start(
        school_attendance::runtime::AttendanceRuntimeState {
            admin_client: state.admin_client.clone(),
            pool_manager: state.pool_manager.clone(),
            notification_channel: state.notification_channel.clone(),
            behavior: &adapters::behavior::APPLICATION_BEHAVIOR,
        },
    ));

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(|error| StartupError::io("server", error))?;
    Ok(())
}
