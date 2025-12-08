use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
pub struct CreateDatabaseRequest {
    pub database: DatabaseConfig,
}

#[derive(Debug, Serialize)]
pub struct DatabaseConfig {
    pub name: String,
    pub owner_name: String,
}

#[derive(Debug, Deserialize)]
pub struct NeonResponse {
    pub database: DatabaseInfo,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseInfo {
    pub id: i64,
    pub name: String,
    pub owner_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionDetails {
    pub connection_uri: String,
}

pub struct NeonClient {
    api_key: String,
    project_id: String,
    client: reqwest::Client,
}

impl NeonClient {
    pub fn new() -> Result<Self, String> {
        let api_key = env::var("NEON_API_KEY")
            .map_err(|_| "NEON_API_KEY not set".to_string())?;
        let project_id = env::var("NEON_PROJECT_ID")
            .map_err(|_| "NEON_PROJECT_ID not set".to_string())?;

        Ok(Self {
            api_key,
            project_id,
            client: reqwest::Client::new(),
        })
    }

    /// Create a new database in Neon project
    pub async fn create_database(&self, db_name: &str) -> Result<String, String> {
        let url = format!(
            "https://console.neon.tech/api/v2/projects/{}/databases",
            self.project_id
        );

        let request = CreateDatabaseRequest {
            database: DatabaseConfig {
                name: db_name.to_string(),
                owner_name: "neondb_owner".to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to create database: {}", e))?;

        if response.status().is_success() {
            let result: NeonResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            // Get connection string
            let connection_string = self.get_connection_string(&result.database.name).await?;
            Ok(connection_string)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Neon API error: {}", error_text))
        }
    }

    /// Get connection string for a database
    async fn get_connection_string(&self, db_name: &str) -> Result<String, String> {
        // Construct connection string from environment and db_name
        let base_url = env::var("NEON_HOST")
            .unwrap_or_else(|_| "ep-xyz.us-east-2.aws.neon.tech".to_string());
        let user = env::var("NEON_USER").unwrap_or_else(|_| "neondb_owner".to_string());
        let password = env::var("NEON_PASSWORD").unwrap_or_else(|_| "".to_string());

        let connection_string = format!(
            "postgresql://{}:{}@{}/{}?sslmode=require",
            user, password, base_url, db_name
        );

        Ok(connection_string)
    }

    /// Run minimal initial setup on new database
    /// Full migrations will be handled by backend-school at startup
    pub async fn run_initial_setup(&self, connection_string: &str) -> Result<(), String> {
        // Connect to database
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(connection_string)
            .await
            .map_err(|e| format!("Failed to connect to database: {}", e))?;

        // Run minimal setup - just enable extensions
        // backend-school will handle full schema migrations
        let minimal_setup = r#"
            -- Enable UUID extension for primary keys
            CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
            
            -- Schema version tracking (for backend-school migrations)
            CREATE TABLE IF NOT EXISTS _sqlx_migrations (
                version BIGINT PRIMARY KEY,
                description TEXT NOT NULL,
                installed_on TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                success BOOLEAN NOT NULL,
                checksum BYTEA NOT NULL,
                execution_time BIGINT NOT NULL
            );
        "#;

        sqlx::raw_sql(minimal_setup)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to run initial setup: {}", e))?;

        println!("  ✅ Database created with minimal setup");
        println!("  ℹ️  Full schema will be created by backend-school on first start");

        Ok(())
    }

    /// Delete a database
    pub async fn delete_database(&self, db_name: &str) -> Result<(), String> {
        let url = format!(
            "https://console.neon.tech/api/v2/projects/{}/databases/{}",
            self.project_id, db_name
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("Failed to delete database: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Neon API error: {}", error_text))
        }
    }
}
