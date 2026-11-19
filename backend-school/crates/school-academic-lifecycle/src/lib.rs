pub mod models;
pub mod ports;
pub mod services;

mod permission_codes_generated;

#[cfg(feature = "integration-test-support")]
mod integration_test_permission_codes {
    include!("test_support/integration/permission_codes_generated.rs");
}

#[cfg(test)]
mod test_permission_codes {
    include!("test_support/permission_codes_generated.rs");
}
