use crate::models::menu::{RouteRegistration, RouteItem, RouteRegistrationResponse};
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::{Response, IntoResponse},
    Json as JsonResponse,
};

/// Register routes from frontend build
/// This endpoint is called during frontend build to auto-sync menu items
pub async fn register_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<RouteRegistration>,
) -> Response {
    // Validate deploy key
    let deploy_key = headers
        .get("X-Deploy-Key")
        .and_then(|h| h.to_str().ok());
    
    let expected_key = match std::env::var("DEPLOY_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("❌ DEPLOY_KEY environment variable not set!");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Server configuration error"
                }))
            ).into_response();
        }
    };
    
    if deploy_key != Some(&expected_key.as_str()) {
        eprintln!("❌ Invalid deploy key provided");
        return (
            StatusCode::UNAUTHORIZED,
            JsonResponse(serde_json::json!({
                "success": false,
                "error": "Invalid deploy key"
            }))
        ).into_response();
    }
    
    println!("✅ Deploy key validated");
    println!("📋 Registering {} routes from {} environment", 
        data.routes.len(), 
        data.environment.as_deref().unwrap_or("unknown")
    );
    
    // Get tenant pool
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("⚠️  No subdomain provided, skipping registration");
            return (
                StatusCode::BAD_REQUEST,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "No subdomain specified"
                }))
            ).into_response();
        }
    };
    
    let db_url = match crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Database connection failed"
                }))
            ).into_response();
        }
    };
    
    let pool = match sqlx::PgPool::connect(&db_url).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to connect to database: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Database connection failed"
                }))
            ).into_response();
        }
    };
    
    
    // Smart sync: UPSERT to preserve user customizations (display_order, is_active)
    // while updating code-controlled fields (name, path, icon, permission, group)
    println!("🔄 Syncing menu items (preserving user customizations)...");
    
    let mut registered_count = 0;
    let mut updated_count = 0;
    let total_routes = data.routes.len();
    
    // Collect all active codes for cleanup phase
    let mut active_codes: Vec<String> = Vec::new();
    
    for route in &data.routes {
        // Generate code from title (slugify)
        let code = slugify(&route.title);
        active_codes.push(code.clone());
        
        // UPSERT: Insert new or update existing, preserving display_order and is_active
        let result = sqlx::query(
            r#"
            INSERT INTO menu_items (
                id, code, name, name_en, path, icon, 
                required_permission, group_id, display_order, is_active
            )
            VALUES (
                gen_random_uuid(),
                $1, $2, NULL, $3, $4, $5,
                (SELECT id FROM menu_groups WHERE code = $6),
                $7,
                true
            )
            ON CONFLICT (code) DO UPDATE SET
                name = EXCLUDED.name,
                path = EXCLUDED.path,
                icon = EXCLUDED.icon,
                required_permission = EXCLUDED.required_permission,
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
    
    (
        StatusCode::OK,
        JsonResponse(RouteRegistrationResponse {
            success: true,
            registered: registered_count,
            message: format!("Synced {} routes (preserved user customizations)", registered_count),
        })
    ).into_response()
}

/// Helper: Convert title to slug (code)
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' {
                '_'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("_")
}
