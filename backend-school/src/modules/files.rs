pub mod consumer_service;
pub mod file_inspector;
pub mod handlers;
pub mod malware_scanner;
pub mod models;
pub mod platform_service;
pub mod platform_types;
pub mod purpose_registry;
pub mod r2_storage_provider;
pub mod reconciler;
pub mod repository;
pub mod runtime_config;
pub mod storage_provider;

#[cfg(test)]
mod schema_tests;
