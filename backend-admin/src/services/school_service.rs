use crate::error::AppError;
use crate::models::{CreateSchool, School, SchoolConfig, UpdateSchool};
use sqlx::{types::Json, PgPool};
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct SchoolService {
    pool: PgPool,
}

fn build_provisioning_failure_message(primary_error: &str, cleanup_errors: &[String]) -> String {
    if cleanup_errors.is_empty() {
        return primary_error.to_string();
    }

    format!(
        "{}. Rollback cleanup also failed: {}",
        primary_error,
        cleanup_errors.join("; ")
    )
}

fn validate_school_subdomain(subdomain: &str) -> Result<(), AppError> {
    if subdomain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Ok(());
    }

    Err(AppError::ValidationError(
        "Subdomain must contain only lowercase letters, numbers, and hyphens".to_string(),
    ))
}

fn build_school_database_name(subdomain: &str) -> String {
    format!("schoolorbit_{}", subdomain)
}

fn build_active_school_config(
    db_id: i64,
    dns_record_id: &str,
    deployment_url: &str,
) -> SchoolConfig {
    SchoolConfig {
        db_id: Some(db_id),
        dns_record_id: Some(dns_record_id.to_string()),
        deployment_url: Some(deployment_url.to_string()),
    }
}

enum ProvisioningReporter {
    Console,
    Sse(crate::utils::sse::SseLogger),
}

impl ProvisioningReporter {
    async fn info(&self, message: &str) {
        match self {
            Self::Console => info!("{}", message),
            Self::Sse(logger) => logger.info(message).await,
        }
    }

    async fn success(&self, message: &str) {
        match self {
            Self::Console => info!("{}", message),
            Self::Sse(logger) => logger.success(message).await,
        }
    }

    async fn warning(&self, message: &str) {
        match self {
            Self::Console => warn!("{}", message),
            Self::Sse(logger) => logger.warning(message).await,
        }
    }

    async fn error(&self, message: &str) {
        match self {
            Self::Console => error!("{}", message),
            Self::Sse(logger) => logger.error(message).await,
        }
    }

    async fn progress(&self, step: u8, total: u8, message: &str) {
        match self {
            Self::Console => info!(step, total, "{}", message),
            Self::Sse(logger) => logger.progress(step, total, message).await,
        }
    }

    async fn complete(&self, data: serde_json::Value) {
        if let Self::Sse(logger) = self {
            logger.complete(data).await;
        }
    }
}

struct ProvisioningRunOptions {
    reporter: ProvisioningReporter,
    wait_for_deployment: bool,
    complete_on_success: bool,
}

impl ProvisioningRunOptions {
    fn api() -> Self {
        Self {
            reporter: ProvisioningReporter::Console,
            wait_for_deployment: false,
            complete_on_success: false,
        }
    }

    fn sse(logger: crate::utils::sse::SseLogger) -> Self {
        Self {
            reporter: ProvisioningReporter::Sse(logger),
            wait_for_deployment: true,
            complete_on_success: true,
        }
    }

    #[cfg(test)]
    fn sse_for_test() -> Self {
        Self {
            reporter: ProvisioningReporter::Console,
            wait_for_deployment: true,
            complete_on_success: true,
        }
    }

    async fn info(&self, message: &str) {
        self.reporter.info(message).await;
    }

    async fn success(&self, message: &str) {
        self.reporter.success(message).await;
    }

    async fn warning(&self, message: &str) {
        self.reporter.warning(message).await;
    }

    async fn error(&self, message: &str) {
        self.reporter.error(message).await;
    }

    async fn progress(&self, step: u8, total: u8, message: &str) {
        self.reporter.progress(step, total, message).await;
    }

    async fn complete_school(&self, school: &School) -> Result<(), AppError> {
        if self.complete_on_success {
            let payload = serde_json::to_value(school)
                .map_err(|e| AppError::InternalServerError(e.to_string()))?;
            self.reporter.complete(payload).await;
        }

        Ok(())
    }
}

