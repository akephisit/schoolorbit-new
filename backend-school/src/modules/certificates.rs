pub mod handlers;
// Staged domain contracts are consumed by certificate handlers in Task 4 onward.
#[allow(dead_code)]
pub mod models;
// Pure helpers are integrated by the following certificate service slices.
#[allow(dead_code)]
pub mod services;

#[cfg(test)]
mod schema_tests;
#[cfg(test)]
mod services_tests;
