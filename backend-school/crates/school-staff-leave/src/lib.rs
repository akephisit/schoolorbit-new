pub mod files;
pub mod models;
pub mod policy;
pub mod service;

mod configuration;
mod lifecycle;
mod queries;
mod repository;

#[cfg(test)]
mod full_day_tests;
#[cfg(test)]
mod pool_facade_tests;

pub mod coverage;
pub mod coverage_models;

mod permission_codes_generated;

#[cfg(test)]
#[path = "test_support/permission_codes_generated.rs"]
mod test_permission_codes;
