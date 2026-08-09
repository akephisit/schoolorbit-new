use std::{fmt, net::IpAddr};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{digest::Key, Hmac, Mac};
use rand::TryRngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

const CSRF_DOMAIN: &[u8] = b"schoolorbit/session/csrf/v1";
const IDENTIFIER_DOMAIN: &[u8] = b"schoolorbit/login/identifier/v1";
const SOURCE_DOMAIN: &[u8] = b"schoolorbit/login/source/v1";

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SessionHmacKey([u8; 64]);

impl SessionHmacKey {
    pub(crate) fn from_secret(secret: &str) -> Result<Self, AppError> {
        if secret.as_bytes().len() < 32 || secret.as_bytes().len() > 1_024 {
            return Err(AppError::ConfigError(
                "session_hmac_key_invalid".to_string(),
            ));
        }

        let mut key = [0_u8; 64];
        if secret.as_bytes().len() <= key.len() {
            key[..secret.as_bytes().len()].copy_from_slice(secret.as_bytes());
        } else {
            let digest = Sha256::digest(secret.as_bytes());
            key[..digest.len()].copy_from_slice(&digest);
        }

        Ok(Self(key))
    }

    #[cfg(test)]
    pub(crate) fn for_tests(value: [u8; 32]) -> Self {
        let mut key = [0_u8; 64];
        key[..value.len()].copy_from_slice(&value);
        Self(key)
    }

    fn hmac(&self) -> HmacSha256 {
        let key: Key<HmacSha256> = self.0.into();
        <HmacSha256 as Mac>::new(&key)
    }
}

impl fmt::Debug for SessionHmacKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionHmacKey([REDACTED])")
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RawSessionToken([u8; 32]);

impl RawSessionToken {
    pub fn generate() -> Result<Self, AppError> {
        let mut bytes = [0_u8; 32];
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| AppError::ServiceUnavailable("session_rng".to_string()))?;
        Ok(Self(bytes))
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        let bytes = decode_fixed_32(value).ok_or_else(authentication_required)?;
        Ok(Self(bytes))
    }

    pub fn encode(&self) -> EncodedSessionToken {
        EncodedSessionToken(URL_SAFE_NO_PAD.encode(self.0))
    }

    pub fn token_hash(&self) -> TokenHash {
        let digest = Sha256::digest(self.0);
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        TokenHash(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for RawSessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RawSessionToken([REDACTED])")
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EncodedSessionToken(String);

impl EncodedSessionToken {
    pub fn expose_for_cookie(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for EncodedSessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncodedSessionToken([REDACTED])")
    }
}

#[derive(Clone, Copy, Eq)]
pub struct TokenHash([u8; 32]);

impl TokenHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for TokenHash {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl fmt::Debug for TokenHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenHash([REDACTED])")
    }
}

#[derive(Clone, Eq, Zeroize, ZeroizeOnDrop)]
pub struct CsrfToken([u8; 32]);

impl CsrfToken {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        let bytes = decode_fixed_32(value).ok_or_else(csrf_rejected)?;
        Ok(Self(bytes))
    }

    pub fn expose_for_header(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }
}

impl PartialEq for CsrfToken {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl fmt::Debug for CsrfToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CsrfToken([REDACTED])")
    }
}

#[derive(Clone, Copy, Eq)]
pub struct ThrottleBucketHash([u8; 32]);

impl ThrottleBucketHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for ThrottleBucketHash {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl fmt::Debug for ThrottleBucketHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ThrottleBucketHash([REDACTED])")
    }
}

pub fn session_csrf_token(key: &SessionHmacKey, tenant_id: Uuid, session_id: Uuid) -> CsrfToken {
    let mut mac = key.hmac();
    mac.update(CSRF_DOMAIN);
    mac.update(tenant_id.as_bytes());
    mac.update(session_id.as_bytes());
    CsrfToken(finalize_hmac(mac))
}

pub fn identifier_bucket(
    key: &SessionHmacKey,
    tenant_id: Uuid,
    normalized_username: &str,
) -> ThrottleBucketHash {
    let mut mac = key.hmac();
    mac.update(IDENTIFIER_DOMAIN);
    mac.update(tenant_id.as_bytes());
    mac.update(normalized_username.as_bytes());
    ThrottleBucketHash(finalize_hmac(mac))
}

pub fn source_bucket(key: &SessionHmacKey, tenant_id: Uuid, source: IpAddr) -> ThrottleBucketHash {
    let mut mac = key.hmac();
    mac.update(SOURCE_DOMAIN);
    mac.update(tenant_id.as_bytes());
    match normalize_ip(source) {
        IpAddr::V4(address) => {
            mac.update(&[4]);
            mac.update(&address.octets());
        }
        IpAddr::V6(address) => {
            mac.update(&[6]);
            mac.update(&address.octets());
        }
    }
    ThrottleBucketHash(finalize_hmac(mac))
}

pub(crate) fn normalize_ip(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(address)),
        address => address,
    }
}

