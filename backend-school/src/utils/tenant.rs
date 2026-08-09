use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_info;
use crate::error::AppError;
use crate::modules::auth::runtime::AuthRuntime;
use crate::utils::subdomain::{extract_subdomain_from_request, TenantOriginPolicy};
use crate::AppState;

#[derive(Clone)]
#[allow(dead_code)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub subdomain: String,
    pub pool: PgPool,
}

pub async fn resolve_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    resolve_tenant_context_by_subdomain(state, &subdomain).await
}

pub async fn resolve_tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    let school = get_school_database_info(&state.admin_client, subdomain)
        .await
        .map_err(|error| {
            tracing::warn!(
                subdomain = %subdomain,
                error = %error,
                "Failed to resolve school database URL"
            );
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state
        .pool_manager
        .get_pool(&school.database_url, subdomain)
        .await
        .map_err(|error| {
            tracing::error!(
                subdomain = %subdomain,
                error = %error,
                "Failed to resolve tenant database pool"
            );
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    Ok(TenantContext {
        tenant_id: school.tenant_id,
        subdomain: subdomain.to_string(),
        pool,
    })
}

pub async fn tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context(state, headers).await
}

pub async fn tenant_pool(state: &AppState, headers: &HeaderMap) -> Result<PgPool, AppError> {
    Ok(resolve_tenant_context(state, headers).await?.pool)
}

pub async fn tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context_by_subdomain(state, subdomain).await
}

pub async fn resolve_auth_tenant_context(
    runtime: &AuthRuntime,
    headers: &HeaderMap,
    dev_realtime_tenant_hint: Option<&str>,
) -> Result<TenantContext, AppError> {
    let policy = TenantOriginPolicy::new(
        &runtime.config.base_domain,
        runtime
            .config
            .allowed_dev_origins
            .iter()
            .map(String::as_str),
    );
    let subdomain = policy.resolve_tenant(headers, dev_realtime_tenant_hint)?;

    let school = get_school_database_info(&runtime.admin_client, &subdomain)
        .await
        .map_err(|error| {
            if error.contains("not found or inactive") {
                AppError::NotFound("ไม่พบโรงเรียน".to_string())
            } else {
                AppError::ServiceUnavailable("tenant_directory".to_string())
            }
        })?;
    let pool = runtime
        .pool_manager
        .get_pool(&school.database_url, &subdomain)
        .await
        .map_err(|_| AppError::ServiceUnavailable("tenant_pool".to_string()))?;

    Ok(TenantContext {
        tenant_id: school.tenant_id,
        subdomain,
        pool,
    })
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use axum::{
        extract::Path,
        http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
        Router,
    };
    use tokio::{net::TcpListener, sync::broadcast};

    use crate::{
        db::{
            admin_client::{AdminClient, AdminClientConfig},
            permission_cache::PermissionCache,
            pool_manager::PoolManager,
        },
        modules::auth::{
            config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
        },
    };

    use super::resolve_auth_tenant_context;

    async fn directory_response(Path(subdomain): Path<String>) -> Response {
        match subdomain.as_str() {
            "missing" => StatusCode::NOT_FOUND.into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "sensitive upstream diagnostic",
            )
                .into_response(),
        }
    }

    async fn runtime_with_directory() -> (AuthRuntime, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test directory listener must bind");
        let address = listener
            .local_addr()
            .expect("test directory address must resolve");
        let app = Router::new().route("/internal/schools/{subdomain}", get(directory_response));
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test directory server must run");
        });
        let (events, _) = broadcast::channel(4);
        let runtime = AuthRuntime {
            admin_client: Arc::new(AdminClient::new(
                format!("http://{address}"),
                "test-secret".to_string(),
                AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
            )),
            pool_manager: Arc::new(PoolManager::new()),
            permission_cache: Arc::new(PermissionCache::new()),
            config: Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
                [17; 32],
            ))),
            session_events: events,
        };
        (runtime, server)
    }

    fn origin(subdomain: &str) -> HeaderMap {
        HeaderMap::from_iter([(
            HeaderName::from_static("origin"),
            HeaderValue::from_str(&format!("https://{subdomain}.schoolorbit.test"))
                .expect("test origin must be valid"),
        )])
    }

    #[tokio::test]
    async fn auth_tenant_resolver_distinguishes_unknown_school_from_directory_failure() {
        let (runtime, server) = runtime_with_directory().await;

        let missing = match resolve_auth_tenant_context(&runtime, &origin("missing"), None).await {
            Err(error) => error,
            Ok(_) => panic!("unknown school must fail"),
        };
        assert_eq!(missing.status_code(), StatusCode::NOT_FOUND);

        let unavailable =
            match resolve_auth_tenant_context(&runtime, &origin("unavailable"), None).await {
                Err(error) => error,
                Ok(_) => panic!("directory failure must fail closed"),
            };
        assert_eq!(unavailable.status_code(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            unavailable.public_message(),
            "Service temporarily unavailable"
        );
        assert!(!unavailable
            .public_message()
            .contains("sensitive upstream diagnostic"));

        server.abort();
    }
}
