use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

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
struct CreateDnsRecordResponse {
    result: DnsRecord,
    success: bool,
}

#[derive(Debug, Deserialize)]
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

    /// Deploy a Cloudflare Worker using wrangler CLI
    /// This executes the deployment script
    pub async fn deploy_worker(
        &self,
        subdomain: &str,
        school_id: &str,
        api_url: &str,
    ) -> Result<String, String> {
        let script_path = env::var("DEPLOY_SCRIPT_PATH")
            .unwrap_or_else(|_| "./scripts/deploy_tenant.sh".to_string());

        println!("📦 Deploying Worker for subdomain: {}", subdomain);

        // Execute the deployment script
        let output = Command::new("bash")
            .arg(&script_path)
            .arg(subdomain)
            .arg(school_id)
            .arg(api_url)
            .output()
            .map_err(|e| format!("Failed to execute deployment script: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Worker deployment failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("✅ Worker deployed: {}", stdout);

        Ok(format!("https://{}.{}", subdomain, self.base_domain))
    }
}
