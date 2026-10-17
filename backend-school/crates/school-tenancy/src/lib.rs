mod admin_client;
pub mod background;
mod directory_cache;
mod pool_manager;
mod school_mapping;
mod tenant_pool;

use uuid::Uuid;

pub use admin_client::{ActiveSchool, AdminClient, AdminClientConfig, SchoolDatabaseInfo};
pub use pool_manager::{
    close_pool, connect_privileged_connection, connect_privileged_pool, PoolManager,
    PoolManagerConfig, PrivilegedConnection, PrivilegedPool,
};
pub use school_mapping::get_school_database_info;
pub use tenant_pool::{TenantPool, TenantPoolRetention};

#[derive(Clone)]
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub subdomain: String,
    pub pool: TenantPool,
}
