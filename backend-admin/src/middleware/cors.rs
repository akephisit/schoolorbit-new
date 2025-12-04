use ohkami::prelude::*;
use std::collections::HashSet;
use std::cell::RefCell;

thread_local! {
    static CORS_ORIGIN: RefCell<Option<String>> = RefCell::new(None);
}

/// Custom CORS middleware that supports multiple origins
#[derive(Clone)]
pub struct MultiCors {
    allowed_origins: HashSet<String>,
    allow_credentials: bool,
    allow_headers: Vec<String>,
    max_age: Option<u32>,
}

impl MultiCors {
    /// Create a new MultiCors middleware with allowed origins
    pub fn new(origins: Vec<String>) -> Self {
        Self {
            allowed_origins: origins.into_iter().collect(),
            allow_credentials: false,
            allow_headers: vec![
                "Content-Type".to_string(),
                "Authorization".to_string(),
            ],
            max_age: Some(3600),
        }
    }

    /// Parse origins from comma-separated string
    pub fn from_env_string(origins_str: &str) -> Self {
        let origins: Vec<String> = origins_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        Self::new(origins)
    }

    /// Allow credentials (cookies, authorization headers, etc.)
    pub fn allow_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }

    /// Set allowed headers
    pub fn allow_headers<I, S>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allow_headers = headers.into_iter().map(|h| h.into()).collect();
        self
    }

    /// Set max age for preflight cache
    pub fn max_age(mut self, seconds: Option<u32>) -> Self {
        self.max_age = seconds;
        self
    }

    /// Check if origin is allowed
    fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.contains(origin) || 
        self.allowed_origins.contains("*")
    }
}

impl FangAction for MultiCors {
    async fn fore<'a>(&'a self, req: &'a mut Request) -> Result<(), Response> {
        // Get the Origin header from the request
        if let Some(origin_header) = req.headers.get("Origin") {
            let origin = origin_header.to_string();
            
            // Check if origin is allowed
            if self.is_origin_allowed(&origin) {
                // Store allowed origin in thread-local for use in back()
                CORS_ORIGIN.with(|cell| {
                    *cell.borrow_mut() = Some(origin);
                });
            }
        }

        Ok(())
    }

    async fn back<'a>(&'a self, res: &'a mut Response) {
        // Get the allowed origin from thread-local storage
        let origin = CORS_ORIGIN.with(|cell| cell.borrow().clone());
        
        if let Some(origin_value) = origin {
            // Set CORS headers using Ohkami's API
            res.headers.set()
                .access_control_allow_origin(origin_value);

            if self.allow_credentials {
                res.headers.set()
                    .access_control_allow_credentials("true");
            }

            res.headers.set()
                .access_control_allow_headers(self.allow_headers.join(", "));

            res.headers.set()
                .access_control_allow_methods("GET, POST, PUT, PATCH, DELETE, OPTIONS");

            if let Some(max_age) = self.max_age {
                res.headers.set()
                    .access_control_max_age(max_age.to_string());
            }
            
            // Clear thread-local after use
            CORS_ORIGIN.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}
