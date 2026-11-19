//! Academic Core domain models, invariants, and persistence services.

pub mod context_reconciliation;
pub mod models;
pub mod policies;
pub mod ports;
pub mod reconciliation;
pub mod services;

mod permission_codes_generated;

#[cfg(test)]
#[path = "test_support/permission_codes_generated.rs"]
mod test_permission_codes;
