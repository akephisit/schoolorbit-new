use ohkami::prelude::*;
use crate::services::SchoolService;
use crate::models::{CreateSchool, UpdateSchool};
use shared::types::ApiResponse;
use shared::error::AppError;
use std::sync::Arc;

pub struct SchoolHandler {
    service: Arc<SchoolService>,
}

impl SchoolHandler {
    pub fn new(service: Arc<SchoolService>) -> Self {
        Self { service }
    }

    pub async fn create(&self, body: String) -> Result<String, String> {
        let data: CreateSchool = serde_json::from_str(&body)
            .map_err(|e| format!("Invalid request body: {}", e))?;

        match self.service.create_school(data).await {
            Ok(school) => {
                let response = ApiResponse::success(school);
                Ok(serde_json::to_string(&response).unwrap())
            }
            Err(e) => Err(format!("{}", e)),
        }
    }

    pub async fn list(&self, query: String) -> Result<String, String> {
        // Parse query params (simplified)
        let page = 1i64;
        let limit = 20i64;

        match self.service.list_schools(page, limit).await {
            Ok((schools, total)) => {
                let response = shared::types::PaginatedResponse::new(schools, page, limit, total);
                Ok(serde_json::to_string(&response).unwrap())
            }
            Err(e) => Err(format!("{}", e)),
        }
    }
}
