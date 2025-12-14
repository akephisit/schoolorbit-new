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
struct CreateDnsRecordResponse {
    result: DnsRecord,  // Only field we use
}

#[derive(Debug, Deserialize)]
struct DnsRecord {
    id: String,  // Only field we use
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
    /// Returns (deployment_url, trigger_timestamp)
    pub async fn deploy_worker(
        &self,
        subdomain: &str,
        school_id: &str,
        api_url: &str,
    ) -> Result<(String, chrono::DateTime<chrono::Utc>), String> {
        println!("📦 Triggering GitHub Actions deployment for: {}", subdomain);
        
        // Record the time before triggering (to account for any delays)
        let trigger_time = chrono::Utc::now();
        
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
        println!("   Triggered at: {}", trigger_time);
        println!("   Deployment will be processed by GitHub Actions");
        println!("   Check: https://github.com/{}/actions", github_repo);
        
        // Return the expected URL and trigger time
        // Note: Actual deployment happens asynchronously in GitHub Actions
        let deployment_url = format!("https://{}.{}", subdomain, self.base_domain);
        Ok((deployment_url, trigger_time))
    }

    /// Wait for GitHub Actions workflow to complete
    /// Returns Ok(()) if successful, Err() if failed or timeout
    pub async fn wait_for_workflow_completion(
        &self,
        subdomain: &str,
        trigger_time: chrono::DateTime<chrono::Utc>,
        timeout_minutes: u64,
    ) -> Result<(), String> {
        let github_token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| "GITHUB_TOKEN not set".to_string())?;
        let github_repo = std::env::var("GITHUB_REPOSITORY")
            .unwrap_or_else(|_| "akephisit/schoolorbit-new".to_string());

        let url = format!(
            "https://api.github.com/repos/{}/actions/runs",
            github_repo
        );

        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_minutes * 60);
        let poll_interval = std::time::Duration::from_secs(10); // Poll every 10 seconds

        println!("⏳ Waiting for GitHub Actions workflow to complete...");
        println!("   Subdomain: {}", subdomain);
        println!("   Triggered at: {}", trigger_time);
        println!("   Timeout: {} minutes", timeout_minutes);

        loop {
            // Check timeout
            if start_time.elapsed() > timeout {
                return Err(format!(
                    "Workflow timeout after {} minutes",
                    timeout_minutes
                ));
            }

            // Fetch recent workflow runs
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", github_token))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", "SchoolOrbit-Backend")
                .query(&[("per_page", "20"), ("event", "workflow_dispatch")])
                .send()
                .await
                .map_err(|e| format!("Failed to fetch workflow runs: {}", e))?;

            if !response.status().is_success() {
                return Err(format!(
                    "GitHub API error: {}",
                    response.status()
                ));
            }

            let runs: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))?;

            // Find the most recent run for deployment workflow that was created after our trigger
            if let Some(workflow_runs) = runs["workflow_runs"].as_array() {
                if workflow_runs.is_empty() {
                    println!("   No workflow runs found yet - waiting...");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }

                // Search for matching workflow run
                let mut found_matching_run = false;
                for run in workflow_runs {
                    let name = run["name"].as_str().unwrap_or("");
                    let created_at_str = run["created_at"].as_str().unwrap_or("");
                    
                    // Parse created_at timestamp
                    let created_at = match chrono::DateTime::parse_from_rfc3339(created_at_str) {
                        Ok(dt) => dt.with_timezone(&chrono::Utc),
                        Err(_) => {
                            println!("   Warning: Could not parse created_at: {}", created_at_str);
                            continue;
                        }
                    };

                    // Check if this workflow was created after we triggered
                    // Allow 5 seconds buffer for clock differences
                    let trigger_with_buffer = trigger_time - chrono::Duration::seconds(5);
                    if created_at < trigger_with_buffer {
                        println!("   Skipping old workflow: {} (created before trigger)", name);
                        continue;
                    }

                    // Check if this is deployment workflow
                    if !name.contains("Deploy") || !name.contains("School") {
                        println!("   Skipping non-deployment workflow: {}", name);
                        continue;
                    }

                    // Found a matching workflow!
                    found_matching_run = true;
                    let status = run["status"].as_str().unwrap_or("");
                    let conclusion = run["conclusion"].as_str();
                    let html_url = run["html_url"].as_str().unwrap_or("");

                    println!("   Found matching workflow: {} - status: {}", name, status);
                    println!("   Created at: {} (trigger: {})", created_at, trigger_time);

                    match status {
                        "completed" => {
                            match conclusion {
                                Some("success") => {
                                    println!("✅ Workflow completed successfully!");
                                    println!("   URL: {}", html_url);
                                    return Ok(());
                                }
                                Some("failure") | Some("cancelled") => {
                                    return Err(format!(
                                        "Workflow {} - Check: {}",
                                        conclusion.unwrap_or("failed"),
                                        html_url
                                    ));
                                }
                                _ => {
                                    println!("   Workflow completed with unknown conclusion: {:?}", conclusion);
                                }
                            }
                        }
                        "in_progress" | "queued" | "waiting" => {
                            println!("   Workflow {} - continuing to wait...", status);
                            // Don't break - continue waiting
                            break; // Break inner loop to wait and poll again
                        }
                        _ => {
                            println!("   Unknown status: {}", status);
                        }
                    }

                    // If we found a matching run, don't check older ones
                    break;
                }

                if !found_matching_run {
                    println!("   No matching workflow run found yet (checked {} runs)", workflow_runs.len());
                }
            } else {
                println!("   No workflow_runs array in response");
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }
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