impl SchoolService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn mark_school_failed(
        &self,
        school_id: Uuid,
        status: &str,
        reason: &str,
    ) -> Result<(), String> {
        warn!(%school_id, status, reason, "marking school status after provisioning error");

        match sqlx::query("UPDATE schools SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(status)
            .bind(school_id)
            .execute(&self.pool)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!(error = %e, "failed to update school status after provisioning error");
                Err(e.to_string())
            }
        }
    }

    async fn delete_school_record(&self, school_id: Uuid) -> Result<(), String> {
        let result = sqlx::query("DELETE FROM schools WHERE id = $1")
            .bind(school_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if result.rows_affected() == 0 {
            return Err("school record was already missing".to_string());
        }

        Ok(())
    }

    async fn rollback_failed_provisioning(
        &self,
        school_id: Option<Uuid>,
        neon_client: &crate::clients::neon_client::NeonClient,
        db_name: &str,
        reason: String,
    ) -> AppError {
        let mut cleanup_errors = Vec::new();

        if let Some(school_id) = school_id {
            if let Err(e) = self
                .mark_school_failed(school_id, "provision_failed", &reason)
                .await
            {
                cleanup_errors.push(format!(
                    "failed to mark school {} as provision_failed: {}",
                    school_id, e
                ));
            }
        }

        let database_deleted = match neon_client.delete_database_by_name(db_name).await {
            Ok(_) => true,
            Err(e) => {
                cleanup_errors.push(format!(
                    "failed to delete Neon database '{}': {}",
                    db_name, e
                ));
                false
            }
        };

        if database_deleted {
            if let Some(school_id) = school_id {
                if let Err(e) = self.delete_school_record(school_id).await {
                    cleanup_errors.push(format!(
                        "failed to delete school record {}: {}",
                        school_id, e
                    ));
                }
            }
        }

        AppError::ExternalServiceError(build_provisioning_failure_message(&reason, &cleanup_errors))
    }

    async fn mark_school_status_error(
        &self,
        school_id: Uuid,
        status: &str,
        reason: String,
    ) -> AppError {
        let cleanup_errors = match self.mark_school_failed(school_id, status, &reason).await {
            Ok(_) => Vec::new(),
            Err(e) => vec![format!(
                "failed to mark school {} as {}: {}",
                school_id, status, e
            )],
        };

        AppError::ExternalServiceError(build_provisioning_failure_message(&reason, &cleanup_errors))
    }

    async fn provision_tenant_database(
        &self,
        client: &crate::clients::backend_school_client::BackendSchoolClient,
        school_id: Uuid,
        db_connection_string: &str,
        data: &CreateSchool,
    ) -> Result<(), String> {
        client
            .provision_tenant(
                &school_id.to_string(),
                db_connection_string,
                &data.subdomain,
                data.admin_username.as_deref(),
                &data.admin_password,
                &data.admin_title,
                &data.admin_first_name,
                &data.admin_last_name,
            )
            .await
            .map(|_| ())
    }

    pub async fn create_school(&self, data: CreateSchool) -> Result<School, AppError> {
        self.provision_school(data, ProvisioningRunOptions::api())
            .await
    }

    async fn provision_school(
        &self,
        data: CreateSchool,
        options: ProvisioningRunOptions,
    ) -> Result<School, AppError> {
        const TOTAL_STEPS: u8 = 5;

        options
            .info(&format!(
                "🚀 Starting school provisioning for: {}",
                data.name
            ))
            .await;
        options
            .progress(1, TOTAL_STEPS, "Validating input...")
            .await;

        // Validate subdomain format (lowercase, alphanumeric, hyphens)
        validate_school_subdomain(&data.subdomain)?;

        // Check if subdomain already exists
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM schools WHERE subdomain = $1)",
        )
        .bind(&data.subdomain)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if exists {
            return Err(AppError::ValidationError(
                "Subdomain นี้มีในระบบแล้ว กรุณาใช้ชื่ออื่น".to_string(),
            ));
        }
        options.success("✅ Validation passed").await;

        // Generate database name
        let db_name = build_school_database_name(&data.subdomain);

        // Step 1: Create database in Neon
        options
            .progress(2, TOTAL_STEPS, "Creating database in Neon...")
            .await;
        use crate::clients::neon_client::NeonClient;
        let neon_client = NeonClient::new()
            .map_err(|e| AppError::ExternalServiceError(format!("Neon client error: {}", e)))?;

        let db_id = neon_client
            .create_database(&db_name, "neondb_owner")
            .await
            .map_err(|e| {
                AppError::ExternalServiceError(format!("Failed to create database: {}", e))
            })?;

        options
            .success(&format!("✅ Database created with ID: {}", db_id))
            .await;

        // Get Neon database password from environment
        // This is the password for neondb_owner role in your Neon project
        let db_password = match std::env::var("NEON_DB_PASSWORD") {
            Ok(value) => value,
            Err(_) => {
                let reason = "NEON_DB_PASSWORD not set. Get this from Neon console.".to_string();
                return Err(self
                    .rollback_failed_provisioning(None, &neon_client, &db_name, reason)
                    .await);
            }
        };

        let db_connection_string =
            neon_client.get_connection_string(&db_name, "neondb_owner", &db_password);

        // Wait for Neon to finish creating the database and for Postgres to accept connections.
        options.info("⏳ Waiting for database to be ready...").await;
        if let Err(e) = neon_client.wait_for_database_ready(&db_name).await {
            return Err(self
                .rollback_failed_provisioning(
                    None,
                    &neon_client,
                    &db_name,
                    format!("Database not ready: {}", e),
                )
                .await);
        }

        options
            .info("⏳ Waiting for database connection to be ready...")
            .await;
        if let Err(e) = neon_client
            .wait_for_database_connectable(&db_connection_string)
            .await
        {
            return Err(self
                .rollback_failed_provisioning(
                    None,
                    &neon_client,
                    &db_name,
                    format!("Database connection not ready: {}", e),
                )
                .await);
        }
        options.success("✅ Database is ready").await;

        // Create school record
        options
            .progress(3, TOTAL_STEPS, "Creating school record...")
            .await;
        let school = match sqlx::query_as::<_, School>(
            r#"
            INSERT INTO schools (name, subdomain, db_name, db_connection_string, status, config)
            VALUES ($1, $2, $3, $4, 'provisioning', '{}')
            RETURNING *
            "#,
        )
        .bind(&data.name)
        .bind(&data.subdomain)
        .bind(&db_name)
        .bind(&db_connection_string)
        .fetch_one(&self.pool)
        .await
        {
            Ok(school) => school,
            Err(e) => {
                return Err(self
                    .rollback_failed_provisioning(
                        None,
                        &neon_client,
                        &db_name,
                        format!("Failed to create school record: {}", e),
                    )
                    .await);
            }
        };

        let school_id = school.id;
        options
            .success("✅ School record created (provisioning)")
            .await;

        // Step 2: Provision tenant via backend-school
        options
            .progress(
                4,
                TOTAL_STEPS,
                "Provisioning tenant database via backend-school...",
            )
            .await;
        use crate::clients::backend_school_client::BackendSchoolClient;
        let backend_school_client = match BackendSchoolClient::new() {
            Ok(client) => client,
            Err(e) => {
                return Err(self
                    .rollback_failed_provisioning(
                        Some(school_id),
                        &neon_client,
                        &db_name,
                        format!("Backend-school client error: {}", e),
                    )
                    .await);
            }
        };

        match self
            .provision_tenant_database(
                &backend_school_client,
                school_id,
                &db_connection_string,
                &data,
            )
            .await
        {
            Ok(_) => {
                options
                    .success("✅ Tenant database provisioned successfully")
                    .await;
                options
                    .success(&format!(
                        "✅ Admin user created (Username: {:?})",
                        data.admin_username
                    ))
                    .await;
            }
            Err(e) => {
                options
                    .error(&format!("❌ Failed to provision tenant: {}", e))
                    .await;
                return Err(self
                    .rollback_failed_provisioning(
                        Some(school_id),
                        &neon_client,
                        &db_name,
                        format!("Tenant provisioning failed: {}", e),
                    )
                    .await);
            }
        }

        // Step 3: Create DNS record in Cloudflare
        // NOTE: Skipped! Wrangler (GitHub Actions) will create and manage DNS automatically
        // when deploying with custom_domain configuration
        options
            .progress(
                5,
                TOTAL_STEPS,
                "Deploying Cloudflare Worker and finalizing school setup...",
            )
            .await;
        options
            .info("Skipping DNS creation (Wrangler will handle this)")
            .await;
        let dns_record_id = "".to_string(); // Managed by Wrangler

        // Step 4: Deploy Cloudflare Worker
        options.info("Deploying Cloudflare Worker...").await;
        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://school-api.schoolorbit.app".to_string());

        // Create Cloudflare client for deployment
        use crate::clients::cloudflare_client::CloudflareClient;
        let cloudflare_client = match CloudflareClient::new() {
            Ok(client) => client,
            Err(e) => {
                return Err(self
                    .mark_school_status_error(
                        school_id,
                        "deployment_failed",
                        format!("Cloudflare client error: {}", e),
                    )
                    .await);
            }
        };

        let subdomain_url = match cloudflare_client
            .deploy_worker(&data.subdomain, &school_id.to_string(), &api_url)
            .await
        {
            Ok((url, trigger_time)) => {
                options
                    .success(&format!("✅ Worker deployment initiated: {}", url))
                    .await;

                if options.wait_for_deployment {
                    options.info("GitHub Actions workflow triggered").await;
                    options
                        .info("⏳ Waiting for deployment to complete (3-5 minutes)...")
                        .await;

                    match cloudflare_client
                        .wait_for_workflow_completion(&data.subdomain, trigger_time, 10)
                        .await
                    {
                        Ok(_) => {
                            options
                                .success("✅ GitHub Actions deployment completed!")
                                .await;
                        }
                        Err(e) => {
                            options
                                .warning(&format!("⚠️ Warning: Could not verify deployment: {}", e))
                                .await;
                            options
                                .info("Workflow may still be processing in background")
                                .await;
                            options
                                .info("Check: https://github.com/akephisit/schoolorbit-new/actions")
                                .await;
                        }
                    }
                }

                url
            }
            Err(e) => {
                options
                    .error(&format!("❌ Failed to deploy worker: {}", e))
                    .await;
                return Err(self
                    .mark_school_status_error(
                        school_id,
                        "deployment_failed",
                        format!("Worker deployment failed: {}", e),
                    )
                    .await);
            }
        };

        // Update school record with deployment info
        let config = build_active_school_config(db_id, &dns_record_id, &subdomain_url);

        let updated_school = sqlx::query_as::<_, School>(
            r#"
            UPDATE schools 
            SET status = 'active', config = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING *
            "#,
        )
        .bind(Json(config))
        .bind(school_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        options.success("✅ School activated").await;
        options
            .success("🎉 School provisioning completed successfully!")
            .await;
        options.info(&format!("School ID: {}", school_id)).await;
        options
            .info(&format!("Subdomain URL: {}", subdomain_url))
            .await;
        options.complete_school(&updated_school).await?;

        Ok(updated_school)
    }

    pub async fn list_schools(
        &self,
        page: i64,
        limit: i64,
    ) -> Result<(Vec<School>, i64), AppError> {
        let offset = (page - 1) * limit;

        let schools = sqlx::query_as::<_, School>(
            "SELECT * FROM schools ORDER BY created_at DESC LIMIT $1 OFFSET $2",
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
        let school = sqlx::query_as::<_, School>("SELECT * FROM schools WHERE subdomain = $1")
            .bind(subdomain)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("School not found".to_string()))?;

        Ok(school)
    }

    pub async fn update_school(&self, id: Uuid, data: UpdateSchool) -> Result<School, AppError> {
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
            q = q.bind(Json(config));
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
        info!(school_id = %id, "starting school deletion");

        // Get school info first
        let school = self.get_school(id).await?;

        info!(school = %school.name, subdomain = %school.subdomain, "loaded school for deletion");

        let db_id = school.config.db_id;
        let dns_record_id = school.config.dns_record_id.as_deref();

        // Step 1: Delete Cloudflare Worker
        info!("deleting Cloudflare Worker");
        use crate::clients::cloudflare_client::CloudflareClient;

        if let Ok(cf_client) = CloudflareClient::new() {
            let worker_name = format!("schoolorbit-school-{}", school.subdomain);
            match cf_client.delete_worker(&worker_name).await {
                Ok(_) => info!(worker_name, "worker deleted"),
                Err(e) => {
                    warn!(error = %e, "failed to delete Worker; continuing with deletion");
                }
            }
        } else {
            warn!("Cloudflare client not available");
        }

        // Step 2: Delete DNS record (if exists)
        info!("deleting DNS record");
        if let Some(dns_id) = dns_record_id {
            if !dns_id.is_empty() {
                if let Ok(cf_client) = CloudflareClient::new() {
                    match cf_client.delete_dns_record(dns_id).await {
                        Ok(_) => info!(dns_id, "DNS record deleted"),
                        Err(e) => {
                            warn!(error = %e, "failed to delete DNS record");
                        }
                    }
                }
            } else {
                info!("no DNS record ID (managed by Wrangler)");
            }
        } else {
            info!("no DNS record to delete");
        }

        // Step 3: Delete Neon database
        info!("deleting Neon database");

        // Construct database name from subdomain (same as creation)
        let db_name = format!("schoolorbit_{}", school.subdomain);

        info!(db_name, db_id = ?db_id, "deleting tenant database");

        use crate::clients::neon_client::NeonClient;

        if let Ok(neon_client) = NeonClient::new() {
            match neon_client.delete_database_by_name(&db_name).await {
                Ok(_) => info!(db_name, "database deleted"),
                Err(e) => {
                    warn!(error = %e, "failed to delete database; manual cleanup may be needed");
                }
            }
        } else {
            warn!("Neon client not available");
        }

        // Step 4: Delete school record from database
        info!("deleting school record");
        let result = sqlx::query("DELETE FROM schools WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("School not found".to_string()));
        }

        info!("school deletion completed");

        Ok(())
    }

    /// Deploy or redeploy frontend for a school
    pub async fn deploy_school(
        &self,
        school_id: Uuid,
    ) -> Result<crate::models::DeployResponse, AppError> {
        use crate::clients::cloudflare_client::CloudflareClient;

        let school = self.get_school(school_id).await?;

        if school.status != "active" {
            return Err(AppError::ValidationError(
                "Cannot deploy: School is not active".to_string(),
            ));
        }

        let api_url = std::env::var("API_URL")
            .unwrap_or_else(|_| "https://school-api.schoolorbit.app".to_string());
        let github_repo = std::env::var("GITHUB_REPO")
            .unwrap_or_else(|_| "akephisit/schoolorbit-new".to_string());

        let history_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO deployment_history (school_id, status, message) 
             VALUES ($1, 'pending', 'Deployment triggered') 
             RETURNING id",
        )
        .bind(school_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let cloudflare_client =
            CloudflareClient::new().map_err(|e| AppError::ExternalServiceError(e))?;

        match cloudflare_client
            .deploy_worker(&school.subdomain, &school_id.to_string(), &api_url)
            .await
        {
            Ok((deployment_url, _trigger_time)) => {
                let github_actions_url = format!("https://github.com/{}/actions", github_repo);

                sqlx::query(
                    "UPDATE deployment_history 
                     SET status = 'in_progress', github_run_url = $2
                     WHERE id = $1",
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
                     WHERE id = $1",
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

    pub async fn bulk_deploy_schools(
        &self,
        school_ids: Vec<Uuid>,
    ) -> Result<crate::models::BulkDeployResult, AppError> {
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
                        school_name: school
                            .map(|s| s.name)
                            .unwrap_or_else(|| "Unknown".to_string()),
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

    pub async fn get_deployment_history(
        &self,
        school_id: Uuid,
    ) -> Result<Vec<crate::models::DeploymentHistory>, AppError> {
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

    /// Create school with SSE logging for real-time progress
    pub async fn create_school_stream(
        &self,
        data: CreateSchool,
        logger: crate::utils::sse::SseLogger,
    ) -> Result<School, AppError> {
        self.provision_school(data, ProvisioningRunOptions::sse(logger))
            .await
    }

    /// Delete school with SSE logging for real-time progress  
    pub async fn delete_school_stream(
        &self,
        id: Uuid,
        logger: crate::utils::sse::SseLogger,
    ) -> Result<(), AppError> {
        logger
            .info(&format!("🗑️  Starting deletion for ID: {}", id))
            .await;
        logger.progress(0, 4, "Getting school info...").await;

        let school = self.get_school(id).await?;

        logger.info(&format!("School: {}", school.name)).await;
        logger
            .info(&format!("Subdomain: {}", school.subdomain))
            .await;

        // Step 1: Delete Cloudflare Worker
        logger.progress(1, 4, "Deleting Cloudflare Worker...").await;

        use crate::clients::cloudflare_client::CloudflareClient;
        if let Ok(cf_client) = CloudflareClient::new() {
            let worker_name = format!("schoolorbit-school-{}", school.subdomain);
            match cf_client.delete_worker(&worker_name).await {
                Ok(_) => {
                    logger
                        .success(&format!("✅ Worker deleted: {}", worker_name))
                        .await
                }
                Err(e) => {
                    logger
                        .warning(&format!("⚠️  Worker deletion failed: {}", e))
                        .await
                }
            }
        }

        // Step 2: Skip DNS (managed by Wrangler)
        logger.progress(2, 4, "Skipping DNS cleanup...").await;
        logger.info("⏭️  DNS managed by Wrangler").await;

        // Step 3: Delete Neon database
        logger.progress(3, 4, "Deleting Neon database...").await;

        let db_name = format!("schoolorbit_{}", school.subdomain);
        logger.info(&format!("Database name: {}", db_name)).await;

        use crate::clients::neon_client::NeonClient;
        if let Ok(neon_client) = NeonClient::new() {
            match neon_client.delete_database_by_name(&db_name).await {
                Ok(_) => {
                    logger
                        .success(&format!("✅ Database deleted: {}", db_name))
                        .await
                }
                Err(e) => {
                    logger
                        .warning(&format!("⚠️  Database deletion failed: {}", e))
                        .await
                }
            }
        }

        // Step 4: Delete school record
        logger.progress(4, 4, "Deleting school record...").await;

        let result = sqlx::query("DELETE FROM schools WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("School not found".to_string()));
        }

        logger.success("✅ School record deleted").await;
        logger.success("🎉 Deletion completed!").await;
        logger.complete(serde_json::json!({"deleted": true})).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_active_school_config, build_provisioning_failure_message, build_school_database_name,
        validate_school_subdomain, ProvisioningRunOptions,
    };

    #[test]
    fn provisioning_failure_message_keeps_primary_error_when_cleanup_succeeds() {
        let message = build_provisioning_failure_message("Tenant provisioning failed", &[]);

        assert_eq!(message, "Tenant provisioning failed");
    }

    #[test]
    fn provisioning_failure_message_includes_cleanup_errors() {
        let cleanup_errors = vec![
            "failed to delete Neon database 'schoolorbit_test': locked".to_string(),
            "failed to delete school record: connection closed".to_string(),
        ];

        let message =
            build_provisioning_failure_message("Tenant provisioning failed", &cleanup_errors);

        assert!(message.contains("Tenant provisioning failed"));
        assert!(message.contains("Rollback cleanup also failed"));
        assert!(message.contains("failed to delete Neon database 'schoolorbit_test': locked"));
        assert!(message.contains("failed to delete school record: connection closed"));
    }

    #[test]
    fn school_database_name_uses_schoolorbit_prefix() {
        assert_eq!(build_school_database_name("sandbox"), "schoolorbit_sandbox");
    }

    #[test]
    fn active_school_config_records_database_and_deployment_url() {
        let config = build_active_school_config(42, "", "https://sandbox.schoolorbit.app");

        assert_eq!(config.db_id, Some(42));
        assert_eq!(config.dns_record_id.as_deref(), Some(""));
        assert_eq!(
            config.deployment_url.as_deref(),
            Some("https://sandbox.schoolorbit.app")
        );
    }

    #[test]
    fn subdomain_validation_rejects_symbols() {
        let error = validate_school_subdomain("bad_domain").unwrap_err();

        assert!(error
            .to_string()
            .contains("Subdomain must contain only lowercase letters"));
    }

    #[test]
    fn provisioning_options_keep_api_and_sse_completion_behavior_separate() {
        let api_options = ProvisioningRunOptions::api();

        assert!(!api_options.wait_for_deployment);
        assert!(!api_options.complete_on_success);

        let sse_options = ProvisioningRunOptions::sse_for_test();

        assert!(sse_options.wait_for_deployment);
        assert!(sse_options.complete_on_success);
    }
}
