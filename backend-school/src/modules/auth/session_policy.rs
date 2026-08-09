use chrono::{DateTime, Duration, Utc};

use crate::error::AppError;

pub const ROTATION_INTERVAL: Duration = Duration::minutes(15);
pub const PREVIOUS_TOKEN_GRACE: Duration = Duration::seconds(60);
pub const TOUCH_INTERVAL: Duration = Duration::minutes(5);
pub const THROTTLE_WINDOW: Duration = Duration::minutes(15);
pub const SESSION_RETENTION: Duration = Duration::days(30);
pub const THROTTLE_RETENTION: Duration = Duration::days(1);
pub const CLEANUP_BATCH_SIZE: i64 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLifetime {
    pub idle: Duration,
    pub absolute: Duration,
}

impl SessionLifetime {
    pub fn normal() -> Self {
        Self {
            idle: Duration::hours(2),
            absolute: Duration::hours(12),
        }
    }

    pub fn remembered() -> Self {
        Self {
            idle: Duration::days(7),
            absolute: Duration::days(30),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionTimes {
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub rotated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BucketKind {
    Identifier,
    Source,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ThrottlePolicy;

impl ThrottlePolicy {
    pub fn delay(self, kind: BucketKind, failure_count: u32) -> Option<Duration> {
        let threshold = match kind {
            BucketKind::Identifier => 5,
            BucketKind::Source => 20,
        };
        let exponent = failure_count.checked_sub(threshold)?;
        let seconds = 1_u64.checked_shl(exponent.min(5)).unwrap_or(32).min(30);
        Some(Duration::seconds(seconds as i64))
    }
}

pub fn normalize_login_identifier(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn device_label(user_agent: Option<&str>) -> String {
    let Some(user_agent) = user_agent.filter(|value| !value.is_empty()) else {
        return "Unknown device".to_string();
    };

    let browser = if user_agent.contains("Edg/") {
        Some("Edge")
    } else if user_agent.contains("Chrome/") || user_agent.contains("CriOS/") {
        Some("Chrome")
    } else if user_agent.contains("Firefox/") || user_agent.contains("FxiOS/") {
        Some("Firefox")
    } else if user_agent.contains("Safari/") && user_agent.contains("Version/") {
        Some("Safari")
    } else {
        None
    };

    let operating_system = if user_agent.contains("iPhone") || user_agent.contains("iPad") {
        Some("iOS")
    } else if user_agent.contains("Windows") {
        Some("Windows")
    } else if user_agent.contains("Android") {
        Some("Android")
    } else if user_agent.contains("Mac OS X") {
        Some("macOS")
    } else if user_agent.contains("Linux") {
        Some("Linux")
    } else {
        None
    };

    match (browser, operating_system) {
        (Some(browser), Some(operating_system)) => format!("{browser} on {operating_system}"),
        (Some(browser), None) => browser.to_string(),
        (None, Some(operating_system)) => operating_system.to_string(),
        (None, None) => "Unknown device".to_string(),
    }
}

pub fn cookie_max_age_seconds(
    remember_me: bool,
    absolute_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<u64> {
    if !remember_me {
        return None;
    }

    let remaining = absolute_expires_at.signed_duration_since(now).num_seconds();
    if remaining <= 0 {
        None
    } else {
        Some((remaining as u64).min(2_592_000))
    }
}

pub fn validate_login_input(username: &str, password: &str) -> Result<(), AppError> {
    let username = username.trim();
    if username.is_empty()
        || username.chars().count() > 100
        || username.as_bytes().len() > 400
        || password.is_empty()
        || password.as_bytes().len() > 1_024
    {
        return Err(invalid_login());
    }
    Ok(())
}

pub fn validate_new_password(value: &str) -> Result<(), AppError> {
    let scalar_count = value.chars().count();
    if !(8..=128).contains(&scalar_count) || value.as_bytes().len() > 71 {
        return Err(AppError::BadRequest(
            "รหัสผ่านต้องมี 8–128 ตัวอักษรและไม่เกิน 71 ไบต์".to_string(),
        ));
    }
    Ok(())
}

pub fn retry_after_seconds(blocked_until: DateTime<Utc>, now: DateTime<Utc>) -> Option<u64> {
    let milliseconds = blocked_until.signed_duration_since(now).num_milliseconds();
    if milliseconds <= 0 {
        return None;
    }
    Some(((milliseconds as u64).saturating_add(999) / 1_000).clamp(1, 30))
}

fn invalid_login() -> AppError {
    AppError::AuthError("ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    #[test]
    fn normal_and_remembered_lifetimes_match_contract() {
        assert_eq!(SessionLifetime::normal().idle, Duration::hours(2));
        assert_eq!(SessionLifetime::normal().absolute, Duration::hours(12));
        assert_eq!(SessionLifetime::remembered().idle, Duration::days(7));
        assert_eq!(SessionLifetime::remembered().absolute, Duration::days(30));
    }

    #[test]
    fn throttle_delay_starts_at_owned_thresholds_and_caps() {
        let policy = ThrottlePolicy;

        assert_eq!(policy.delay(BucketKind::Identifier, 4), None);
        assert_eq!(
            policy.delay(BucketKind::Identifier, 5),
            Some(Duration::seconds(1))
        );
        assert_eq!(
            policy.delay(BucketKind::Identifier, 10),
            Some(Duration::seconds(30))
        );
        assert_eq!(policy.delay(BucketKind::Source, 19), None);
        assert_eq!(
            policy.delay(BucketKind::Source, 20),
            Some(Duration::seconds(1))
        );
    }

    #[test]
    fn remembered_cookie_uses_only_remaining_absolute_lifetime() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 0, 0, 0).unwrap();

        assert_eq!(
            cookie_max_age_seconds(false, now + Duration::days(30), now),
            None
        );
        assert_eq!(
            cookie_max_age_seconds(true, now + Duration::days(30), now),
            Some(2_592_000)
        );
        assert_eq!(
            cookie_max_age_seconds(
                true,
                now + Duration::days(1) - Duration::milliseconds(1),
                now
            ),
            Some(86_399)
        );
        assert_eq!(
            cookie_max_age_seconds(true, now - Duration::seconds(1), now),
            None
        );
    }

    #[test]
    fn login_input_is_bounded_and_uses_one_generic_error() {
        assert!(validate_login_input(" teacher.one ", "password").is_ok());

        for (username, password) in [
            ("", "password"),
            ("   ", "password"),
            (&"a".repeat(101), "password"),
            ("teacher.one", ""),
            ("teacher.one", &"p".repeat(1_025)),
        ] {
            let error = validate_login_input(username, password).unwrap_err();
            assert_eq!(error.public_message(), "ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง");
        }
    }

    #[test]
    fn login_identifier_normalization_trims_and_lowercases_unicode() {
        assert_eq!(normalize_login_identifier("  Teacher.หนึ่ง  "), "teacher.หนึ่ง");
    }

    #[test]
    fn new_password_respects_scalar_and_bcrypt_byte_boundaries() {
        assert!(validate_new_password(&"a".repeat(71)).is_ok());
        assert!(validate_new_password(&"ก".repeat(23)).is_ok());

        for invalid in [
            "a".repeat(7),
            "a".repeat(72),
            "ก".repeat(24),
            "a".repeat(129),
        ] {
            assert!(
                validate_new_password(&invalid).is_err(),
                "accepted {} bytes",
                invalid.len()
            );
        }
    }

    #[test]
    fn device_labels_are_coarse_and_never_echo_the_user_agent() {
        let cases = [
            (
                "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/125.0 Safari/537.36",
                "Chrome on Windows",
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1 Version/17.0 Mobile/15E148 Safari/604.1",
                "Safari on iOS",
            ),
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0",
                "Firefox on Linux",
            ),
            ("custom-private-agent/123", "Unknown device"),
        ];

        for (raw, expected) in cases {
            let label = device_label(Some(raw));
            assert_eq!(label, expected);
            assert!(!label.contains(raw));
        }
        assert_eq!(device_label(None), "Unknown device");
    }

    #[test]
    fn retry_after_ceil_is_positive_and_capped() {
        let now = Utc.with_ymd_and_hms(2026, 8, 10, 0, 0, 0).unwrap();

        assert_eq!(
            retry_after_seconds(now + Duration::milliseconds(1), now),
            Some(1)
        );
        assert_eq!(
            retry_after_seconds(now + Duration::seconds(31), now),
            Some(30)
        );
        assert_eq!(retry_after_seconds(now, now), None);
    }
}
