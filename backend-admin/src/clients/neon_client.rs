use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::env;

#[derive(Debug, Serialize)]
struct CreateDatabaseRequest {
    database: DatabaseConfig,
}

#[derive(Debug, Serialize)]
struct DatabaseConfig {
    name: String,
    owner_name: String,
}

#[derive(Debug, Deserialize)]
struct CreateDatabaseResponse {
    database: DatabaseInfo,
}

#[derive(Debug, Deserialize)]
struct DatabaseInfo {
    id: i64,  // Only field we actually use
}

pub struct NeonClient {
    client: Client,
    api_key: String,
    project_id: String,
    branch_id: String,
}

impl NeonClient {
    pub fn new() -> Result<Self, String> {
        let api_key = env::var("NEON_API_KEY")
            .map_err(|_| "NEON_API_KEY not set".to_string())?;
        let project_id = env::var("NEON_PROJECT_ID")
            .map_err(|_| "NEON_PROJECT_ID not set".to_string())?;
        let branch_id = env::var("NEON_BRANCH_ID")
            .unwrap_or_else(|_| "main".to_string());

        Ok(Self {
            client: Client::new(),
            api_key,
            project_id,
            branch_id,
        })
    }

    /// Create a new database in Neon
    pub async fn create_database(
        &self,
        db_name: &str,
        owner_name: &str,
    ) -> Result<i64, String> {  // Changed from String to i64
        let url = format!(
            "https://console.neon.tech/api/v2/projects/{}/branches/{}/databases",
            self.project_id, self.branch_id
        );

        let request_body = CreateDatabaseRequest {
            database: DatabaseConfig {
                name: db_name.to_string(),
                owner_name: owner_name.to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!(
                "Neon API error ({}): {}",
                status, error_text
            ));
        }

        // Get response text first for debugging
        let response_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;
        
        println!("Neon API Response: {}", response_text);

        // Try to parse the response
        let response_data: CreateDatabaseResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse response: {}. Response was: {}", e, response_text))?;

        Ok(response_data.database.id)
    }

    /// Get connection string for a database
    pub fn get_connection_string(
        &self,
        db_name: &str,
        db_user: &str,
        db_password: &str,
    ) -> String {
        // Neon connection string format
        // postgres://user:password@host/dbname?sslmode=require
        let host = env::var("NEON_HOST")
            .unwrap_or_else(|_| format!("{}.neon.tech", self.project_id));

        format!(
            "postgresql://{}:{}@{}/{}?sslmode=require",
            db_user, db_password, host, db_name
        )
    }


    /// Delete a database by name (not ID!)
    /// Neon API requires database name, not ID
    pub async fn delete_database_by_name(&self, db_name: &str) -> Result<(), String> {
        let url = format!(
            "https://console.neon.tech/api/v2/projects/{}/branches/{}/databases/{}",
            self.project_id, self.branch_id, db_name
        );

        let max_attempts = 12;

        for attempt in 1..=max_attempts {
            println!("      Neon API: DELETE {} (attempt {}/{})", url, attempt, max_attempts);

            let response = self
                .client
                .delete(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .map_err(|e| format!("Failed to delete database: {}", e))?;

            let status = response.status();
            println!("      Neon API Response: {}", status);

            if status.is_success() {
                let response_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "{}".to_string());

                println!("      Neon API Success: {}", response_text);
                return Ok(());
            }

            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            
            println!("      Neon API Error: {}", error_text);

            let is_locked = status.as_u16() == 423
                || error_text.contains("conflicting operations")
                || error_text.contains("Locked");

            if is_locked && attempt < max_attempts {
                println!("      Neon project is locked by another operation; retrying in 5s...");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
            
            return Err(format!(
                "Failed to delete database ({}): {}",
                status, error_text
            ));
        }

        Err("Failed to delete database: retry attempts exhausted".to_string())
    }

    /// Delete a database by ID (deprecated - use delete_database_by_name)
    /// Kept for backward compatibility
    pub async fn delete_database(&self, db_id: i64) -> Result<(), String> {

        let url = format!(
            "https://console.neon.tech/api/v2/projects/{}/branches/{}/databases/{}",
            self.project_id, self.branch_id, db_id
        );

        println!("      Neon API: DELETE {}", url);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("Failed to delete database: {}", e))?;

        let status = response.status();
        println!("      Neon API Response: {}", status);

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            
            println!("      Neon API Error: {}", error_text);
            
            return Err(format!(
                "Failed to delete database ({}): {}",
                status, error_text
            ));
        }

        // Get response body for verification
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|_| "{}".to_string());
        
        println!("      Neon API Success: {}", response_text);

        Ok(())
    }

    /// Wait for database to be ready
    /// Neon creates databases asynchronously, so we need to wait for it to be ready
    pub async fn wait_for_database_ready(&self, db_name: &str) -> Result<(), String> {
        println!("⏳ Waiting for database to be ready...");
        
        let max_attempts = 30; // 30 seconds max
        let mut attempts = 0;
        
        while attempts < max_attempts {
            attempts += 1;
            
            // Check if database exists and is ready
            let url = format!(
                "https://console.neon.tech/api/v2/projects/{}/branches/{}/databases",
                self.project_id, self.branch_id
            );
            
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .map_err(|e| format!("Failed to check database status: {}", e))?;
            
            if response.status().is_success() {
                let text = response.text().await
                    .map_err(|e| format!("Failed to read response: {}", e))?;
                
                // Check if our database is in the list
                if text.contains(db_name) {
                    println!("✅ Database is listed in Neon API");
                    return Ok(());
                }
            }
            
            // Wait 1 second before retry
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            if attempts % 5 == 0 {
                println!("   Still waiting... ({}/{})", attempts, max_attempts);
            }
        }
        
        Err(format!("Timeout waiting for database '{}' to be ready", db_name))
    }

    /// Wait until PostgreSQL accepts connections to the newly created database.
    ///
    /// Neon can return the database in the API list while its create operation is
    /// still running. A real connection check prevents provisioning from racing
    /// ahead into a database that is listed but not usable yet.
    pub async fn wait_for_database_connectable(&self, database_url: &str) -> Result<(), String> {
        println!("⏳ Waiting for database to accept connections...");

        let max_attempts = 60;

        for attempt in 1..=max_attempts {
            match PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    pool.close().await;
                    println!("✅ Database accepts PostgreSQL connections");
                    return Ok(());
                }
                Err(e) => {
                    if attempt % 5 == 0 {
                        println!(
                            "   Still waiting for database connection... ({}/{}) last error: {}",
                            attempt, max_attempts, e
                        );
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Err("Timeout waiting for database to accept PostgreSQL connections".to_string())
    }
}
