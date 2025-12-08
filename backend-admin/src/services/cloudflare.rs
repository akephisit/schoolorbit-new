use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
pub struct CreateWorkerRequest {
    pub name: String,
    pub script: String,
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Serialize)]
pub struct Binding {
    #[serde(rename = "type")]
    pub binding_type: String,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudflareResponse<T> {
    pub success: bool,
    pub errors: Vec<String>,
    pub result: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct WorkerResponse {
    pub id: String,
}

pub struct CloudflareClient {
    api_token: String,
    account_id: String,
    client: reqwest::Client,
}

impl CloudflareClient {
    pub fn new() -> Result<Self, String> {
        let api_token = env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| "CLOUDFLARE_API_TOKEN not set".to_string())?;
        let account_id = env::var("CLOUDFLARE_ACCOUNT_ID")
            .map_err(|_| "CLOUDFLARE_ACCOUNT_ID not set".to_string())?;

        Ok(Self {
            api_token,
            account_id,
            client: reqwest::Client::new(),
        })
    }

    /// Deploy a Worker script
    pub async fn deploy_worker(
        &self,
        name: &str,
        script_content: &str,
    ) -> Result<String, String> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts/{}",
            self.account_id, name
        );

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/javascript")
            .body(script_content.to_string())
            .send()
            .await
            .map_err(|e| format!("Failed to deploy worker: {}", e))?;

        if response.status().is_success() {
            Ok(format!("Worker {} deployed successfully", name))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Cloudflare API error: {}", error_text))
        }
    }

    /// Create a custom domain route
    pub async fn create_route(
        &self,
        zone_id: &str,
        pattern: &str,
        script_name: &str,
    ) -> Result<String, String> {
        #[derive(Serialize)]
        struct RouteRequest {
            pattern: String,
            script: String,
        }

        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/workers/routes",
            zone_id
        );

        let request_body = RouteRequest {
            pattern: pattern.to_string(),
            script: script_name.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to create route: {}", e))?;

        if response.status().is_success() {
            Ok(format!("Route created for {}", pattern))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Cloudflare API error: {}", error_text))
        }
    }

    /// Create DNS record for subdomain
    pub async fn create_dns_record(
        &self,
        zone_id: &str,
        subdomain: &str,
    ) -> Result<String, String> {
        #[derive(Serialize)]
        struct DnsRecord {
            #[serde(rename = "type")]
            record_type: String,
            name: String,
            content: String,
            ttl: u32,
            proxied: bool,
        }

        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            zone_id
        );

        let record = DnsRecord {
            record_type: "AAAA".to_string(),
            name: subdomain.to_string(),
            content: "100::".to_string(), // Placeholder for Workers
            ttl: 1, // Auto
            proxied: true,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&record)
            .send()
            .await
            .map_err(|e| format!("Failed to create DNS record: {}", e))?;

        if response.status().is_success() {
            Ok(format!("DNS record created for {}", subdomain))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Cloudflare API error: {}", error_text))
        }
    }
}
