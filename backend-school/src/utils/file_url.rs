use std::env;

/// File URL Builder - Converts storage paths to full URLs
/// 
/// This utility handles path → URL conversion using configured base URLs.
/// It supports CDN failover and environment-specific URLs.
#[derive(Debug, Clone)]
pub struct FileUrlBuilder {
    base_url: String,
    cdn_url: Option<String>,
}

impl FileUrlBuilder {
    /// Create a new FileUrlBuilder from environment variables
    pub fn new() -> Result<Self, String> {
        let env_val = env::var("R2_PUBLIC_URL").unwrap_or_default();
        
        // Use hardcoded fallback if env is missing or empty
        let base_url = if env_val.is_empty() {
             tracing::warn!("⚠️ R2_PUBLIC_URL is missing! Using default: https://files.schoolorbit.app");
             "https://files.schoolorbit.app".to_string() 
        } else {
             env_val
        };
        
        // Filter out empty string for CDN_URL
        let cdn_url = env::var("CDN_URL")
            .ok()
            .filter(|s| !s.is_empty()); // Important: Ignore empty string
            
        Ok(Self { base_url, cdn_url })
    }
    
    /// Build URL from storage path
    /// 
    /// # Arguments
    /// * `path` - Storage path (e.g., "school-abc/users/profiles/uuid.jpg")
    /// 
    /// # Returns
    /// Full URL (e.g., "https://cdn.schoolorbit.app/school-abc/users/profiles/uuid.jpg")
    pub fn build_url(&self, path: &str) -> String {
        // If path is already a full URL, return it as is
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }

        let base = self.cdn_url.as_ref().unwrap_or(&self.base_url);
        format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
    }
    
    /// Build URL from optional storage path
    pub fn build_url_option(&self, path: Option<&str>) -> Option<String> {
        path.map(|p| self.build_url(p))
    }
    
    /// Get the effective base URL (CDN if available, otherwise R2)
    pub fn base_url(&self) -> &str {
        self.cdn_url.as_deref().unwrap_or(&self.base_url)
    }
}

impl Default for FileUrlBuilder {
    fn default() -> Self {
        Self::new().expect("Failed to create FileUrlBuilder from environment")
    }
}

/// Helper function to quickly convert path to URL
/// 
/// # Example
/// ```
/// let url = get_file_url(Some("school-abc/users/profiles/uuid.jpg"));
/// assert_eq!(url, Some("https://cdn.schoolorbit.app/school-abc/users/profiles/uuid.jpg".to_string()));
/// ```
pub fn get_file_url(path: Option<&str>) -> Option<String> {
    path.and_then(|p| {
        FileUrlBuilder::new()
            .ok()
            .map(|builder| builder.build_url(p))
    })
}

/// Helper function to convert path reference to URL
pub fn get_file_url_from_string(path: &Option<String>) -> Option<String> {
    get_file_url(path.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_url() {
        std::env::set_var("R2_PUBLIC_URL", "https://pub-test.r2.dev");
        
        let builder = FileUrlBuilder::new().unwrap();
        let url = builder.build_url("school-abc/users/profiles/test.jpg");
        
        assert_eq!(url, "https://pub-test.r2.dev/school-abc/users/profiles/test.jpg");
    }
    
    #[test]
    fn test_build_url_with_cdn() {
        std::env::set_var("R2_PUBLIC_URL", "https://pub-test.r2.dev");
        std::env::set_var("CDN_URL", "https://cdn.schoolorbit.app");
        
        let builder = FileUrlBuilder::new().unwrap();
        let url = builder.build_url("school-abc/users/profiles/test.jpg");
        
        assert_eq!(url, "https://cdn.schoolorbit.app/school-abc/users/profiles/test.jpg");
        
        std::env::remove_var("CDN_URL");
    }
    
    #[test]
    fn test_build_url_option() {
        std::env::set_var("R2_PUBLIC_URL", "https://pub-test.r2.dev");
        
        let builder = FileUrlBuilder::new().unwrap();
        
        assert_eq!(
            builder.build_url_option(Some("test.jpg")),
            Some("https://pub-test.r2.dev/test.jpg".to_string())
        );
        
        assert_eq!(builder.build_url_option(None), None);
    }
}
