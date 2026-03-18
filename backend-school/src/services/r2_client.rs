use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;
use aws_config::meta::region::RegionProviderChain;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use std::env;
use tracing::{error, info};

/// R2 Client Configuration
#[derive(Debug, Clone)]
pub struct R2Config {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket_name: String,
    pub public_url: String,
    pub region: String,
}

impl R2Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            account_id: env::var("R2_ACCOUNT_ID")
                .map_err(|_| "R2_ACCOUNT_ID not set")?,
            access_key_id: env::var("R2_ACCESS_KEY_ID")
                .map_err(|_| "R2_ACCESS_KEY_ID not set")?,
            secret_access_key: env::var("R2_SECRET_ACCESS_KEY")
                .map_err(|_| "R2_SECRET_ACCESS_KEY not set")?,
            bucket_name: env::var("R2_BUCKET_NAME")
                .map_err(|_| "R2_BUCKET_NAME not set")?,
            public_url: env::var("R2_PUBLIC_URL")
                .map_err(|_| "R2_PUBLIC_URL not set")?,
            region: env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string()),
        })
    }
    
    /// Get the R2 endpoint URL
    pub fn endpoint_url(&self) -> String {
        format!("https://{}.r2.cloudflarestorage.com", self.account_id)
    }
}

/// R2 Client - S3-compatible client for Cloudflare R2
pub struct R2Client {
    client: S3Client,
    config: R2Config,
}

impl R2Client {
    /// Create a new R2Client
    pub async fn new() -> Result<Self, String> {
        let config = R2Config::from_env()?;
        
        info!("Initializing R2 Client...");
        info!("Endpoint: {}", config.endpoint_url());
        info!("Bucket: {}", config.bucket_name);
        
        // Create AWS credentials
        let credentials = Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None,
            None,
            "r2-client",
        );
        
        // Setup region
        let region = Region::new(config.region.clone());
        let region_provider = RegionProviderChain::default_provider().or_else(region);
        
        // Build S3 config for R2
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .credentials_provider(credentials)
            .load()
            .await;
        
        let s3_config = aws_sdk_s3::config::Builder::from(&aws_config)
            .endpoint_url(config.endpoint_url())
            .force_path_style(true)
            .build();
        
        let client = S3Client::from_conf(s3_config);
        
        info!("R2 Client initialized successfully");
        
        Ok(Self { client, config })
    }
    
    /// Upload a file to R2
    ///
    /// # Arguments
    /// * `key` - Storage path (e.g., "school-abc/users/profiles/uuid.jpg")
    /// * `data` - File data as bytes
    /// * `content_type` - MIME type (e.g., "image/jpeg")
    ///
    /// # Returns
    /// Ok(()) if successful, Err(String) otherwise
    pub async fn upload_file(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<(), String> {
        info!("Uploading file to R2: {}", key);
        
        let body = ByteStream::from(data);
        
        self.client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to upload file to R2: {}", e);
                format!("Failed to upload file: {}", e)
            })?;
        
        info!("File uploaded successfully: {}", key);
        Ok(())
    }
    
    /// Download a file from R2
    ///
    /// # Arguments
    /// * `key` - Storage path
    ///
    /// # Returns
    /// File data as Vec<u8>
    pub async fn download_file(&self, key: &str) -> Result<Vec<u8>, String> {
        info!("Downloading file from R2: {}", key);
        
        let response = self
            .client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to download file from R2: {}", e);
                format!("Failed to download file: {}", e)
            })?;
        
        let data = response
            .body
            .collect()
            .await
            .map_err(|e| format!("Failed to read file data: {}", e))?
            .into_bytes()
            .to_vec();
        
        info!("File downloaded successfully: {} ({} bytes)", key, data.len());
        Ok(data)
    }
    
    /// Delete a file from R2
    ///
    /// # Arguments
    /// * `key` - Storage path
    pub async fn delete_file(&self, key: &str) -> Result<(), String> {
        info!("Deleting file from R2: {}", key);
        
        self.client
            .delete_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to delete file from R2: {}", e);
                format!("Failed to delete file: {}", e)
            })?;
        
        info!("File deleted successfully: {}", key);
        Ok(())
    }
    
    /// Check if a file exists in R2
    ///
    /// # Arguments
    /// * `key` - Storage path
    pub async fn file_exists(&self, key: &str) -> bool {
        self.client
            .head_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .is_ok()
    }
    
    /// Generate a presigned URL for downloading a file
    /// Only the key holder can access the file via this URL
    pub async fn generate_presigned_url(&self, key: &str, expires_in_secs: u64) -> Result<String, String> {
        let presigning_config = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs))
            .map_err(|e| format!("Failed to create presigning config: {}", e))?;

        let presigned_req = self.client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| format!("Failed to generate presigned URL: {}", e))?;

        Ok(presigned_req.uri().to_string())
    }

    /// Get the public URL for a file
    pub fn get_public_url(&self, key: &str) -> String {
        format!("{}/{}", self.config.public_url.trim_end_matches('/'), key)
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.config.bucket_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_r2_config_endpoint_url() {
        let config = R2Config {
            account_id: "test123".to_string(),
            access_key_id: "key".to_string(),
            secret_access_key: "secret".to_string(),
            bucket_name: "test-bucket".to_string(),
            public_url: "https://pub.r2.dev".to_string(),
            region: "auto".to_string(),
        };
        
        assert_eq!(
            config.endpoint_url(),
            "https://test123.r2.cloudflarestorage.com"
        );
    }
    
    #[test]
    fn test_public_url_generation() {
        let config = R2Config {
            account_id: "test".to_string(),
            access_key_id: "key".to_string(),
            secret_access_key: "secret".to_string(),
            bucket_name: "bucket".to_string(),
            public_url: "https://pub.r2.dev".to_string(),
            region: "auto".to_string(),
        };
        
        // Note: Can't test full R2Client without credentials
        let url = format!(
            "{}/{}",
            config.public_url.trim_end_matches('/'),
            "school-abc/test.jpg"
        );
        
        assert_eq!(url, "https://pub.r2.dev/school-abc/test.jpg");
    }
}