fn decode_fixed_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 43 || !value.is_ascii() {
        return None;
    }

    let decoded = URL_SAFE_NO_PAD.decode(value).ok()?;
    let bytes: [u8; 32] = decoded.try_into().ok()?;
    if URL_SAFE_NO_PAD
        .encode(bytes)
        .as_bytes()
        .ct_eq(value.as_bytes())
        .into()
    {
        Some(bytes)
    } else {
        None
    }
}

fn finalize_hmac(mac: HmacSha256) -> [u8; 32] {
    let output = mac.finalize().into_bytes();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&output);
    bytes
}

fn authentication_required() -> AppError {
    AppError::AuthError("Authentication required".to_string())
}

fn csrf_rejected() -> AppError {
    AppError::Forbidden("CSRF validation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use std::net::IpAddr;
    use uuid::Uuid;

    #[test]
    fn token_round_trip_is_32_bytes_and_debug_is_redacted() {
        let token = RawSessionToken::generate().unwrap();
        let encoded = token.encode();

        assert_eq!(
            RawSessionToken::parse(encoded.expose_for_cookie())
                .unwrap()
                .as_bytes()
                .len(),
            32
        );
        assert_eq!(encoded.expose_for_cookie().len(), 43);
        assert_eq!(format!("{token:?}"), "RawSessionToken([REDACTED])");
        assert!(!format!("{encoded:?}").contains(encoded.expose_for_cookie()));
    }

    #[test]
    fn token_parser_rejects_noncanonical_or_wrong_length_values() {
        for value in [
            format!("{}=", URL_SAFE_NO_PAD.encode([1_u8; 32])),
            "not+base64url".to_string(),
            URL_SAFE_NO_PAD.encode([1_u8; 31]),
            URL_SAFE_NO_PAD.encode([1_u8; 33]),
            format!(" {}", URL_SAFE_NO_PAD.encode([1_u8; 32])),
            format!("\"{}\"", URL_SAFE_NO_PAD.encode([1_u8; 32])),
        ] {
            let error = RawSessionToken::parse(&value).unwrap_err();
            assert_eq!(error.status_code(), axum::http::StatusCode::UNAUTHORIZED);
            assert_eq!(error.public_message(), "Authentication required");
            assert!(!format!("{error:?}").contains(&value));
        }
    }

    #[test]
    fn csrf_parser_rejects_noncanonical_values() {
        let valid = URL_SAFE_NO_PAD.encode([3_u8; 32]);
        assert_eq!(CsrfToken::parse(&valid).unwrap().expose_for_header(), valid);

        for value in [
            format!("{valid}="),
            format!(" {valid}"),
            URL_SAFE_NO_PAD.encode([3_u8; 31]),
            "not+base64url".to_string(),
        ] {
            let error = CsrfToken::parse(&value).unwrap_err();
            assert_eq!(error.status_code(), axum::http::StatusCode::FORBIDDEN);
            assert_eq!(error.public_message(), "CSRF validation failed");
            assert!(!format!("{error:?}").contains(&value));
        }
    }

    #[test]
    fn token_hash_is_sha256_and_constant_time_comparable() {
        let token = RawSessionToken::from_bytes([9_u8; 32]);
        let same = RawSessionToken::from_bytes([9_u8; 32]);
        let different = RawSessionToken::from_bytes([8_u8; 32]);

        assert_eq!(token.token_hash(), same.token_hash());
        assert_ne!(token.token_hash(), different.token_hash());
        assert_eq!(token.token_hash().as_bytes().len(), 32);
        assert_eq!(format!("{:?}", token.token_hash()), "TokenHash([REDACTED])");
    }

    #[test]
    fn hmac_domains_and_logical_session_inputs_do_not_overlap() {
        let key = SessionHmacKey::for_tests([7_u8; 32]);
        let tenant = Uuid::new_v4();
        let session = Uuid::new_v4();
        let csrf = session_csrf_token(&key, tenant, session);

        assert_eq!(csrf, session_csrf_token(&key, tenant, session));
        assert_ne!(
            csrf.expose_for_header(),
            session_csrf_token(&key, Uuid::new_v4(), session).expose_for_header()
        );
        assert_ne!(
            csrf.expose_for_header(),
            session_csrf_token(&key, tenant, Uuid::new_v4()).expose_for_header()
        );
        assert_ne!(
            csrf.expose_for_header(),
            URL_SAFE_NO_PAD.encode(identifier_bucket(&key, tenant, "teacher.one").as_bytes())
        );
        assert_ne!(
            identifier_bucket(&key, Uuid::nil(), "teacher.one"),
            source_bucket(&key, Uuid::nil(), "203.0.113.9".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn ipv4_mapped_ipv6_uses_the_same_source_bucket() {
        let key = SessionHmacKey::for_tests([5_u8; 32]);
        let tenant = Uuid::new_v4();

        assert_eq!(
            source_bucket(&key, tenant, "203.0.113.9".parse().unwrap()),
            source_bucket(&key, tenant, "::ffff:203.0.113.9".parse().unwrap())
        );
    }
}
