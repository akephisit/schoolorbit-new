use crate::models::School;
use crate::services::cloudflare::CloudflareClient;
use sqlx::PgPool;
use uuid::Uuid;

pub struct DeploymentService {
    cloudflare: CloudflareClient,
    pool: PgPool,
}

impl DeploymentService {
    pub fn new(pool: PgPool) -> Result<Self, String> {
        let cloudflare = CloudflareClient::new()?;

        Ok(Self {
            cloudflare,
            pool,
        })
    }

    /// Deploy complete school infrastructure
    pub async fn deploy_school(&self, school: &School) -> Result<(), String> {
        println!("🚀 Starting deployment for school: {}", school.name);

        // Step 1: Delegate database creation to backend-school
        println!("  📊 Requesting backend-school to create database...");
        let connection_string = self
            .create_school_database(school)
            .await
            .map_err(|e| format!("Database creation failed: {}", e))?;

        // Step 3: Update school record with connection string
        println!("  💾 Updating school record...");
        self.update_connection_string(school.id, &connection_string)
            .await?;

        // Step 4: Deploy Worker
        println!("  ☁️  Deploying Cloudflare Worker...");
        let worker_script = self.generate_worker_script(school, &connection_string);
        self.cloudflare
            .deploy_worker(&format!("school-{}", school.subdomain), &worker_script)
            .await
            .map_err(|e| format!("Worker deployment failed: {}", e))?;

        // Step 5: Create DNS record
        println!("  🌐 Creating DNS record...");
        let zone_id = std::env::var("CLOUDFLARE_ZONE_ID")
            .map_err(|_| "CLOUDFLARE_ZONE_ID not set".to_string())?;
        let subdomain_full = format!("{}.schoolorbit.app", school.subdomain);
        
        self.cloudflare
            .create_dns_record(&zone_id, &subdomain_full)
            .await
            .map_err(|e| format!("DNS creation failed: {}", e))?;

        // Step 6: Create route
        println!("  🛣️  Creating Workers route...");
        let pattern = format!("{}/*", subdomain_full);
        self.cloudflare
            .create_route(&zone_id, &pattern, &format!("school-{}", school.subdomain))
            .await
            .map_err(|e| format!("Route creation failed: {}", e))?;

        println!("✅ Deployment completed for {}", school.name);
        println!("   URL: https://{}", subdomain_full);

        Ok(())
    }

    /// Undeploy school (delete all resources)
    /// Note: Database cleanup should be done via backend-school
    pub async fn undeploy_school(&self, school: &School) -> Result<(), String> {
        println!("🗑️  Undeploying school: {}", school.name);

        // TODO: Call backend-school to delete database
        // For now, manual cleanup may be needed

        Ok(())
    }

    async fn update_connection_string(
        &self,
        school_id: Uuid,
        connection_string: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "UPDATE schools SET db_connection_string = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(connection_string)
        .bind(school_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update connection string: {}", e))?;

        Ok(())
    }

    fn generate_worker_script(&self, school: &School, db_connection: &str) -> String {
        // Generate a simple Worker script for the school
        // In production, this would be a built SvelteKit app
        format!(
            r#"
export default {{
  async fetch(request, env) {{
    return new Response(JSON.stringify({{
      school: "{}",
      subdomain: "{}",
      status: "active",
      message: "Welcome to {} - School Management System"
    }}), {{
      headers: {{
        "Content-Type": "application/json",
        "Access-Control-Allow-Origin": "*"
      }}
    }});
  }}
}}
            "#,
            school.name, school.subdomain, school.name
        )
    }

    async fn create_school_database(&self, school: &School) -> Result<String, String> {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize)]
        struct CreateDatabaseRequest {
            #[serde(rename = "schoolName")]
            school_name: String,
            subdomain: String,
        }

        #[derive(Deserialize)]
        struct CreateDatabaseResponse {
            success: bool,
            message: String,
            database_name: String,
            connection_string: String,
            tables_created: Vec<String>,
        }

        let backend_school_url = std::env::var("BACKEND_SCHOOL_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string());

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/v1/create-school-database", backend_school_url))
            .json(&CreateDatabaseRequest {
                school_name: school.name.clone(),
                subdomain: school.subdomain.clone(),
            })
            .send()
            .await
            .map_err(|e| format!("Failed to call backend-school: {}", e))?;

        if response.status().is_success() {
            let db_response: CreateDatabaseResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            println!("  ✅ {}", db_response.message);
            println!("  📊 Database: {}", db_response.database_name);
            println!("  📋 Tables: {}", db_response.tables_created.join(", "));
            
            Ok(db_response.connection_string)
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Backend-school returned error: {}", error_text))
        }
    }
}
