use sqlx::{PgPool, postgres::PgPoolOptions};

/// Auto-configure encryption key for all tenant databases at startup
/// This runs ALTER ROLE SET to make encryption key persistent
pub async fn setup_encryption_for_all_tenants(admin_pool: &PgPool) -> Result<(), String> {
    tracing::info!("🔐 Setting up encryption key for all tenant databases...");
    
    // Get encryption key
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| "ENCRYPTION_KEY not set".to_string())?;
    
    // Get database user from environment or use default
    let db_user = std::env::var("DB_USER")
        .unwrap_or_else(|_| "postgres".to_string());
    
    // Get all active tenant database URLs
    let tenant_urls: Vec<String> = sqlx::query_scalar(
        "SELECT database_url FROM schools WHERE status = 'active'"
    )
    .fetch_all(admin_pool)
    .await
    .map_err(|e| format!("Failed to fetch tenant databases: {}", e))?;
    
    if tenant_urls.is_empty() {
        tracing::warn!("⚠️  No active tenant databases found");
        return Ok(());
    }
    
    tracing::info!("📊 Found {} active tenant database(s)", tenant_urls.len());
    
    let mut success_count = 0;
    let mut fail_count = 0;
    
    // Process each tenant database
    for tenant_url in tenant_urls {
        // Extract database name for logging
        let db_name = tenant_url.split('/').last().unwrap_or("unknown");
        
        // Connect to tenant database
        match PgPoolOptions::new()
            .max_connections(1)
            .connect(&tenant_url)
            .await
        {
            Ok(pool) => {
                // Run ALTER ROLE to set encryption key persistently
                let alter_query = format!(
                    "ALTER ROLE {} SET app.encryption_key = '{}'",
                    db_user, encryption_key
                );
                
                match sqlx::query(&alter_query).execute(&pool).await {
                    Ok(_) => {
                        tracing::info!("  ✅ {}: Encryption key configured", db_name);
                        success_count += 1;
                    }
                    Err(e) => {
                        tracing::error!("  ❌ {}: Failed to set encryption key: {}", db_name, e);
                        fail_count += 1;
                    }
                }
                
                pool.close().await;
            }
            Err(e) => {
                tracing::error!("  ❌ {}: Failed to connect: {}", db_name, e);
                fail_count += 1;
            }
        }
    }
    
    tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    tracing::info!("✅ Success: {}", success_count);
    if fail_count > 0 {
        tracing::warn!("❌ Failed: {}", fail_count);
    }
    
    if fail_count > 0 {
        tracing::warn!("⚠️  Some databases failed encryption setup. Check logs above.");
    } else {
        tracing::info!("🎉 All tenant databases configured successfully!");
        tracing::info!("🔑 Encryption key will be set automatically for all connections");
    }
    
    Ok(())
}
