pub mod academic_template_service;
pub mod feature_toggle_service;
pub mod menu_service;
pub mod public_menu_service;
pub mod route_registration_service;

// SQLx messages can contain query or bind data. Keep only the owned operation context
// and the fixed category when this reason reaches the HTTP error logger.
fn database_failure(context: &'static str, error: sqlx::Error) -> school_errors::AppError {
    school_errors::AppError::InternalServerError(format!(
        "{context}: {}",
        school_database::error_class(&error)
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn database_failure_reason_excludes_native_payload() {
        let error = super::database_failure(
            "Failed to update menu workspace",
            sqlx::Error::Protocol("sensitive SQL/bind sentinel".to_string()),
        );
        let school_errors::AppError::InternalServerError(reason) = error else {
            panic!("navigation keeps its internal-server-error policy");
        };
        assert_eq!(reason, "Failed to update menu workspace: protocol");
        assert!(!reason.contains("sentinel"));
    }
}
