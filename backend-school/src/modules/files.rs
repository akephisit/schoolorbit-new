pub mod handlers;
pub mod models;
// These platform contracts are consumed by the staged provider and application-service tasks.
#[allow(dead_code)]
pub mod platform_types;
#[allow(dead_code)]
pub mod purpose_registry;
#[allow(dead_code)]
pub mod r2_storage_provider;
pub mod services;
#[allow(dead_code)]
pub mod storage_provider;

#[cfg(test)]
mod schema_tests;
