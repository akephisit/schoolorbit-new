use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub success: bool,
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationMeta {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: i64, limit: i64, total: i64) -> Self {
        let total_pages = (total as f64 / limit as f64).ceil() as i64;
        Self {
            success: true,
            data,
            meta: PaginationMeta {
                page,
                limit,
                total,
                total_pages,
            },
        }
    }
}
