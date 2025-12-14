use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
struct CreateDnsRecordRequest {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
    proxied: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CreateDnsRecordResponse {
    result: DnsRecord,
    success: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DnsRecord {
    id: String,
    name: String,
}

pub struct CloudflareClient {
    client: Client,
    api_token: String,
    zone_id: String,
    account_id: String,
    base_domain: String,
}

impl CloudflareClient {
    pub fn new() -> Result<Self, String> {
        let api_token = env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| "CLOUDFLARE_API_TOKEN not set".to_string())?;
        let zone_id = env::var("CLOUDFLARE_ZONE_ID")
            .map_err(|_| "CLOUDFLARE_ZONE_ID not set".to_string())?;
        let account_id = env::var("CLOUDFLARE_ACCOUNT_ID")
            .map_err(|_| "CLOUDFLARE_ACCOUNT_ID not set".to_string())?;
        let base_domain = env::var("BASE_DOMAIN")
            .unwrap_or_else(|_| "schoolorbit.app".to_string());

        Ok(Self {
            client: Client::new(),
            api_token,
            zone_id,
            account_id,
            base_domain,
        })
    }

    /// Create a DNS record for the subdomain
    /// For Cloudflare Workers, we create a CNAME to workers.dev or an A record to Worker's IP
    pub async fn create_dns_record(
        &self,
        subdomain: &str,
    ) -> Result<String, String> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        let full_domain = format!("{}.{}", subdomain, self.base_domain);

        // Create an A record pointing to Cloudflare's proxy (192.0.2.1 is a placeholder)
        // When proxied=true, Cloudflare handles the routing to the Worker
        let request_body = CreateDnsRecordRequest {
            record_type: "A".to_string(),
            name: full_domain.clone(),
            content: "192.0.2.1".to_string(), // Placeholder IP
            ttl: 1, // Auto TTL
            proxied: true, // Enable Cloudflare proxy
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to create DNS record: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!(
                "DNS record creation failed ({}): {}",
                status, error_text
            ));
        }

        let response_data: CreateDnsRecordResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(response_data.result.id)
    }

    /// Delete a DNS record (for rollback)
    pub async fn delete_dns_record(&self, record_id: &str) -> Result<(), String> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record_id
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| format!("Failed to delete DNS record: {}", e))?;

        if !response.status().is_success() {
            return Err("Failed to delete DNS record".to_string());
        }

        Ok(())
    }


    /// Deploy a Cloudflare Worker via GitHub Actions
    /// Triggers the deploy-school-tenant workflow
    pub async fn deploy_worker(
        &self,
        subdomain: &str,
        school_id: &str,
        api_url: &str,
    ) -> Result<String, String> {
        println!("📦 Triggering GitHub Actions deployment for: {}", subdomain);
        
        // Get GitHub configuration
        let github_token = env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set".to_string())?;
        let github_repo = env::var("GITHUB_REPO")
            .unwrap_or_else(|_| "akephisit/schoolorbit-new".to_string());
        
        // Trigger workflow via GitHub API
        let url = format!(
            "https://api.github.com/repos/{}/actions/workflows/deploy-school-tenant.yml/dispatches",
            github_repo
        );
        
        #[derive(Debug, Serialize)]
        struct WorkflowDispatch {
            #[serde(rename = "ref")]
            git_ref: String,
            inputs: WorkflowInputs,
        }
        
        #[derive(Debug, Serialize)]
        struct WorkflowInputs {
            subdomain: String,
            school_id: String,
            api_url: String,
        }
        
        let dispatch = WorkflowDispatch {
            git_ref: "main".to_string(),
            inputs: WorkflowInputs {
                subdomain: subdomain.to_string(),
                school_id: school_id.to_string(),
                api_url: api_url.to_string(),
            },
        };
        
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", github_token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "SchoolOrbit-Backend")
            .json(&dispatch)
            .send()
            .await
            .map_err(|e| format!("Failed to trigger GitHub Actions: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!(
                "GitHub Actions trigger failed ({}): {}",
                status, error_text
            ));
        }
        
        println!("✅ GitHub Actions workflow triggered successfully");
        println!("   Deployment will be processed by GitHub Actions");
        println!("   Check: https://github.com/{}/actions", github_repo);
        
        // Return the expected URL
        // Note: Actual deployment happens asynchronously in GitHub Actions
        Ok(format!("https://{}.{}", subdomain, self.base_domain))
    }

    /// Delete a Cloudflare Worker
    pub async fn delete_worker(&self, worker_name: &str) -> Result<(), String> {
        println!("🗑️  Deleting Worker: {}", worker_name);
        
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts/{}",
            self.account_id, worker_name
        );
        
        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .map_err(|e| format!("Failed to delete worker: {}", e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            
            // 404 is OK - worker already deleted
            if status == 404 {
                println!("   ℹ️  Worker not found (already deleted)");
                return Ok(());
            }
            
            return Err(format!(
                "Failed to delete worker ({}): {}",
                status, error_text
            ));
        }
        
        println!("   ✅ Worker deleted successfully");
        Ok(())
    }
}
