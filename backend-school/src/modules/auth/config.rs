use std::{collections::HashSet, env};

use ipnet::IpNet;
use url::Url;

use crate::error::AppError;

use super::session_crypto::SessionHmacKey;

pub const SESSION_COOKIE_NAME: &str = "__Host-schoolorbit_session";
pub const LEGACY_COOKIE_NAME: &str = "auth_token";
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";

pub struct SessionConfig {
    hmac_key: SessionHmacKey,
    pub base_domain: String,
    pub trusted_proxy_cidrs: Vec<IpNet>,
    pub allowed_dev_origins: HashSet<String>,
}

impl SessionConfig {
    pub fn from_env() -> Result<Self, AppError> {
        dotenvy::dotenv().ok();
        let hmac_key = required_env("SESSION_HMAC_KEY", "session_hmac_key_missing")?;
        let base_domain = required_env("BASE_DOMAIN", "base_domain_missing")?;
        let trusted_proxy_cidrs = env::var("TRUSTED_PROXY_CIDRS").unwrap_or_default();
        let allowed_dev_origins = env::var("SCHOOL_ALLOWED_DEV_ORIGINS").unwrap_or_default();

        Self::from_values(
            &hmac_key,
            &base_domain,
            &trusted_proxy_cidrs,
            &allowed_dev_origins,
        )
    }

    pub fn hmac_key(&self) -> &SessionHmacKey {
        &self.hmac_key
    }

    fn from_values(
        hmac_key: &str,
        base_domain: &str,
        trusted_proxy_cidrs: &str,
        allowed_dev_origins: &str,
    ) -> Result<Self, AppError> {
        Ok(Self {
            hmac_key: SessionHmacKey::from_secret(hmac_key)?,
            base_domain: parse_base_domain(base_domain)?,
            trusted_proxy_cidrs: parse_cidrs(trusted_proxy_cidrs)?,
            allowed_dev_origins: parse_origins(allowed_dev_origins)?,
        })
    }
}

fn required_env(name: &str, reason: &str) -> Result<String, AppError> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::ConfigError(reason.to_string()))
}

fn parse_base_domain(value: &str) -> Result<String, AppError> {
    if value.is_empty() || value.trim() != value || value.len() > 253 || !value.is_ascii() {
        return Err(config_error("base_domain_invalid"));
    }

    let normalized = value.to_ascii_lowercase();
    let labels = normalized.split('.').collect::<Vec<_>>();
    if labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(config_error("base_domain_invalid"));
    }

    Ok(normalized)
}

fn parse_cidrs(value: &str) -> Result<Vec<IpNet>, AppError> {
    parse_comma_separated(value, "trusted_proxy_cidrs_invalid", |entry| {
        entry.parse::<IpNet>().ok()
    })
}

fn parse_origins(value: &str) -> Result<HashSet<String>, AppError> {
    let origins = parse_comma_separated(value, "allowed_dev_origins_invalid", |entry| {
        let url = Url::parse(entry).ok()?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        Some(url.origin().ascii_serialization())
    })?;
    Ok(origins.into_iter().collect())
}

fn parse_comma_separated<T, C>(
    value: &str,
    reason: &str,
    mut convert: C,
) -> Result<Vec<T>, AppError>
where
    C: FnMut(&str) -> Option<T>,
{
    if value.is_empty() {
        return Ok(Vec::new());
    }

    value
        .split(',')
        .map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                None
            } else {
                convert(entry)
            }
            .ok_or_else(|| config_error(reason))
        })
        .collect()
}

fn config_error(reason: &str) -> AppError {
    AppError::ConfigError(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_hmac_key() {
        let result = SessionConfig::from_values("short", "schoolorbit.app", "", "");
        let error = match result {
            Ok(_) => panic!("short HMAC key must be rejected"),
            Err(error) => error,
        };

        assert_eq!(error.public_message(), "System configuration error");
        assert!(!format!("{error:?}").contains("short"));
    }

    #[test]
    fn rejects_invalid_proxy_cidr_and_non_origin_development_url() {
        assert!(
            SessionConfig::from_values(&"k".repeat(32), "schoolorbit.app", "not-a-cidr", "",)
                .is_err()
        );
        assert!(SessionConfig::from_values(
            &"k".repeat(32),
            "schoolorbit.app",
            "",
            "http://localhost:5173/path",
        )
        .is_err());
    }

    #[test]
    fn parses_valid_production_and_development_values() {
        let config = SessionConfig::from_values(
            &"k".repeat(32),
            "SchoolOrbit.App",
            "10.0.0.0/8, 2001:db8::/32",
            "http://localhost:5173,https://preview.example.test",
        )
        .unwrap();

        assert_eq!(config.base_domain, "schoolorbit.app");
        assert_eq!(config.trusted_proxy_cidrs.len(), 2);
        assert!(config.allowed_dev_origins.contains("http://localhost:5173"));
        assert!(config
            .allowed_dev_origins
            .contains("https://preview.example.test"));
        assert_eq!(
            format!("{:?}", config.hmac_key()),
            "SessionHmacKey([REDACTED])"
        );
    }
}
