use crate::db::school_mapping::get_school_database_url;
use crate::modules::menu::models::*;
use crate::modules::auth::models::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::AppState;
use crate::error::AppError;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};

use std::collections::HashMap;
use uuid::Uuid;

/// Get user's menu items based on permissions
pub async fn get_user_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    // Get authenticated user
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    let token_from_header = auth_header
        .and_then(|h| {
            if h.starts_with("Bearer ") {
                Some(h[7..].to_string())
            } else {
                None
            }
        });

    // Fallback to cookie
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    
    let claims = crate::utils::jwt::JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    
    let mut user: User = sqlx::query_as(
        "SELECT 
            id,
            username,
            national_id,
            email,
            password_hash,
            first_name,
            last_name,
            user_type,
            phone,
            date_of_birth,
            address,
            status,
            metadata,
            created_at,
            updated_at,
            title,
            nickname,
            emergency_contact,
            line_id,
            gender,
            profile_image_url,
            hired_date,
            resigned_date
         FROM users 
         WHERE id = $1"
    )
        .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get user: {}", e);
            AppError::InternalServerError(format!("Database error: {}", e))
        })?;

    // Decrypt national_id
    // Decrypt national_id
    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Get user permissions
    let user_permissions = get_user_permissions(&user.id, &pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get user permissions: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูล permissions ได้".to_string())
        })?;

    // Query menu items with groups - filter by user_type
    let user_type = user.user_type.as_str();
    
    // [NEW] If staff, fetch allowed menu IDs from department assignments
    let allowed_dept_menu_ids: Option<Vec<Uuid>> = if user_type == "staff" {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT dma.menu_item_id
            FROM department_members dm
            JOIN department_menu_access dma ON dm.department_id = dma.department_id
            WHERE dm.user_id = $1
              AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
            "#
        )
        .bind(&user.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
             eprintln!("❌ Failed to fetch department menu access: {}", e);
             AppError::InternalServerError("ไม่สามารถตรวจสอบสิทธิ์เมนูฝ่ายได้".to_string())
        })?;
        
        Some(rows.into_iter().map(|r| r.get("menu_item_id")).collect())
    } else {
        None 
    };

    let menu_rows: Vec<(Uuid, String, String, String, Option<String>, Option<String>, String, String, Option<String>, i32, i32)> = 
        sqlx::query_as(
            r#"
            SELECT 
                mi.id,
                mi.code,
                mi.name,
                mi.path,
                mi.icon,
                mi.required_permission,
                mg.code as group_code,
                mg.name as group_name,
                mg.icon as group_icon,
                mg.display_order as group_order,
                mi.display_order
            FROM menu_items mi
            JOIN menu_groups mg ON mi.group_id = mg.id
            WHERE mi.is_active = true 
              AND mg.is_active = true
              AND mi.user_type = $1
            ORDER BY mg.display_order, mi.display_order
            "#
        )
        .bind(user_type)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to fetch menu items: {}", e);
             AppError::InternalServerError("ไม่สามารถดึงข้อมูลเมนูได้".to_string())
        })?;

    // Group and filter menu items
    let groups = group_and_filter_menu(menu_rows, &user_permissions, allowed_dept_menu_ids.as_deref());

    Ok((
        StatusCode::OK,
        Json(UserMenuResponse {
            success: true,
            groups,
        }),
    ))
}

/// Check if user has ANY permission in the specified module
/// Example: user_has_module_permission(&["staff.update.all"], "staff") -> true
/// Also checks for wildcard permission (*)
fn user_has_module_permission(user_permissions: &[String], module: &str) -> bool {
    // Check for wildcard permission first
    if user_permissions.contains(&"*".to_string()) {
        return true;
    }
    
    // Check for exact permission match
    if user_permissions.contains(&module.to_string()) {
        return true;
    }
    
    // Check for module-specific permissions
    let prefix = format!("{}.", module);
    user_permissions.iter().any(|perm| perm.starts_with(&prefix))
}

/// Group menu items by group and filter by permissions
fn group_and_filter_menu(
    rows: Vec<(Uuid, String, String, String, Option<String>, Option<String>, String, String, Option<String>, i32, i32)>,
    user_permissions: &[String],
    allowed_dept_menu_ids: Option<&[Uuid]>,
) -> Vec<MenuGroupResponse> {
    // Intermediate struct to hold order information
    struct GroupWithOrder {
        order: i32,
        code: String,
        name: String,
        icon: Option<String>,
        items: Vec<(i32, MenuItemResponse)>, // (item_order, item)
    }
    
    let mut groups_map: HashMap<String, GroupWithOrder> = HashMap::new();

    for (id, code, name, path, icon, required_permission, group_code, group_name, group_icon, group_order, item_order) in rows {
        // 1. Check Role Permission (Standard RBAC)
        if let Some(module) = &required_permission {
            if !user_has_module_permission(user_permissions, module) {
                continue; // Skip if user doesn't have any permission in this module
            }
        }

        // 2. Check Department Menu Access (Menu RBAC)
        // If list is provided (for Staff), user MUST have access via department
        // UNLESS user has super-admin wildcard permission (*)
        if let Some(allowed_ids) = allowed_dept_menu_ids {
            let is_super_admin = user_permissions.contains(&"*".to_string());
            if !is_super_admin && !allowed_ids.contains(&id) {
                continue;
            }
        }

        // Get or create group
        let group = groups_map
            .entry(group_code.clone())
            .or_insert_with(|| GroupWithOrder {
                order: group_order,
                code: group_code.clone(),
                name: group_name.clone(),
                icon: group_icon.clone(),
                items: vec![],
            });

        // Add item to group with its order
        group.items.push((item_order, MenuItemResponse {
            id,
            code,
            name,
            path,
            icon,
        }));
    }

    // Convert to vector and sort
    let mut groups: Vec<GroupWithOrder> = groups_map
        .into_values()
        .filter(|g| !g.items.is_empty())
        .collect();

    // Sort groups by display_order
    groups.sort_by_key(|g| g.order);

    // Sort items within each group and convert to final format
    groups
        .into_iter()
        .map(|mut g| {
            // Sort items by display_order
            g.items.sort_by_key(|(order, _)| *order);
            
            MenuGroupResponse {
                code: g.code,
                name: g.name,
                icon: g.icon,
                items: g.items.into_iter().map(|(_, item)| item).collect(),
            }
        })
        .collect()
}

/// Get user's permissions from roles (normalized schema)
async fn get_user_permissions(
    user_id: &Uuid,
    pool: &sqlx::PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    let permissions: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT p.code
        FROM user_roles ur
        JOIN role_permissions rp ON ur.role_id = rp.role_id
        JOIN permissions p ON rp.permission_id = p.id
        WHERE ur.user_id = $1
          AND ur.ended_at IS NULL
        ORDER BY p.code
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(permissions)
}
