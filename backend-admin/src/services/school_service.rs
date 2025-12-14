use crate::models::{School, CreateSchool, UpdateSchool};
use crate::error::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SchoolService {
    pool: PgPool,
}

impl SchoolService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_school(&self, data: CreateSchool) -> Result<School, AppError> {
        // Validate Thai national ID (13 digits)
        if !data.admin_national_id.chars().all(|c| c.is_ascii_digit()) || data.admin_national_id.len() != 13 {
            return Err(AppError::ValidationError(
                "Admin national ID must be exactly 13 digits".to_string()
            ));
        }

        // Validate subdomain format (lowercase, alphanumeric, hyphens)
        if !data.subdomain.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(AppError::ValidationError(
                "Subdomain must contain only lowercase letters, numbers, and hyphens".to_string()
            ));
        }

        // Check if subdomain already exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM schools WHERE subdomain = $1)"
        )
        .bind(&data.subdomain)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if exists {
            return Err(AppError::ValidationError(
                "Subdomain already exists".to_string()
            ));
        }

        // Generate database name
        let db_name = format!("schoolorbit_{}", data.subdomain);

        println!("🚀 Starting school provisioning for: {}", data.name);

        // Step 1: Create database in Neon
        println!("📦 Step 1/4: Creating database in Neon...");
        use crate::clients::neon_client::NeonClient;
        let neon_client = NeonClient::new()
            .map_err(|e| AppError::ExternalServiceError(format!("Neon client error: {}", e)))?;

        let db_id = neon_client
            .create_database(&db_name, "neondb_owner")
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Failed to create database: {}", e)))?;

        println!("✅ Database created with ID: {}", db_id);

        // Wait for database to be ready
        neon_client
            .wait_for_database_ready(&db_name)
            .await
            .map_err(|e| {
                // Rollback: Delete database if wait fails
                let _ = async {
                    neon_client.delete_database(db_id).await
                };
                AppError::ExternalServiceError(format!("Database not ready: {}", e))
            })?;

        // Get Neon database password from environment
        // This is the password for neondb_owner role in your Neon project
        let db_password = std::env::var("NEON_DB_PASSWORD")
            .map_err(|_| AppError::ExternalServiceError(
                "NEON_DB_PASSWORD not set. Get this from Neon console.".to_string()
            ))?;
        
        let db_connection_string = neon_client.get_connection_string(
            &db_name,
            "neondb_owner",
            &db_password,
        );

        // Create school record
        let school = sqlx::query_as::<_, School>(
            r#"
            INSERT INTO schools (name, subdomain, db_name, db_connection_string, status, config)
            VALUES ($1, $2, $3, $4, 'provisioning', '{}')
            RETURNING *
            "#
        )
        .bind(&data.name)
        .bind(&data.subdomain)
        .bind(&db_name)
        .bind(&db_connection_string)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let school_id = school.id;

        // Step 2: Provision tenant via backend-school
        println!("📦 Step 2/4: Provisioning tenant database via backend-school...");
        use crate::clients::backend_school_client::BackendSchoolClient;
        let backend_school_client = BackendSchoolClient::new()
            .map_err(|e| AppError::ExternalServiceError(format!("Backend-school client error: {}", e)))?;

        match backend_school_client
            .provision_tenant(&school_id.to_string(), &db_connection_string, &data.subdomain)
            .await
        {
            Ok(_) => {
                println!("✅ Tenant database provisioned successfully");
            }
            Err(e) => {
                eprintln!("❌ Failed to provision tenant: {}", e);
                // Rollback: Delete database
                let _ = neon_client.delete_database(db_id).await;
                return Err(AppError::ExternalServiceError(format!("Tenant provisioning failed: {}", e)));
            }
        }

        // Step 3: Create DNS record in Cloudflare
        // NOTE: Skipped! Wrangler (GitHub Actions) will create and manage DNS automatically
        // when deploying with custom_domain configuration
        println!("📦 Step 3/4: Skipping DNS creation (Wrangler will handle this)...");
        let dns_record_id = "".to_string(); // Managed by Wrangler
        
        /* Commented out - DNS is now managed by Wrangler
        println!("📦 Step 3/4: Creating DNS record...");
        use crate::clients::cloudflare_client::CloudflareClient;
        let cloudflare_client = CloudflareClient::new()
            .map_err(|e| AppError::ExternalServiceError(format!("Cloudflare client error: {}", e)))?;

        let dns_record_id = match cloudflare_client.create_dns_record(&data.subdomain).await {
            Ok(id) => {
                println!("✅ DNS record created with ID: {}", id);
                id
            }
            Err(e) => {
                eprintln!("❌ Failed to create DNS record: {}", e);
                // Continue anyway - DNS can be fixed manually
                "".to_string()
            }
        };
        */


        // Step 4: Deploy Cloudflare Worker
        println!("📦 Step 4/4: Deploying Cloudflare Worker...");
        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://school-api.schoolorbit.app".to_string());

        // Create Cloudflare client for deployment
        use crate::clients::cloudflare_client::CloudflareClient;
        let cloudflare_client = CloudflareClient::new()
            .map_err(|e| AppError::ExternalServiceError(format!("Cloudflare client error: {}", e)))?;

        let subdomain_url = match cloudflare_client
            .deploy_worker(&data.subdomain, &school_id.to_string(), &api_url)
            .await
        {
            Ok(url) => {
                println!("✅ Worker deployed successfully: {}", url);
                url
            }
            Err(e) => {
                eprintln!("❌ Failed to deploy worker: {}", e);
                // Update school status to 'failed'
                let _ = sqlx::query(
                    "UPDATE schools SET status = 'deployment_failed' WHERE id = $1"
                )
                .bind(school_id)
                .execute(&self.pool)
                .await;

                return Err(AppError::ExternalServiceError(format!("Worker deployment failed: {}", e)));
            }
        };

        // Update school record with deployment info
        let mut config = serde_json::json!({
            "db_id": db_id,
            "dns_record_id": dns_record_id,
            "deployment_url": subdomain_url,
        });

        let updated_school = sqlx::query_as::<_, School>(
            r#"
            UPDATE schools 
            SET status = 'active', config = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING *
            "#
        )
        .bind(&config)
        .bind(school_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        println!("🎉 School provisioning completed successfully!");
        println!("   School ID: {}", school_id);
        println!("   Subdomain URL: {}", subdomain_url);

        Ok(updated_school)
    }


    pub async fn list_schools(
        &self,
        page: i64,
        limit: i64,
    ) -> Result<(Vec<School>, i64), AppError> {
        let offset = (page - 1) * limit;

        let schools = sqlx::query_as::<_, School>(
            "SELECT * FROM schools ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM schools")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok((schools, total))
    }

    pub async fn get_school(&self, id: Uuid) -> Result<School, AppError> {
        let school = sqlx::query_as::<_, School>("SELECT * FROM schools WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("School not found".to_string()))?;

        Ok(school)
    }

    pub async fn get_school_by_subdomain(&self, subdomain: &str) -> Result<School, AppError> {
        let school = sqlx::query_as::<_, School>(
            "SELECT * FROM schools WHERE subdomain = $1"
        )
        .bind(subdomain)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("School not found".to_string()))?;

        Ok(school)
    }

    pub async fn update_school(
        &self,
        id: Uuid,
        data: UpdateSchool,
    ) -> Result<School, AppError> {
        // Start building the update query dynamically
        let mut query = String::from("UPDATE schools SET updated_at = NOW()");
        let mut bind_count = 1;

        if data.name.is_some() {
            query.push_str(&format!(", name = ${}", bind_count));
            bind_count += 1;
        }
        if data.status.is_some() {
            query.push_str(&format!(", status = ${}", bind_count));
            bind_count += 1;
        }
        if data.config.is_some() {
            query.push_str(&format!(", config = ${}", bind_count));
            bind_count += 1;
        }

        query.push_str(&format!(" WHERE id = ${} RETURNING *", bind_count));

        let mut q = sqlx::query_as::<_, School>(&query);

        if let Some(name) = data.name {
            q = q.bind(name);
        }
        if let Some(status) = data.status {
            q = q.bind(status);
        }
        if let Some(config) = data.config {
            q = q.bind(config);
        }

        q = q.bind(id);

        let school = q
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("School not found".to_string()))?;

        Ok(school)
    }

    pub async fn delete_school(&self, id: Uuid) -> Result<(), AppError> {
        println!("🗑️  Starting school deletion for ID: {}", id);
        
        // Get school info first
        let school = self.get_school(id).await?;
        
        println!("   School: {}", school.name);
        println!("   Subdomain: {}", school.subdomain);
        
        // Parse config to get resource IDs
        let config = school.config.as_object();
        let db_id = config.and_then(|c| c.get("db_id")).and_then(|v| v.as_i64());
        let dns_record_id = config.and_then(|c| c.get("dns_record_id")).and_then(|v| v.as_str());
        
        // Step 1: Delete Cloudflare Worker
        println!("📦 Step 1/4: Deleting Cloudflare Worker...");
        use crate::clients::cloudflare_client::CloudflareClient;
        
        if let Ok(cf_client) = CloudflareClient::new() {
            let worker_name = format!("schoolorbit-school-{}", school.subdomain);
            match cf_client.delete_worker(&worker_name).await {
                Ok(_) => println!("   ✅ Worker deleted: {}", worker_name),
                Err(e) => {
                    eprintln!("   ⚠️  Failed to delete Worker: {}", e);
                    eprintln!("   Continuing with deletion...");
                }
            }
        } else {
            eprintln!("   ⚠️  Cloudflare client not available");
        }
        
        // Step 2: Delete DNS record (if exists)
        println!("📦 Step 2/4: Deleting DNS record...");
        if let Some(dns_id) = dns_record_id {
            if !dns_id.is_empty() {
                if let Ok(cf_client) = CloudflareClient::new() {
                    match cf_client.delete_dns_record(dns_id).await {
                        Ok(_) => println!("   ✅ DNS record deleted: {}", dns_id),
                        Err(e) => {
                            eprintln!("   ⚠️  Failed to delete DNS: {}", e);
                        }
                    }
                }
            } else {
                println!("   ⏭️  No DNS record ID (managed by Wrangler)");
            }
        } else {
            println!("   ⏭️  No DNS record to delete");
        }
        
        // Step 3: Delete Neon database
        println!("📦 Step 3/4: Deleting Neon database...");
        if let Some(neon_db_id) = db_id {
            use crate::clients::neon_client::NeonClient;
            
            if let Ok(neon_client) = NeonClient::new() {
                match neon_client.delete_database(neon_db_id).await {
                    Ok(_) => println!("   ✅ Database deleted: {}", neon_db_id),
                    Err(e) => {
                        eprintln!("   ⚠️  Failed to delete database: {}", e);
                        eprintln!("   You may need to delete manually from Neon console");
                    }
                }
            } else {
                eprintln!("   ⚠️  Neon client not available");
            }
        } else {
            println!("   ⏭️  No database ID found");
        }
        
        // Step 4: Delete school record from database
        println!("📦 Step 4/4: Deleting school record...");
        let result = sqlx::query("DELETE FROM schools WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("School not found".to_string()));
        }
        
        println!("   ✅ School record deleted from database");
        println!("🎉 School deletion completed!");

        Ok(())
    }

    /// Deploy or redeploy frontend for a school
    pub async fn deploy_school(&self, school_id: Uuid) -> Result<crate::models::DeployResponse, AppError> {
        use crate::clients::cloudflare_client::CloudflareClient;
        
        let school = self.get_school(school_id).await?;
        
        if school.status != "active" {
            return Err(AppError::ValidationError(
                "Cannot deploy: School is not active".to_string()
            ));
        }
        
        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://school-api.schoolorbit.app".to_string());
        let github_repo = std::env::var("GITHUB_REPO")
            .unwrap_or_else(|_| "akephisit/schoolorbit-new".to_string());
        
        let history_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO deployment_history (school_id, status, message) 
             VALUES ($1, 'pending', 'Deployment triggered') 
             RETURNING id"
        )
        .bind(school_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        let cloudflare_client = CloudflareClient::new()
            .map_err(|e| AppError::ExternalServiceError(e))?;
        
        match cloudflare_client.deploy_worker(&school.subdomain, &school_id.to_string(), &api_url).await {
            Ok(deployment_url) => {
                let github_actions_url = format!("https://github.com/{}/actions", github_repo);
                
                sqlx::query(
                    "UPDATE deployment_history 
                     SET status = 'in_progress', github_run_url = $2
                     WHERE id = $1"
                )
                .bind(history_id)
                .bind(&github_actions_url)
                .execute(&self.pool)
                .await
                .ok();
                
                Ok(crate::models::DeployResponse {
                    success: true,
                    message: "Deployment triggered successfully".to_string(),
                    deployment_url: Some(deployment_url),
                    github_actions_url: Some(github_actions_url),
                })
            }
            Err(e) => {
                sqlx::query(
                    "UPDATE deployment_history 
                     SET status = 'failed', message = $2, completed_at = NOW()
                     WHERE id = $1"
                )
                .bind(history_id)
                .bind(&e)
                .execute(&self.pool)
                .await
                .ok();
                
                Err(AppError::ExternalServiceError(e))
            }
        }
    }
    
    pub async fn bulk_deploy_schools(&self, school_ids: Vec<Uuid>) -> Result<crate::models::BulkDeployResult, AppError> {
        let mut successful = Vec::new();
        let mut failed = Vec::new();
        
        for school_id in &school_ids {
            match self.deploy_school(*school_id).await {
                Ok(response) => {
                    let school = self.get_school(*school_id).await?;
                    successful.push(crate::models::DeployResult {
                        school_id: *school_id,
                        school_name: school.name,
                        success: true,
                        message: response.message,
                        deployment_url: response.deployment_url,
                    });
                }
                Err(e) => {
                    let school = self.get_school(*school_id).await.ok();
                    failed.push(crate::models::DeployResult {
                        school_id: *school_id,
                        school_name: school.map(|s| s.name).unwrap_or_else(|| "Unknown".to_string()),
                        success: false,
                        message: e.to_string(),
                        deployment_url: None,
                    });
                }
            }
        }
        
        Ok(crate::models::BulkDeployResult {
            total: school_ids.len(),
            successful,
            failed,
        })
    }
    
    pub async fn get_deployment_history(&self, school_id: Uuid) -> Result<Vec<crate::models::DeploymentHistory>, AppError> {
        let history = sqlx::query_as::<_, crate::models::DeploymentHistory>(
            "SELECT id, school_id, status, message, github_run_id, github_run_url, created_at, completed_at
             FROM deployment_history
             WHERE school_id = $1
             ORDER BY created_at DESC
             LIMIT 50"
        )
        .bind(school_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        
        Ok(history)
    }
}
