use crate::models::School;
use crate::services::cloudflare::CloudflareClient;
use crate::services::neon::NeonClient;
use sqlx::PgPool;
use uuid::Uuid;

pub struct DeploymentService {
    cloudflare: CloudflareClient,
    neon: NeonClient,
    pool: PgPool,
}

impl DeploymentService {
    pub fn new(pool: PgPool) -> Result<Self, String> {
        let cloudflare = CloudflareClient::new()?;
        let neon = NeonClient::new()?;

        Ok(Self {
            cloudflare,
            neon,
            pool,
        })
    }

    /// Deploy complete school infrastructure
    pub async fn deploy_school(&self, school: &School) -> Result<(), String> {
        println!("🚀 Starting deployment for school: {}", school.name);

        // Step 1: Create database
        println!("  📊 Creating database...");
        let connection_string = self
            .neon
            .create_database(&school.db_name)
            .await
            .map_err(|e| format!("Database creation failed: {}", e))?;

        // Step 2: Run migrations
        println!("  🔧 Running migrations...");
        self.neon
            .run_migrations(&connection_string)
            .await
            .map_err(|e| format!("Migration failed: {}", e))?;

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
    pub async fn undeploy_school(&self, school: &School) -> Result<(), String> {
        println!("🗑️  Undeploying school: {}", school.name);

        // Delete database
        if let Err(e) = self.neon.delete_database(&school.db_name).await {
            eprintln!("Warning: Failed to delete database: {}", e);
        }

        // Note: Cloudflare Workers and DNS cleanup would go here
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
}
