use std::{env, fmt, time::Duration};

const DEFAULT_PRIVATE_GRANT_TTL_SECONDS: u64 = 120;
const DEFAULT_RECONCILE_LEASE_SECONDS: u64 = 60;
const DEFAULT_RECONCILE_BATCH_SIZE: i64 = 25;
const DEFAULT_RECONCILE_MAX_ATTEMPTS: i32 = 8;
const DEFAULT_RETRY_BASE_SECONDS: u64 = 5;
const DEFAULT_RETRY_MAX_SECONDS: u64 = 3_600;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilePlatformRuntimeConfig {
    pub private_download_grant_ttl: Duration,
    pub reconciliation_lease: Duration,
    pub reconciliation_batch_size: i64,
    pub max_operation_attempts: i32,
    pub retry_base: Duration,
    pub retry_max: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeConfigError {
    InvalidValue,
}

impl RuntimeConfigError {
    pub const fn log_safe_code(self) -> &'static str {
        "file_runtime_configuration_invalid"
    }
}

impl fmt::Display for RuntimeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.log_safe_code())
    }
}

impl std::error::Error for RuntimeConfigError {}

impl Default for FilePlatformRuntimeConfig {
    fn default() -> Self {
        Self {
            private_download_grant_ttl: Duration::from_secs(DEFAULT_PRIVATE_GRANT_TTL_SECONDS),
            reconciliation_lease: Duration::from_secs(DEFAULT_RECONCILE_LEASE_SECONDS),
            reconciliation_batch_size: DEFAULT_RECONCILE_BATCH_SIZE,
            max_operation_attempts: DEFAULT_RECONCILE_MAX_ATTEMPTS,
            retry_base: Duration::from_secs(DEFAULT_RETRY_BASE_SECONDS),
            retry_max: Duration::from_secs(DEFAULT_RETRY_MAX_SECONDS),
        }
    }
}

impl FilePlatformRuntimeConfig {
    pub fn from_env() -> Result<Self, RuntimeConfigError> {
        let values = [
            env_value(
                "FILE_PRIVATE_GRANT_TTL_SECONDS",
                DEFAULT_PRIVATE_GRANT_TTL_SECONDS,
            )?,
            env_value(
                "FILE_RECONCILE_LEASE_SECONDS",
                DEFAULT_RECONCILE_LEASE_SECONDS,
            )?,
            env_value("FILE_RECONCILE_BATCH_SIZE", DEFAULT_RECONCILE_BATCH_SIZE)?,
            env_value(
                "FILE_RECONCILE_MAX_ATTEMPTS",
                DEFAULT_RECONCILE_MAX_ATTEMPTS,
            )?,
            env_value(
                "FILE_RECONCILE_RETRY_BASE_SECONDS",
                DEFAULT_RETRY_BASE_SECONDS,
            )?,
            env_value(
                "FILE_RECONCILE_RETRY_MAX_SECONDS",
                DEFAULT_RETRY_MAX_SECONDS,
            )?,
        ];
        Self::from_values(values.each_ref().map(String::as_str))
    }

    fn from_values(values: [&str; 6]) -> Result<Self, RuntimeConfigError> {
        let grant_ttl = parse_bounded(values[0], 1_u64, 300)?;
        let lease = parse_bounded(values[1], 1_u64, 300)?;
        let batch_size = parse_bounded(values[2], 1_i64, 100)?;
        let max_attempts = parse_bounded(values[3], 1_i32, 32)?;
        let retry_base = parse_bounded(values[4], 1_u64, 3_600)?;
        let retry_max = parse_bounded(values[5], 1_u64, 86_400)?;
        if retry_base > retry_max {
            return Err(RuntimeConfigError::InvalidValue);
        }

        Ok(Self {
            private_download_grant_ttl: Duration::from_secs(grant_ttl),
            reconciliation_lease: Duration::from_secs(lease),
            reconciliation_batch_size: batch_size,
            max_operation_attempts: max_attempts,
            retry_base: Duration::from_secs(retry_base),
            retry_max: Duration::from_secs(retry_max),
        })
    }

    pub fn retry_delay(self, attempt: i32) -> Duration {
        let exponent = u32::try_from(attempt.saturating_sub(1).clamp(0, 16)).unwrap_or(0);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        Duration::from_secs(
            self.retry_base
                .as_secs()
                .saturating_mul(multiplier)
                .min(self.retry_max.as_secs()),
        )
    }
}

fn env_value<T: ToString>(name: &str, default: T) -> Result<String, RuntimeConfigError> {
    match env::var(name) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Ok(default.to_string()),
        Err(env::VarError::NotUnicode(_)) => Err(RuntimeConfigError::InvalidValue),
    }
}

fn parse_bounded<T>(value: &str, minimum: T, maximum: T) -> Result<T, RuntimeConfigError>
where
    T: std::str::FromStr + PartialOrd,
{
    let value = value
        .parse::<T>()
        .map_err(|_| RuntimeConfigError::InvalidValue)?;
    if value < minimum || value > maximum {
        return Err(RuntimeConfigError::InvalidValue);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::FilePlatformRuntimeConfig;
    use std::time::Duration;

    #[test]
    fn runtime_configuration_has_safe_bounded_defaults() {
        let config = FilePlatformRuntimeConfig::default();

        assert!(config.private_download_grant_ttl <= Duration::from_secs(300));
        assert!(config.reconciliation_lease <= Duration::from_secs(300));
        assert!((1..=100).contains(&config.reconciliation_batch_size));
        assert!(config.retry_base <= config.retry_max);
    }

    #[test]
    fn runtime_configuration_rejects_invalid_grant_retry_and_lease_values() {
        for values in [
            ["0", "60", "25", "8", "5", "3600"],
            ["301", "60", "25", "8", "5", "3600"],
            ["120", "0", "25", "8", "5", "3600"],
            ["120", "301", "25", "8", "5", "3600"],
            ["120", "60", "0", "8", "5", "3600"],
            ["120", "60", "101", "8", "5", "3600"],
            ["120", "60", "25", "0", "5", "3600"],
            ["120", "60", "25", "33", "5", "3600"],
            ["120", "60", "25", "8", "0", "3600"],
            ["120", "60", "25", "8", "10", "5"],
            ["not-a-number", "60", "25", "8", "5", "3600"],
        ] {
            assert!(
                FilePlatformRuntimeConfig::from_values(values).is_err(),
                "{values:?}"
            );
        }
    }

    #[test]
    fn retry_backoff_is_configurable_and_bounded() {
        let config =
            FilePlatformRuntimeConfig::from_values(["120", "60", "25", "8", "2", "30"]).unwrap();

        assert_eq!(config.retry_delay(0), Duration::from_secs(2));
        assert_eq!(config.retry_delay(1), Duration::from_secs(2));
        assert_eq!(config.retry_delay(2), Duration::from_secs(4));
        assert_eq!(config.retry_delay(100), Duration::from_secs(30));
    }
}
