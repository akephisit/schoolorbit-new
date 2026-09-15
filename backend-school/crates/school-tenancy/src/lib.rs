mod admin_client;
mod pool_manager;
mod school_mapping;

use sqlx::PgPool;
use uuid::Uuid;

pub use admin_client::{ActiveSchool, AdminClient, AdminClientConfig, SchoolDatabaseInfo};
pub use pool_manager::PoolManager;
pub use school_mapping::get_school_database_info;

#[derive(Clone)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub subdomain: String,
    pub pool: PgPool,
}
