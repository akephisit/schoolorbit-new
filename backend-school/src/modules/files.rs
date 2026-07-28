pub mod handlers;
pub mod models;
// These platform contracts are consumed by the staged provider and application-service tasks.
#[allow(dead_code)]
pub mod platform_types;
#[allow(dead_code)]
pub mod purpose_registry;
pub mod services;

#[cfg(test)]
mod schema_tests;
