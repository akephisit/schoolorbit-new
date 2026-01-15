use crate::modules::menu::models::{RouteRegistration, RouteRegistrationResponse};
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;

use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
    Json as JsonResponse,
};

/// Register routes from frontend build
/// This endpoint is called during frontend build to auto-sync menu items
pub async fn register_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<RouteRegistration>,
) -> Result<impl IntoResponse, AppError> {
    // Validate deploy key
    let deploy_key = headers
        .get("X-Deploy-Key")
        .and_then(|h| h.to_str().ok());
    
    let expected_key = std::env::var("DEPLOY_KEY")
        .map_err(|_| {
            eprintln!("❌ DEPLOY_KEY environment variable not set!");
            AppError::InternalServerError("Server configuration error".to_string())
        })?;
    
    if deploy_key != Some(&expected_key.as_str()) {
        eprintln!("❌ Invalid deploy key provided");
        return Err(AppError::AuthError("Invalid deploy key".to_string()));
    }
    
    println!("✅ Deploy key validated");
    println!("📋 Registering {} routes from {} environment", 
        data.routes.len(), 
        data.environment.as_deref().unwrap_or("unknown")
    );
    
    // Get tenant pool
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| {
            eprintln!("⚠️  No subdomain provided, skipping registration");
            AppError::BadRequest("No subdomain specified".to_string())
        })?;
    
    let db_url = crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
             AppError::InternalServerError("Database connection failed".to_string())
        })?;
    
    let pool = sqlx::PgPool::connect(&db_url).await
        .map_err(|e| {
            eprintln!("❌ Failed to connect to database: {}", e);
            AppError::InternalServerError("Database connection failed".to_string())
        })?;
    
    
    // Smart sync: UPSERT to preserve user customizations (display_order, is_active)
    // while updating code-controlled fields (name, path, icon, permission, group)
    println!("🔄 Syncing menu items (preserving user customizations)...");
    
    let mut registered_count = 0;
    let _updated_count = 0;
    let total_routes = data.routes.len();
    
    // Collect all active codes for cleanup phase
    let mut active_codes: Vec<String> = Vec::new();
    
    for route in &data.routes {
        // Generate code from path (unique) instead of title (can duplicate)
        // Example: /staff → "staff", /staff/students -> "staff-students"
        let code = route.path
            .trim_start_matches('/')
            .replace('/', "-");
        active_codes.push(code.clone());
        
        
        // Use provided user_type or default to 'staff'
        // Frontend SHOULD explicitly set user_type in _meta
        let user_type = route.user_type.as_deref().unwrap_or("staff");
        
        // Debug logging
        println!("📝 Syncing: {} -> {} (user_type: {:?} -> {})", 
            route.title, route.path, route.user_type, user_type);

        // UPSERT: Insert new or update existing, preserving display_order and is_active
        let result = sqlx::query(
            r#"
            INSERT INTO menu_items (
                id, code, name, name_en, path, icon, 
                required_permission, user_type, group_id, display_order, is_active
            )
            VALUES (
                gen_random_uuid(),
                $1, $2, NULL, $3, $4, $5, $6,
                (SELECT id FROM menu_groups WHERE code = $7),
                $8,
                true
            )
            ON CONFLICT (code) DO UPDATE SET
                name = EXCLUDED.name,
                path = EXCLUDED.path,
                icon = EXCLUDED.icon,
                required_permission = EXCLUDED.required_permission,
                user_type = EXCLUDED.user_type,
                -- Preserve existing group_id (user may have moved items manually):
                group_id = COALESCE(menu_items.group_id, EXCLUDED.group_id),
                -- Preserve user customizations:
                display_order = COALESCE(menu_items.display_order, EXCLUDED.display_order),
                is_active = menu_items.is_active
            "#,
        )
        .bind(&code)
        .bind(&route.title)
        .bind(&route.path)
        .bind(&route.icon)
        .bind(&route.permission)
        .bind(user_type)
        .bind(&route.group)
        .bind(route.order)
        .execute(&pool)
        .await;
        
        match result {
            Ok(res) => {
                if res.rows_affected() > 0 {
                    println!("✅ Synced: {} -> {}", route.title, route.path);
                    registered_count += 1;
                    // Note: rows_affected() for ON CONFLICT UPDATE is always 1, can't distinguish insert vs update reliably
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to sync {}: {}", route.title, e);
            }
        }
    }
    
    // Cleanup: Remove menu items that no longer exist in code (orphaned items)
    if !active_codes.is_empty() {
        println!("🧹 Cleaning up orphaned menu items...");
        
        // Build placeholders for IN clause
        let placeholders: Vec<String> = (1..=active_codes.len())
            .map(|i| format!("${}", i))
            .collect();
        let in_clause = placeholders.join(", ");
        
        let delete_query = format!(
            "DELETE FROM menu_items WHERE code NOT IN ({})",
            in_clause
        );
        
        let mut query = sqlx::query(&delete_query);
        for code in &active_codes {
            query = query.bind(code);
        }
        
        match query.execute(&pool).await {
            Ok(result) => {
                let deleted = result.rows_affected();
                if deleted > 0 {
                    println!("🗑️  Removed {} orphaned menu items", deleted);
                }
            }
            Err(e) => {
                eprintln!("⚠️  Failed to clean orphaned items: {}", e);
            }
        }
    }
    
    println!("🎉 Successfully synced {}/{} routes", registered_count, total_routes);
    
    Ok((
        StatusCode::OK,
        JsonResponse(RouteRegistrationResponse {
            success: true,
            registered: registered_count,
            message: format!("Synced {} routes (preserved user customizations)", registered_count),
        })
    ))
}


