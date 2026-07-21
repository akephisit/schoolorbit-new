use super::models::{
    ChangePasswordRequest, Claims, LoginData, LoginRequest, ProfileResponse, UpdateProfileRequest,
    UserResponse,
};
use super::services;
use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::middleware::permission::get_cached_user_permissions;
use crate::utils::file_url::get_file_url_from_string;
use crate::utils::jwt::JwtService;
use crate::utils::request_context::{
    current_user_tenant_context_from_claims, current_user_tenant_context_from_headers,
    tenant_context,
};
use crate::AppState;
use axum::{
    extract::{rejection::JsonRejection, Request, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use tower_cookies::{Cookie, Cookies};

/// Login handler
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    payload_result: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let Json(payload) =
        payload_result.map_err(|rejection| AppError::ValidationError(rejection.body_text()))?;

    let tenant = tenant_context(&state, &headers).await?;
    let subdomain = tenant.subdomain;
    let pool = tenant.pool;

    tracing::info!(school = %subdomain, username = %payload.username, "Login attempt");

    let user = services::find_active_login_user_by_username(&pool, &payload.username).await?;

    // Verify password
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return Err(AppError::AuthError("รหัสผ่านไม่ถูกต้อง".to_string()));
    }

    // Generate JWT token
    let token = JwtService::generate_token(
        &user.id.to_string(),
        &user.username,
        &user.user_type,
        &subdomain,
    )
    .map_err(|e| {
        tracing::error!("Failed to generate token: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้าง token ได้".to_string())
    })?;

    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    let permissions =
        get_cached_user_permissions(&subdomain, user.id, &pool, &state.permission_cache)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch login permissions: {}", e);
                AppError::InternalServerError("ไม่สามารถดึงสิทธิ์ผู้ใช้ได้".to_string())
            })?;

    // Create user response manually (LoginUser doesn't implement From)
    let user_response = UserResponse {
        id: user.id,
        username: user.username.clone(),
        national_id: None, // Don't send national_id on login via username for privacy
        email: user.email.clone(),
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        user_type: user.user_type.clone(),
        phone: None,
        status: user.status.clone(),
        created_at: chrono::Utc::now(),
        primary_role_name,
        profile_image_url: get_file_url_from_string(&user.profile_image_url),
        permissions: Some(permissions),
    };

    // Set cookie (optional, based on remember_me)
    let max_age = if payload.remember_me.unwrap_or(false) {
        30 * 24 * 60 * 60 // 30 days in seconds
    } else {
        24 * 60 * 60 // 1 day in seconds
    };

    let mut cookie = Cookie::new("auth_token", token.clone());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_max_age(time::Duration::seconds(max_age));
    cookie.set_secure(true);

    cookies.add(cookie);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            LoginData {
                user: user_response,
            },
            "เข้าสู่ระบบสำเร็จ",
        )),
    ))
}

/// Logout handler
pub async fn logout(cookies: Cookies) -> Result<impl IntoResponse, AppError> {
    // Remove auth token cookie
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0)); // Expire immediately

    cookies.add(cookie);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ออกจากระบบสำเร็จ")),
    ))
}

/// Get current user handler (protected)
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract claims from middleware
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    let subdomain = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;

    let user = services::find_user_by_id(&pool, context.user_id).await?;
    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    let permissions =
        get_cached_user_permissions(&subdomain, user.id, &pool, &state.permission_cache)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch current user permissions: {}", e);
                AppError::InternalServerError("ไม่สามารถดึงสิทธิ์ผู้ใช้ได้".to_string())
            })?;

    // Create response with primary role name and permissions
    let mut user_response = UserResponse::from(user);
    user_response.primary_role_name = primary_role_name;
    user_response.permissions = Some(permissions);

    Ok((StatusCode::OK, Json(ApiResponse::ok(user_response))))
}

/// Get full profile handler (GET /me/profile)
/// Returns complete user profile with all fields
/// Get full profile handler (GET /me/profile)
/// Returns complete user profile with all fields
pub async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract claims from middleware
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    let context = current_user_tenant_context_from_claims(&state, &headers, &claims).await?;
    let pool = context.tenant.pool;

    let user = services::find_user_by_id(&pool, context.user_id).await?;
    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    // Create full profile response with primary role name
    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(ApiResponse::ok(profile_response))))
}

/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;
    let user_id = context.user_id;

    let user = services::update_profile(&pool, user_id, payload).await?;
    let primary_role_name = services::get_primary_role_name(&pool, user.id).await?;

    // Return updated profile
    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(ApiResponse::ok(profile_response))))
}

/// Change password handler (POST /me/change-password)
/// Changes user's password after verifying current password
/// Change password handler (POST /me/change-password)
/// Changes user's password after verifying current password
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;
    let user_id = context.user_id;

    let user = services::find_active_login_user_by_id(&pool, user_id).await?;

    // Verify current (old) password
    let is_valid = bcrypt::verify(&payload.current_password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return Err(AppError::AuthError("รหัสผ่านปัจจุบันไม่ถูกต้อง".to_string()));
    }

    // Hash new password
    let new_password_hash = bcrypt::hash(&payload.new_password, 10).map_err(|e| {
        tracing::error!("Failed to hash password: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    services::update_password_hash(&pool, user_id, new_password_hash).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("เปลี่ยนรหัสผ่านสำเร็จ")),
    ))
}
