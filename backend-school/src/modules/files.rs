// These staged contracts are consumed by the FilePlatform application service in Task 6.
#[allow(dead_code)]
pub mod file_inspector;
pub mod handlers;
// The scanner adapter is constructed with runtime configuration in Task 6.
#[allow(dead_code)]
pub mod malware_scanner;
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
