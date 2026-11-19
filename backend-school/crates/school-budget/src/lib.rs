//! Budget owns accepted plans, reservations and the append-only financial ledger.
pub mod adjustments;
pub mod deletion;
pub mod files;
pub mod http;
pub mod models;
pub mod money;
pub mod policy;
pub mod ports;
mod process;
pub mod projects;
pub mod queries;
mod repository;
pub mod requests;
pub mod settings;

mod permission_codes_generated;

#[cfg(test)]
mod test_permission_codes {
    include!("test_support/permission_codes_generated.rs");
}
