use super::types::AdminClaims;
use crate::error::AppError;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use std::env;

fn jwt_secret() -> Result<String, AppError> {
    env::var("JWT_SECRET")
        .map_err(|_| AppError::InternalServerError("JWT configuration unavailable".to_string()))
}

fn generate_token_with_secret(claims: AdminClaims, secret: &[u8]) -> Result<String, AppError> {
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|error| AppError::InternalServerError(format!("JWT generation failed: {error}")))
}

fn validate_token_with_secret(token: &str, secret: &[u8]) -> Result<AdminClaims, AppError> {
    decode::<AdminClaims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::new(Algorithm::HS256),
    )
    .map(|token_data| token_data.claims)
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))
}

pub fn generate_token(claims: AdminClaims) -> Result<String, AppError> {
    let secret = jwt_secret()?;
    generate_token_with_secret(claims, secret.as_bytes())
}

pub fn validate_token(token: &str) -> Result<AdminClaims, AppError> {
    let secret = jwt_secret()?;
    validate_token_with_secret(token, secret.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AdminRole;

    const FIXTURE_SECRET: &[u8] = b"wave-four-jwt-fixture-secret";
    const V9_HS256_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIwMDAwMDAwMC0wMDAwLTAwMDAtMDAwMC0wMDAwMDAwMDAwMDEiLCJlbWFpbCI6ImFkbWluQGV4YW1wbGUuaW52YWxpZCIsInJvbGUiOiJhZG1pbiIsImV4cCI6NDEwMjQ0NDgwMCwiaWF0IjoxODAwMDAwMDAwfQ.cHWfX1jgPcasffE2Vvlf7Z394O3RZNj0MOCuivqPKCA";

    fn fixture_claims() -> AdminClaims {
        AdminClaims {
            sub: "00000000-0000-0000-0000-000000000001".to_string(),
            email: "admin@example.invalid".to_string(),
            role: AdminRole::Admin,
            exp: 4_102_444_800,
            iat: 1_800_000_000,
        }
    }

    #[test]
    fn hs256_token_format_remains_compatible_with_v9() {
        let emitted = generate_token_with_secret(fixture_claims(), FIXTURE_SECRET).unwrap();
        assert_eq!(emitted, V9_HS256_TOKEN);

        let claims = validate_token_with_secret(V9_HS256_TOKEN, FIXTURE_SECRET).unwrap();
        assert_eq!(claims.sub, "00000000-0000-0000-0000-000000000001");
        assert_eq!(claims.email, "admin@example.invalid");
        assert_eq!(claims.role, AdminRole::Admin);
        assert_eq!(claims.exp, 4_102_444_800);
        assert_eq!(claims.iat, 1_800_000_000);
    }

    #[test]
    fn hs256_validation_rejects_a_changed_signature() {
        let altered = V9_HS256_TOKEN.replacen(".cHWf", ".dHWf", 1);
        assert!(matches!(
            validate_token_with_secret(&altered, FIXTURE_SECRET),
            Err(AppError::Unauthorized(_))
        ));
    }

    #[test]
    fn hs256_validation_rejects_another_algorithm_and_expired_claims() {
        let other_algorithm = encode(
            &Header::new(Algorithm::HS384),
            &fixture_claims(),
            &EncodingKey::from_secret(FIXTURE_SECRET),
        )
        .unwrap();
        assert!(matches!(
            validate_token_with_secret(&other_algorithm, FIXTURE_SECRET),
            Err(AppError::Unauthorized(_))
        ));

        let mut expired = fixture_claims();
        expired.exp = 1;
        let expired = generate_token_with_secret(expired, FIXTURE_SECRET).unwrap();
        assert!(matches!(
            validate_token_with_secret(&expired, FIXTURE_SECRET),
            Err(AppError::Unauthorized(_))
        ));
    }
}
