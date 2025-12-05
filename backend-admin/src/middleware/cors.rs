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
        println!("[CORS] Request: {} {}", req.method, req.path);
        println!("[CORS] Headers: {:?}", req.headers);

        // WORKAROUND: Parse Origin from Debug string because .get() is failing
        let headers_str = format!("{:?}", req.headers);
        let origin = if let Some(start) = headers_str.find("\"Origin\": \"") {
            let rest = &headers_str[start + 11..];
            if let Some(end) = rest.find("\"") {
                let s = rest[..end].to_string();
                println!("[CORS] Origin found (via debug parse): {}", s);
                Some(s)
            } else {
                None
            }
        } else {
            // Try lowercase "origin"
            if let Some(start) = headers_str.find("\"origin\": \"") {
                let rest = &headers_str[start + 11..];
                if let Some(end) = rest.find("\"") {
                    let s = rest[..end].to_string();
                    println!("[CORS] Origin found (via debug parse lowercase): {}", s);
                    Some(s)
                } else {
                    None
                }
            } else {
                println!("[CORS] No Origin header found in debug string");
                None
            }
        };

        let origin = match origin {
            Some(s) => s,
            None => return Ok(()),
        };

        // Check if origin is allowed
        if self.is_origin_allowed(&origin) {
            println!("[CORS] Origin allowed");
            
            // Handle preflight OPTIONS request
            if req.method == Method::OPTIONS {
                println!("[CORS] Handling OPTIONS preflight");
                let mut res = Response::NoContent();
                
                // Set CORS headers directly for OPTIONS
                res.headers.set()
                    .access_control_allow_origin(origin.clone());

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
                
                println!("[CORS] Returning preflight response");
                return Err(res);
            }

            // For other methods, store allowed origin in thread-local for use in back()
            CORS_ORIGIN.with(|cell| {
                *cell.borrow_mut() = Some(origin);
            });
        } else {
            println!("[CORS] Origin NOT allowed: {}", origin);
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
