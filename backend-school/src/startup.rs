//! Application startup failures contain only sanitized operational context.
use std::{env, io};
use tokio_cron_scheduler::JobSchedulerError;

#[derive(Debug)]
pub(crate) struct StartupError {
    stage: &'static str,
    reason: String,
}

impl StartupError {
    pub fn stage(&self) -> &'static str {
        self.stage
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub fn new(stage: &'static str, reason: &'static str) -> Self {
        Self {
            stage,
            reason: reason.into(),
        }
    }

    pub fn tenancy_configuration(stage: &'static str, message: &str) -> Self {
        for constraint in [
            "Aggregate connection allocations exceed POSTGRES_MAX_CONNECTIONS",
            "Tenant pool maximum exceeds aggregate runtime budget",
        ] {
            if message == constraint {
                return Self::new(stage, constraint);
            }
        }
        // Reconstruct the approved provider diagnostic from fixed names and
        // categories; arbitrary error contents never enter startup output.
        for name in [
            "TENANT_RUNTIME_CONNECTIONS",
            "TENANT_POOL_MAX_CONNECTIONS",
            "TENANT_POOL_ADMISSION_TIMEOUT_MS",
            "POSTGRES_MAX_CONNECTIONS",
            "ADMIN_POOL_MAX_CONNECTIONS",
            "PG_PROVISIONING_MAX_CONNECTIONS",
            "SCHOOL_PRIVILEGED_CONNECTIONS",
            "POSTGRES_CONNECTION_HEADROOM",
            "TENANT_DIRECTORY_MAX_IN_FLIGHT",
            "TENANT_DIRECTORY_ADMISSION_TIMEOUT_MS",
            "TENANT_DIRECTORY_CAPACITY",
            "TENANT_DIRECTORY_TTL_SECONDS",
            "BACKEND_ADMIN_REQUEST_TIMEOUT_MS",
            "BACKEND_ADMIN_RETRY_MAX_ATTEMPTS",
            "BACKEND_ADMIN_RETRY_BASE_DELAY_MS",
            "TENANT_BACKGROUND_MAX_TURNS",
        ] {
            for category in [
                "must be an integer",
                "must be a valid integer",
                "is outside the supported range",
            ] {
                let reason = format!("{name} {category}");
                if message == reason {
                    return Self { stage, reason };
                }
            }
        }
        Self::new(stage, "configuration_validation_failed")
    }

    fn environment(name: &'static str, error: env::VarError) -> Self {
        let category = match error {
            env::VarError::NotPresent => "missing",
            env::VarError::NotUnicode(_) => "invalid_unicode",
        };
        Self {
            stage: "environment",
            reason: format!("{name}: {category}"),
        }
    }

    pub fn io(stage: &'static str, error: io::Error) -> Self {
        Self {
            stage,
            reason: format!("io_{:?}", error.kind()),
        }
    }

    pub fn scheduler(stage: &'static str, error: JobSchedulerError) -> Self {
        let reason = match error {
            JobSchedulerError::ParseSchedule => "invalid_cron_schedule",
            JobSchedulerError::CantAdd => "registration_failed",
            JobSchedulerError::CantInit => "initialization_failed",
            JobSchedulerError::StartScheduler => "start_failed",
            JobSchedulerError::FetchJob | JobSchedulerError::ErrorLoadingJob => "job_load_failed",
            JobSchedulerError::SaveJob | JobSchedulerError::UpdateJobData => "job_save_failed",
            JobSchedulerError::GetJobData | JobSchedulerError::GetJobStore => {
                "job_store_unavailable"
            }
            JobSchedulerError::JobTypeNotSet
            | JobSchedulerError::RunOrRunAsyncNotSet
            | JobSchedulerError::ScheduleNotSet => "job_configuration_incomplete",
            JobSchedulerError::CantGetTimeUntil
            | JobSchedulerError::CouldNotGetTimeUntilNextTick
            | JobSchedulerError::NoNextTick => "next_run_unavailable",
            _ => "scheduler_operation_failed",
        };
        Self::new(stage, reason)
    }
}

pub(crate) fn required_environment(name: &'static str) -> Result<String, StartupError> {
    env::var(name).map_err(|error| StartupError::environment(name, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_diagnostics_include_stage_and_category_without_raw_error_contents() {
        let secret = "synthetic-private-marker";
        let environment = StartupError::environment(
            "INTERNAL_API_SECRET",
            env::VarError::NotUnicode(secret.into()),
        );
        let diagnostic = format!("{environment:?}");
        assert!(
            diagnostic.contains("INTERNAL_API_SECRET") && diagnostic.contains("invalid_unicode")
        );
        assert!(!diagnostic.contains(secret));
        let bind = StartupError::io(
            "listener_bind",
            io::Error::new(io::ErrorKind::AddrInUse, secret),
        );
        let diagnostic = format!("{bind:?}");
        assert!(diagnostic.contains("listener_bind") && diagnostic.contains("AddrInUse"));
        assert!(!diagnostic.contains(secret));
        let scheduler =
            StartupError::scheduler("calendar_registration", JobSchedulerError::CantAdd);
        assert!(format!("{scheduler:?}").contains("registration_failed"));
    }
    #[test]
    fn tenancy_configuration_diagnostics_keep_known_constraints_and_reject_untrusted_details() {
        for message in [
            "TENANT_POOL_MAX_CONNECTIONS must be an integer",
            "BACKEND_ADMIN_REQUEST_TIMEOUT_MS is outside the supported range",
            "TENANT_BACKGROUND_MAX_TURNS must be an integer",
            "Aggregate connection allocations exceed POSTGRES_MAX_CONNECTIONS",
            "Tenant pool maximum exceeds aggregate runtime budget",
        ] {
            assert_eq!(
                StartupError::tenancy_configuration("configuration", message).reason(),
                message
            );
        }
        let error = StartupError::tenancy_configuration(
            "configuration",
            "TENANT_POOL_MAX_CONNECTIONS must be an integer: synthetic-private-marker",
        );
        assert_eq!(error.reason(), "configuration_validation_failed");
        assert!(!format!("{error:?}").contains("synthetic-private-marker"));
    }
}
