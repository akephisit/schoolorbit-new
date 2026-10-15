use super::{auth, models::*, service, PortalError};
use crate::{types::ApiResponse, AppState};
use axum::{
    extract::{Extension, Path, Request, State},
    http::{HeaderMap, Method},
    middleware::Next,
    response::Response,
    Json,
};
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

fn token(cookies: &Cookies) -> Result<String, PortalError> {
    cookies
        .get(auth::COOKIE)
        .map(|c| c.value().to_string())
        .ok_or_else(PortalError::unauthorized)
}
fn session_cookie(value: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(auth::COOKIE, value);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(true);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_max_age(tower_cookies::cookie::time::Duration::days(1));
    cookie
}
fn message() -> Json<ApiResponse<()>> {
    Json(ApiResponse {
        success: true,
        data: None,
        message: Some("ดำเนินการแล้ว หากข้อมูลตรงกับบัญชี ระบบจะส่งอีเมลให้ กรุณาตรวจสอบกล่องจดหมาย".into()),
    })
}
async fn public_limit(
    pool: &sqlx::PgPool,
    headers: &HeaderMap,
    action: &str,
    identifier: &str,
) -> Result<(), PortalError> {
    auth::require_origin(headers)?;
    let source = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<std::net::IpAddr>().ok())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".into());
    auth::throttle(pool, &format!("source:{action}:{source}"), 100).await?;
    auth::throttle(pool, &format!("identifier:{action}:{identifier}"), 10).await
}
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Registration>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    let email = auth::normalize_email(&input.email)?;
    public_limit(&state.pool, &headers, "register", &email).await?;
    auth::register(&state.pool, input).await?;
    Ok(message())
}
pub async fn login(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
    Json(input): Json<Credentials>,
) -> Result<Json<ApiResponse<SessionResponse>>, PortalError> {
    let email = auth::normalize_email(&input.email)?;
    public_limit(&state.pool, &headers, "login", &email).await?;
    let (user, token) = auth::login(&state.pool, input).await?;
    cookies.add(session_cookie(token.clone()));
    let mut legacy = Cookie::new("auth_token", "");
    legacy.set_path("/");
    cookies.remove(legacy);
    Ok(Json(ApiResponse::success(SessionResponse {
        user,
        csrf_token: auth::csrf(&token),
    })))
}
pub async fn me(
    State(state): State<AppState>,
    cookies: Cookies,
) -> Result<Json<ApiResponse<SessionResponse>>, PortalError> {
    let token = token(&cookies)?;
    let user = auth::authenticate(&state.pool, &token).await?;
    Ok(Json(ApiResponse::success(SessionResponse {
        user,
        csrf_token: auth::csrf(&token),
    })))
}
pub async fn logout(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    let token = token(&cookies)?;
    auth::require_csrf(&headers, &token)?;
    auth::logout(&state.pool, &token).await?;
    cookies.remove(session_cookie(String::new()));
    Ok(message())
}
pub async fn resend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EmailInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    let email = auth::normalize_email(&input.email)?;
    public_limit(&state.pool, &headers, "verify-email", &email).await?;
    auth::send_link(&state.pool, &email, "verify").await?;
    Ok(message())
}
pub async fn forgot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EmailInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    let email = auth::normalize_email(&input.email)?;
    public_limit(&state.pool, &headers, "forgot", &email).await?;
    auth::send_link(&state.pool, &email, "reset").await?;
    Ok(message())
}
pub async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TokenInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    public_limit(&state.pool, &headers, "verify-token", &input.token).await?;
    auth::consume_token(&state.pool, &input.token, None).await?;
    Ok(message())
}
pub async fn reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ResetInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    public_limit(&state.pool, &headers, "reset-token", &input.token).await?;
    auth::consume_token(&state.pool, &input.token, Some(input.password)).await?;
    Ok(message())
}
pub async fn require_session(
    State(state): State<AppState>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Result<Response, PortalError> {
    let token = token(&cookies)?;
    let account = auth::authenticate(&state.pool, &token).await?;
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        auth::require_csrf(request.headers(), &token)?;
    }
    request.extensions_mut().insert(account);
    Ok(next.run(request).await)
}
pub async fn require_operator(request: Request, next: Next) -> Result<Response, PortalError> {
    if !request
        .extensions()
        .get::<Account>()
        .is_some_and(Account::is_operator)
    {
        return Err(PortalError::forbidden());
    }
    Ok(next.run(request).await)
}
pub async fn schools(
    State(state): State<AppState>,
    Extension(account): Extension<Account>,
) -> Result<Json<ApiResponse<Vec<CustomerSchool>>>, PortalError> {
    Ok(Json(ApiResponse::success(
        service::schools(&state.pool, &account).await?,
    )))
}
pub async fn requests(
    State(state): State<AppState>,
    Extension(account): Extension<Account>,
) -> Result<Json<ApiResponse<Vec<ServiceRequest>>>, PortalError> {
    Ok(Json(ApiResponse::success(
        service::requests(&state.pool, &account).await?,
    )))
}
pub async fn create_request(
    State(state): State<AppState>,
    Extension(account): Extension<Account>,
    Json(input): Json<ServiceRequestInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    service::create_request(&state.pool, &account, input).await?;
    Ok(message())
}
pub async fn decide(
    State(state): State<AppState>,
    Extension(account): Extension<Account>,
    Path(id): Path<Uuid>,
    Json(input): Json<DecisionInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    service::decide(&state.pool, &account, id, input).await?;
    Ok(message())
}
pub async fn assign(
    State(state): State<AppState>,
    Extension(account): Extension<Account>,
    Path(id): Path<Uuid>,
    Json(input): Json<AssignmentInput>,
) -> Result<Json<ApiResponse<()>>, PortalError> {
    service::assign(&state.pool, &account, id, &input.email).await?;
    Ok(message())
}
