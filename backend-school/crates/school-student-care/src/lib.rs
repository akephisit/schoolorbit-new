//! Confidential student support, independently owned from Behavior and academics.
pub mod http;
pub mod models;
pub mod policy;
pub mod scoring;
pub mod services;

mod permission_codes_generated;

#[cfg(test)]
mod test_permission_codes {
    include!("test_support/permission_codes_generated.rs");
}
