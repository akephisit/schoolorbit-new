//! Intentional CLI stdout/stderr protocol only. Runtime/startup diagnostics use tracing.
use school_file_platform::namespace_binding::BindingError;
use serde::Serialize;

/// Headless operations emit exactly one safe typed protocol object to stdout.
pub fn json(value: &impl Serialize) -> Result<(), BindingError> {
    println!(
        "{}",
        serde_json::to_string(value).map_err(|_| BindingError::Database)?
    );
    Ok(())
}
/// This closed error enum contains fixed operational codes, never provider payloads.
pub fn diagnostic(error: &BindingError) {
    eprintln!("{error}");
}
pub fn invalid_arguments() {
    eprintln!("unsupported_operational_arguments");
}
pub fn legacy_inventory(
    tenants: usize,
    checked: usize,
    failed: usize,
    watches: i64,
    unresolved: i64,
    mismatched: i64,
) {
    println!("storage_namespace_inventory tenants={} checked={} failed={} watches={} unresolved={} mismatched={}",tenants,checked,failed,watches,unresolved,mismatched);
}
pub fn legacy_manifest(watches: usize) {
    println!("storage_namespace_manifest watches={}", watches);
}
pub fn legacy_binding(selected: usize, applied: usize) {
    println!(
        "storage_namespace_binding selected={} applied={}",
        selected, applied
    );
}
