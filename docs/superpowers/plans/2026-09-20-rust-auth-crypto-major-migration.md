# Rust Authentication and Cryptography Major Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the remaining Rust authentication and cryptography dependencies to their newest reviewed stable releases while preserving every deployed token, password hash, encrypted value, blind index, session credential, checksum, secret boundary, and rollback guarantee.

**Architecture:** `backend-admin` keeps its existing HS256 JWT contract but moves to JsonWebToken 11 with the explicit pure-Rust crypto provider and aligns bcrypt with the school backend. `backend-school` continues to centralize dependency versions at the workspace root; `school-crypto`, `school-auth`, and `school-certificates` retain their existing ownership while moving AES-GCM, HMAC, SHA-2, and security-sensitive randomness together so their shared type/API boundary cannot drift. Fixed pre-upgrade vectors characterize all persisted or cross-release formats before any version changes, and direct random generation uses the operating-system `SysRng` through fallible APIs.

**Tech Stack:** Rust 1.98.1, Cargo, JsonWebToken 11.1/RustCrypto HS256, bcrypt 0.19, AES-GCM 0.11, HMAC 0.13, SHA-2 0.11, Rand 0.10/SysRng, Base64 0.23, rootless Podman/PostgreSQL 18, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-20-repository-dependency-modernization-design.md`

## Global Constraints

- Read `.rules`, `docs/TESTING.md`, and `docs/OPERATIONS.md` before implementation; `.rules` remains authoritative.
- Continue on `chore/modernize-rust-auth-crypto-dependencies`, which was created from the accepted `origin/main`; do not implement on `main`.
- Reconfirm registry versions before editing. The reviewed stable targets on 2026-09-20 are JsonWebToken `11.1.0`, bcrypt `0.19.3`, AES-GCM `0.11.1`, HMAC `0.13.0`, SHA-2 `0.11.0`, and Rand `0.10.2`. If a newer stable release exists at execution time, stop and amend this reviewed plan instead of silently changing a target.
- Rust 1.98.1 is the repository toolchain and satisfies JsonWebToken 11's Rust 1.88 requirement and the RustCrypto/Rand Rust 1.85 requirements. Do not add a toolchain override or lower the toolchain.
- Preserve backend-admin JWT header and claim JSON, HS256-only validation, 60-second validation leeway, `JWT_SECRET` ownership, 24-hour issued-token lifetime, cookie behavior, and acceptance of already-issued valid tokens.
- Configure JsonWebToken with `default-features = false` and only `rust_crypto`. Do not enable unused PEM handling or AWS-LC, install a process-global custom provider, allow algorithm-family fallback, or use any insecure decode API.
- Preserve bcrypt's stored `$2b$` hashes and current cost at every owner. Backend-school retains its shared 8–128 Unicode scalar and 71-byte non-truncating new-password policy. This wave does not redesign backend-admin password policy or rehash stored passwords.
- Preserve the exact encrypted-field format: SHA-256 of the stable `ENCRYPTION_KEY` string is the AES-256 key; output is standard padded Base64 of `12-byte nonce || ciphertext || 16-byte tag`. Existing ciphertext must decrypt without re-encryption.
- Preserve blind-index, domain-separated HMAC, CSRF, throttle-bucket, session-token SHA-256, file checksum, academic revision, and certificate-verification bytes exactly. Do not rotate `ENCRYPTION_KEY`, `BLIND_INDEX_KEY`, `SESSION_HMAC_KEY`, or `JWT_SECRET`.
- Use fallible operating-system randomness through Rand 0.10 `SysRng` for the only three direct consumers: session credentials, AES-GCM nonces, and certificate proofs. Do not use seeded, deterministic, thread-local, or silently panicking randomness in runtime security paths.
- Declare AES-GCM with only `aes`, `alloc`, and `zeroize`; HMAC and SHA-2 with only `zeroize`; Rand with only `sys_rng`; and bcrypt with only `std` and `zeroize`. Keep all defaults disabled where the manifest syntax supports it.
- Remove the unused direct `sha256 = "1.6.0"` workspace dependency after a repository-wide source search proves it has no consumer. Do not remove older transitive HMAC, SHA-2, or Rand versions owned by JsonWebToken or other upstream crates.
- Never edit an applied migration. This wave adds no migration and changes no database schema, SQL, query count, endpoint, DTO, OpenAPI output, permission, realtime payload, frontend route, or deployment topology.
- Never put real national IDs, passwords, JWTs, session tokens, cookies, CSRF values, encryption keys, blind-index keys, database URLs, or production-derived ciphertext in source, fixtures, logs, commands, or screenshots. All fixed vectors use synthetic values and non-production keys.
- Run focused tests after every boundary, then the complete authentication, encryption/PII, backend, contract, and deployment matrix. An unavailable credential or external target is reported as unrun, never as passing.
- Keep both this plan and the program design on the implementation branch while work is active. Remove both after all four waves are implemented and verified and before Wave 4 squash integration, as required by `.rules`.

## Review Focus

- **Cross-release JWTs:** a fixed token emitted by JsonWebToken 9 must validate under 11, JsonWebToken 11 must emit the same HS256 compact value, and altered signatures, algorithms, or expiry must still fail closed.
- **Stored secrets:** the pre-upgrade bcrypt hash and AES-GCM ciphertext fixtures must remain readable without rehashing, re-encryption, key rotation, or fallback formats.
- **Derived bytes:** blind indexes, domain-separated hashes, CSRF tokens, throttle buckets, session-token hashes, file hashes, and academic revision checksums must retain exact known outputs.
- **Randomness failure:** session-token, nonce, and certificate-proof generation must use fallible OS entropy, preserve output sizes/encodings, map failure to bounded errors, and never expose partially generated bytes.
- **Dependency graph:** direct owners must resolve to the reviewed versions and minimal features; older transitive majors may remain only when `cargo tree -i <crate>@<version>` identifies an upstream owner.
- **Security handling:** keys and credentials remain environment-owned and redacted; runtime paths do not gain `unwrap`, `expect`, insecure JWT decoding, plaintext logging, or error text containing secret material.
- **Release acceptance:** both backend deployments must succeed for the exact integrated SHA, including backend-school authenticated session/CSRF smoke and accepted-release evidence before the modernization program is declared complete.

---

## File Structure

### Dependency ownership and executable policy

- Modify `backend-school/Cargo.toml` — own bcrypt 0.19.3, AES-GCM 0.11.1, HMAC 0.13.0, SHA-2 0.11.0, and Rand 0.10.2 with explicit features; remove the unused `sha256` declaration and redundant root Rand/HMAC consumers.
- Modify `backend-school/Cargo.lock` — lock the reviewed school graph without forced overrides.
- Modify `backend-admin/Cargo.toml` — own JsonWebToken 11.1.0 with `rust_crypto` and bcrypt 0.19.3 with explicit features.
- Modify `backend-admin/Cargo.lock` — lock the reviewed admin graph.
- Modify `backend-school/tests/static_architecture.rs` — enforce exact direct versions/features, the three Rand consumers, the absence of direct school JWT and `sha256` owners, and the explicit admin JWT provider.

### Admin authentication compatibility

- Modify `backend-admin/src/auth/jwt.rs` — keep the public API, remove runtime environment panics, isolate secret-parameterized helpers for deterministic vectors, and preserve HS256 validation.
- Modify `backend-admin/src/auth/validation.rs` — add legacy bcrypt verification and new-hash round-trip coverage.
- Read-only verify `backend-admin/src/auth/types.rs`, `backend-admin/src/services/auth_service.rs`, `backend-admin/src/middleware/auth.rs`, and `backend-admin/src/handlers/auth.rs` — confirm claims, 24-hour lifetime, role gating, cookie extraction, and callers stay unchanged.

### School stored-format and session compatibility

- Modify `backend-school/crates/school-crypto/src/lib.rs` — add deterministic nonce injection behind the public random encryptor, migrate AES/HMAC/SHA/Rand APIs, and pin ciphertext/blind-index vectors.
- Modify `backend-school/crates/school-auth/src/session_crypto.rs` — migrate HMAC/SHA/Rand APIs and pin session-token, CSRF, identifier-bucket, and source-bucket vectors.
- Modify `backend-school/crates/school-certificates/src/services/proof.rs` — migrate certificate proof generation to fallible `SysRng` without changing Base64URL, encrypted proof, or domain-separated hash behavior.
- Read-only verify every direct SHA-2 and bcrypt owner listed by repository search, especially File Platform hashes, academic checksums/revisions, certificate verification/limiting, school login/password change, provisioning, admission, staff, students, sandbox seeding, and test database fixtures.

---

### Task 1: Reconfirm the Security Graph and Capture a Passing Baseline

**Files:**
- Read: `backend-school/Cargo.toml`
- Read: `backend-admin/Cargo.toml`
- Read: direct dependency consumers found by repository search

**Interfaces:**
- Consumes: accepted Wave 3 production tree `d1851298413b0e31a96a07c51c498427ed65f4a0` and current registry metadata.
- Produces: a clean reviewed implementation branch, confirmed targets, an exact owner inventory, and passing pre-upgrade behavior evidence.

- [ ] **Step 1: Verify the reviewed implementation branch and accepted base**

Run from the repository root:

```bash
git fetch origin
git status --short --branch
test "$(git branch --show-current)" = "chore/modernize-rust-auth-crypto-dependencies"
git merge-base --is-ancestor origin/main HEAD
git rev-list --left-right --count origin/main...HEAD
```

Expected: the feature tree is clean apart from the reviewed plan commit and is based on the accepted `origin/main`. If `origin/main` advanced, merge it into this feature branch, inspect the intervening changes, and amend the plan when they affect the migration before continuing.

- [ ] **Step 2: Reconfirm toolchain, versions, features, and MSRV**

Run:

```bash
rustc --version
cargo --version
cargo info jsonwebtoken@11.1.0
cargo info bcrypt@0.19.3
cargo info aes-gcm@0.11.1
cargo info hmac@0.13.0
cargo info sha2@0.11.0
cargo info rand@0.10.2
```

Expected: Rust/Cargo are 1.98.1; the exact targets remain newest stable releases; JsonWebToken exposes `rust_crypto`; AES-GCM exposes `aes`, `alloc`, and `zeroize`; HMAC/SHA-2 expose `zeroize`; Rand exposes `sys_rng`; bcrypt exposes `std` and `zeroize`.

- [ ] **Step 3: Reconfirm every direct source owner**

Run:

```bash
rg -n 'jsonwebtoken|bcrypt|aes-gcm|hmac|sha2|sha256|rand' \
  backend-school/Cargo.toml backend-school/crates/*/Cargo.toml backend-admin/Cargo.toml
rg -n '\b(jsonwebtoken|bcrypt|aes_gcm|hmac|sha2|sha256|rand)::|use (jsonwebtoken|bcrypt|aes_gcm|hmac|sha2|sha256|rand)' \
  backend-school backend-admin --glob '*.rs' --glob '!target/**'
```

Expected: JsonWebToken is admin-only; AES-GCM is owned by `school-crypto`; direct Rand source use is limited to `school-auth`, `school-crypto`, and `school-certificates`; `sha256` has no source consumer; HMAC is limited to `school-auth` and `school-crypto`; bcrypt and SHA-2 consumers match the file structure above.

- [ ] **Step 4: Capture the passing pre-upgrade behavior baseline**

Run:

```bash
cd backend-admin
cargo test auth::
cd ../backend-school
cargo test -p school-crypto
cargo test -p school-auth session_crypto::tests
cargo test -p school-auth session_policy::tests
cargo test -p school-certificates services::proof::tests -- --test-threads=8
cargo test -p school-file-platform file_hash::tests
cargo test -p school-academic-results services::term_preview::tests::term_preview_preserves_the_pre_batch_source_checksum_contract -- --exact
cd ..
```

Expected: every existing focused test passes on the old direct versions. Save only command/result summaries; never copy runtime keys, hashes from production, or credential-bearing output.

---

### Task 2: Upgrade Backend Admin JWT and Bcrypt Without Invalidating Stored Credentials

**Files:**
- Modify: `backend-admin/Cargo.toml`
- Modify: `backend-admin/Cargo.lock`
- Modify: `backend-admin/src/auth/jwt.rs`
- Modify: `backend-admin/src/auth/validation.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `AdminClaims`, `JWT_SECRET`, HS256, existing `$2b$` password hashes, and public `generate_token`/`validate_token`/`hash_password`/`verify_password` functions.
- Produces: the same public functions and wire values backed by JsonWebToken 11.1.0 and bcrypt 0.19.3.

- [ ] **Step 1: Add deterministic JWT helpers and the fixed JsonWebToken 9 vector test**

Refactor `backend-admin/src/auth/jwt.rs` so the environment-facing functions remain public while tests can exercise secret-parameterized private helpers:

```rust
fn jwt_secret() -> Result<String, AppError> {
    env::var("JWT_SECRET").map_err(|_| {
        AppError::InternalServerError("JWT configuration unavailable".to_string())
    })
}

fn generate_token_with_secret(
    claims: AdminClaims,
    secret: &[u8],
) -> Result<String, AppError> {
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|error| {
        AppError::InternalServerError(format!("JWT generation failed: {error}"))
    })
}

fn validate_token_with_secret(
    token: &str,
    secret: &[u8],
) -> Result<AdminClaims, AppError> {
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
```

Add a `#[cfg(test)]` module using only these synthetic values:

```rust
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
```

- [ ] **Step 2: Add the legacy bcrypt fixture and new-hash round-trip**

Add to `backend-admin/src/auth/validation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    const LEGACY_BCRYPT_HASH: &str =
        "$2b$12$rri4KlbA4XkEM.JADCiBdu3/cjxESITAXMOYiKoklPN9HW6vgjEP2";

    #[test]
    fn legacy_bcrypt_hashes_remain_verifiable() {
        assert!(verify_password(
            "synthetic-password-for-wave-four",
            LEGACY_BCRYPT_HASH,
        )
        .unwrap());
        assert!(!verify_password("wrong-synthetic-password", LEGACY_BCRYPT_HASH).unwrap());
    }

    #[test]
    fn newly_generated_bcrypt_hashes_keep_the_existing_format_and_cost() {
        let hash = hash_password("another-synthetic-password").unwrap();
        assert!(hash.starts_with("$2b$12$"));
        assert!(verify_password("another-synthetic-password", &hash).unwrap());
    }
}
```

- [ ] **Step 3: Run the compatibility tests against the old libraries**

Run:

```bash
cd backend-admin
cargo test auth::
```

Expected: PASS before the version change. This proves the constants describe the deployed v9 JWT and v0.15 bcrypt behavior rather than values first generated by the new libraries.

- [ ] **Step 4: Add the failing admin dependency-policy test**

Add `admin_auth_dependencies_follow_the_reviewed_policy` to `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn admin_auth_dependencies_follow_the_reviewed_policy() {
    let admin_manifest = read_source(repo_root().join("backend-admin/Cargo.toml"));
    assert!(admin_manifest.contains(
        "jsonwebtoken = { version = \"11.1.0\", default-features = false, features = [\"rust_crypto\"] }"
    ));
    assert!(admin_manifest.contains(
        "bcrypt = { version = \"0.19.3\", default-features = false, features = [\"std\", \"zeroize\"] }"
    ));
    let jwt_source = read_source(repo_root().join("backend-admin/src/auth/jwt.rs"));
    assert!(!jwt_source.contains(".expect("));
    assert!(!jwt_source.contains("insecure_decode"));
    assert!(!read_source(manifest_dir().join("Cargo.toml")).contains("jsonwebtoken"));
}
```

Run:

```bash
cd ../backend-school
cargo test --test static_architecture admin_auth_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: FAIL because backend-admin still declares JsonWebToken 9.3.1 and bcrypt 0.15.1.

- [ ] **Step 5: Apply the exact admin dependency declarations**

Change `backend-admin/Cargo.toml` to:

```toml
jsonwebtoken = { version = "11.1.0", default-features = false, features = ["rust_crypto"] }
bcrypt = { version = "0.19.3", default-features = false, features = ["std", "zeroize"] }
```

Run an unlocked check once to resolve the reviewed graph, then use locked commands thereafter:

```bash
cd ../backend-admin
cargo check --all-targets
cargo test auth::
cargo check --locked --all-targets
cd ../backend-school
cargo test --test static_architecture admin_auth_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: all pass; the fixed v9 token is both emitted and accepted; the legacy bcrypt hash verifies; no PEM or AWS-LC feature is enabled.

- [ ] **Step 6: Audit the admin graph and commit**

Run:

```bash
cd ../backend-admin
cargo tree -e features -i jsonwebtoken@11.1.0
cargo tree -e features -i bcrypt@0.19.3
cargo tree -d
cargo fmt --all -- --check
cd ..
git diff --check
git add backend-admin backend-school/tests/static_architecture.rs
git commit -m "chore: migrate admin authentication dependencies"
```

Expected: JsonWebToken has exactly one direct owner with `rust_crypto`; bcrypt resolves directly to 0.19.3. Record JsonWebToken-owned HMAC 0.12/SHA-2 0.10/Rand 0.8 duplicates as upstream-owned, not as new direct dependencies.

---

### Task 3: Pin School Ciphertext, HMAC, and Session-Derivation Vectors Before Upgrading

**Files:**
- Modify: `backend-school/crates/school-crypto/src/lib.rs`
- Modify: `backend-school/crates/school-auth/src/session_crypto.rs`

**Interfaces:**
- Consumes: current AES-GCM 0.10, HMAC 0.12, SHA-2 0.10, Rand 0.9 behavior and the existing stored ciphertext fixture.
- Produces: version-independent exact vectors that both the old and new libraries must satisfy.

- [ ] **Step 1: Write the failing deterministic encryption-vector test**

Extend `standard_base64_and_legacy_ciphertext_remain_compatible` in `school-crypto`:

```rust
const STORED_FORMAT_CIPHERTEXT: &str =
    "AAECAwQFBgcICQoLMz5OUZn55E+f9hdUy/hd2vqHkvCmExY6AzMgW9b6NkAGj9uHkA==";

assert_eq!(
    encrypt_with_nonce(
        "stored-format-fixture",
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    )
    .unwrap(),
    STORED_FORMAT_CIPHERTEXT,
);
assert_eq!(
    decrypt(STORED_FORMAT_CIPHERTEXT).unwrap(),
    "stored-format-fixture",
);
```

Run:

```bash
cd backend-school
cargo test -p school-crypto tests::standard_base64_and_legacy_ciphertext_remain_compatible -- --exact
```

Expected: FAIL to compile because `encrypt_with_nonce` does not exist.

- [ ] **Step 2: Extract deterministic nonce assembly without changing the public encryptor**

Keep the empty-string behavior in `encrypt`, fill a fresh 12-byte nonce there, then call this private helper:

```rust
fn encrypt_with_nonce(plaintext: &str, nonce_bytes: [u8; 12]) -> Result<String, String> {
    let cipher = get_cipher()?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|error| format!("Encryption failed: {error}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(general_purpose::STANDARD.encode(result))
}
```

At this pre-upgrade step, use the AES-GCM 0.10 API shown above. Task 4 replaces deprecated array calls for 0.11. The public result remains standard padded Base64 of nonce plus ciphertext/tag.

- [ ] **Step 3: Add exact domain-separated blind-index output**

Extend `domain_separated_hashes_do_not_overlap_other_features` with:

```rust
assert_eq!(
    certificate,
    "cdc708938d1e5a4b21e8105a67ead62d59c1af4953bd1475c929109def59bbfe",
);
```

Keep the existing exact `hash_for_search` vector unchanged.

- [ ] **Step 4: Add exact session SHA-256 and HMAC vectors**

In `school-auth/src/session_crypto.rs`, extend the token-hash test:

```rust
assert_eq!(
    hex::encode(token.token_hash().as_bytes()),
    "8c0cc17a04942cc4f8e0fe0b302606d3108860c126428ba2ceeb5f9ed41c2b05",
);
```

Add:

```rust
#[test]
fn csrf_and_throttle_hmac_outputs_remain_stable() {
    let key = SessionHmacKey::for_tests([7_u8; 32]);
    let tenant = Uuid::nil();
    let session = Uuid::from_u128(1);

    assert_eq!(
        session_csrf_token(&key, tenant, session).expose_for_header(),
        "kW1kSYKYiGjNyOSlllnyodRuchSkrbOz6rRW_klbMSQ",
    );
    assert_eq!(
        hex::encode(identifier_bucket(&key, tenant, "teacher.one").as_bytes()),
        "4d03f9b95ed261e64480fdcfed01d62652515f919ef91c6a47a20912f180a811",
    );
    assert_eq!(
        hex::encode(
            source_bucket(&key, tenant, "203.0.113.9".parse().unwrap()).as_bytes(),
        ),
        "4131ad434dcd458df1a0bd57a0260259485d2a64a2cc104b5727fc94e87f76eb",
    );
}
```

The literal IP is the RFC 5737 documentation range. No user or production identity enters the fixture.

- [ ] **Step 5: Prove all vectors pass before the major upgrade**

Run:

```bash
cargo test -p school-crypto
cargo test -p school-auth session_crypto::tests
cargo test -p school-file-platform file_hash::tests
cargo test -p school-academic-results services::term_preview::tests::term_preview_preserves_the_pre_batch_source_checksum_contract -- --exact
```

Expected: PASS under AES-GCM 0.10, HMAC 0.12, SHA-2 0.10, and Rand 0.9.

- [ ] **Step 6: Commit the pre-upgrade compatibility evidence**

```bash
cargo fmt --all -- --check
cd ..
git add backend-school/crates/school-crypto/src/lib.rs \
  backend-school/crates/school-auth/src/session_crypto.rs
git commit -m "test: pin authentication cryptography formats"
```

---

### Task 4: Upgrade the School Cryptography Stack and Use Fallible OS Entropy

**Files:**
- Modify: `backend-school/Cargo.toml`
- Modify: `backend-school/Cargo.lock`
- Modify: `backend-school/crates/school-crypto/src/lib.rs`
- Modify: `backend-school/crates/school-auth/src/session_crypto.rs`
- Modify: `backend-school/crates/school-certificates/src/services/proof.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: the exact vectors from Task 3, the public `school-crypto` APIs, `RawSessionToken::generate`, and `generate_certificate_proof`.
- Produces: the same stored/wire formats and public signatures on AES-GCM 0.11.1, HMAC 0.13.0, SHA-2 0.11.0, and Rand 0.10.2.

- [ ] **Step 1: Add the failing school dependency and ownership policy**

Add `school_auth_crypto_dependencies_follow_the_reviewed_policy` to `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn school_auth_crypto_dependencies_follow_the_reviewed_policy() {
    let manifest = read_source(manifest_dir().join("Cargo.toml"));
    assert!(manifest.contains(
        "aes-gcm = { version = \"0.11.1\", default-features = false, features = [\"aes\", \"alloc\", \"zeroize\"] }"
    ));
    assert!(manifest.contains(
        "bcrypt = { version = \"0.19.3\", default-features = false, features = [\"std\", \"zeroize\"] }"
    ));
    assert!(manifest.contains(
        "hmac = { version = \"0.13.0\", default-features = false, features = [\"zeroize\"] }"
    ));
    assert!(manifest.contains(
        "rand = { version = \"0.10.2\", default-features = false, features = [\"sys_rng\"] }"
    ));
    assert!(manifest.contains(
        "sha2 = { version = \"0.11.0\", default-features = false, features = [\"zeroize\"] }"
    ));
    assert!(!manifest.lines().any(|line| {
        line.trim_start().starts_with("sha256 =")
    }));

    let rand_consumers = backend_rs_files()
        .into_iter()
        .filter(|file| read_source(file).contains("rand::"))
        .map(|file| repo_relative(&file))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        rand_consumers,
        BTreeSet::from([
            "backend-school/crates/school-auth/src/session_crypto.rs".to_string(),
            "backend-school/crates/school-certificates/src/services/proof.rs".to_string(),
            "backend-school/crates/school-crypto/src/lib.rs".to_string(),
        ])
    );

    for relative_path in [
        "crates/school-auth/src/session_crypto.rs",
        "crates/school-certificates/src/services/proof.rs",
        "crates/school-crypto/src/lib.rs",
    ] {
        let source = read_source(manifest_dir().join(relative_path));
        assert!(source.contains("rand::rngs::SysRng"));
        assert!(source.contains(".try_fill_bytes("));
        assert!(!source.contains("rand::rng()"));
        assert!(!source.contains("rand::rngs::OsRng"));
    }
    assert!(read_source(manifest_dir().join("crates/school-auth/src/session_crypto.rs"))
        .contains("ServiceUnavailable(\"session_rng\".to_string())"));
    assert!(read_source(manifest_dir().join("crates/school-crypto/src/lib.rs"))
        .contains("Random number generation failed"));
    assert!(read_source(manifest_dir().join("crates/school-certificates/src/services/proof.rs"))
        .contains("proof_crypto_error(\"certificate proof randomness unavailable\".to_string())"));
}
```

Remove the old Rand 0.9 hold assertion and duplicate consumer set from `rust_data_transport_document_dependencies_follow_the_reviewed_policy`; retain all SQLx, Reqwest, Base64, and Lopdf assertions there.

Run:

```bash
cd backend-school
cargo test --test static_architecture school_auth_crypto_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: FAIL on the old direct version declarations and the still-present `sha256` line.

- [ ] **Step 2: Apply the exact workspace declarations and remove redundant root consumers**

Change `[workspace.dependencies]` in `backend-school/Cargo.toml` to:

```toml
aes-gcm = { version = "0.11.1", default-features = false, features = ["aes", "alloc", "zeroize"] }
bcrypt = { version = "0.19.3", default-features = false, features = ["std", "zeroize"] }
hmac = { version = "0.13.0", default-features = false, features = ["zeroize"] }
rand = { version = "0.10.2", default-features = false, features = ["sys_rng"] }
sha2 = { version = "0.11.0", default-features = false, features = ["zeroize"] }
```

Delete `sha256 = "1.6.0"`. Remove `hmac = { workspace = true }` and `rand = { workspace = true }` from the root package's runtime dependencies and remove its duplicate Rand dev-dependency; the workspace declarations and three internal-crate owners remain. Keep root bcrypt and SHA-2 because root binaries/services and academic/menu modules consume them.

- [ ] **Step 3: Migrate `school-crypto` to the new array, HMAC, and Rand APIs**

Use these imports:

```rust
use aes_gcm::{
    aead::{consts::U12, Aead},
    Aes256Gcm, Nonce,
};
use hmac::{Hmac, KeyInit, Mac};
use rand::TryRng;
use sha2::{Digest, Sha256};
```

Construct the cipher and nonce without deprecated `from_slice` or runtime `expect`:

```rust
let key_bytes = Sha256::digest(key_str.as_bytes());
let cipher = <Aes256Gcm as aes_gcm::KeyInit>::new(&key_bytes);

let nonce = Nonce::<U12>::try_from(nonce_bytes.as_slice())
    .map_err(|_| "Invalid encryption nonce".to_string())?;
```

Pass `&nonce` to both `encrypt` and `decrypt`. Create HMAC values with:

```rust
let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key.as_bytes())
    .map_err(|_| "Invalid BLIND_INDEX_KEY".to_string())?;
```

Fill the nonce only in the public random encryptor:

```rust
let mut nonce_bytes = [0_u8; 12];
rand::rngs::SysRng
    .try_fill_bytes(&mut nonce_bytes)
    .map_err(|_| "Random number generation failed".to_string())?;
encrypt_with_nonce(plaintext, nonce_bytes)
```

Do not log the upstream RNG error or nonce bytes.

- [ ] **Step 4: Migrate session HMAC and session-token randomness**

In `school-auth/src/session_crypto.rs`, import `TryRng`, use `rand::rngs::SysRng`, and replace `TryRngCore`/`OsRng`. Retain the current bounded service error:

```rust
rand::rngs::SysRng
    .try_fill_bytes(&mut bytes)
    .map_err(|_| AppError::ServiceUnavailable("session_rng".to_string()))?;
```

Import HMAC's `KeyInit` with the existing `digest::Key`, then construct the fixed 64-byte session HMAC key without a fallible slice conversion:

```rust
fn hmac(&self) -> HmacSha256 {
    let key: Key<HmacSha256> = self.0.into();
    HmacSha256::new(&key)
}
```

Keep all domains, UUID byte ordering, IPv4 marker bytes, constant-time comparisons, redacted `Debug`, token size, and Base64URL canonical parsing unchanged.

- [ ] **Step 5: Migrate certificate proof randomness**

In `school-certificates/src/services/proof.rs`, replace the Rand import with `use rand::TryRng;` and fill exactly 32 bytes through:

```rust
rand::rngs::SysRng
    .try_fill_bytes(&mut *bytes)
    .map_err(|_| proof_crypto_error("certificate proof randomness unavailable".to_string()))?;
```

`proof_crypto_error` must continue discarding its input and returning only `certificate proof cryptography failed`; do not expose RNG internals. Keep the 43-character Base64URL-no-padding proof, field encryption, domain string, and redacted debug output unchanged.

- [ ] **Step 6: Resolve the lockfile and run all changed-boundary tests**

Run once without `--locked` to update only the manifest-driven graph, then lock all later checks:

```bash
cd backend-school
cargo check -p school-crypto -p school-auth -p school-certificates
cargo test --locked -p school-crypto
cargo test --locked -p school-auth session_crypto::tests
cargo test --locked -p school-auth session_policy::tests
cargo test --locked -p school-certificates services::proof::tests -- --test-threads=8
cargo test --locked --test static_architecture school_auth_crypto_dependencies_follow_the_reviewed_policy -- --exact
```

Expected: every Task 3 vector passes unchanged and the dependency-policy test turns green.

- [ ] **Step 7: Compile and test every direct SHA-2 and bcrypt owner**

Run:

```bash
cargo test --locked -p school-file-platform file_hash::tests
cargo test --locked -p school-academic-results services::term_preview::tests::term_preview_preserves_the_pre_batch_source_checksum_contract -- --exact
cargo check --locked --all-targets \
  -p school-academic-core \
  -p school-academic-delivery \
  -p school-academic-timetable \
  -p school-academic-assessment \
  -p school-academic-results \
  -p school-academic-lifecycle \
  -p school-staff \
  -p school-students \
  -p school-admission \
  -p school-test-db
```

Expected: all pass. In particular, the exact File Platform and term-result checksum fixtures stay unchanged, and every stored bcrypt consumer still compiles against the single workspace owner.

- [ ] **Step 8: Audit duplicates and commit the school migration**

Run:

```bash
cargo tree -e features -i aes-gcm@0.11.1
cargo tree -e features -i bcrypt@0.19.3
cargo tree -e features -i hmac@0.13.0
cargo tree -e features -i sha2@0.11.0
cargo tree -e features -i rand@0.10.2
cargo tree -d
cargo fmt --all -- --check
cd ..
git diff --check
git add backend-school
git commit -m "chore: migrate school authentication cryptography dependencies"
```

Expected: every direct school owner uses the workspace declaration; older versions shown by `cargo tree -d` have only upstream inverse owners. Do not add a Cargo patch or direct compatibility dependency to collapse them.

---

### Task 5: Verify Authentication, Stored PII, Contracts, and the Complete Local Candidate

**Files:**
- Verify: all changed manifests, lockfiles, source, and tests.
- Verify unchanged: migrations, API contract, permission contract, frontend auth/session code, and deployment topology.

**Interfaces:**
- Consumes: the complete Wave 4 feature tree.
- Produces: local evidence that stored formats, auth/session behavior, database integration, contracts, and dependency ownership remain valid before integration.

- [ ] **Step 1: Run the full backend-admin matrix**

Run:

```bash
cd backend-admin
cargo fmt --all -- --check
cargo test --locked
cargo check --locked --all-targets
cd ..
```

Expected: PASS, including the fixed v9 JWT and bcrypt fixtures.

- [ ] **Step 2: Run the encryption and PII matrix**

Run:

```bash
cd backend-school
cargo test --locked -p school-crypto
cd ..
./scripts/test_backend_school.sh --package school-admission services::pii::tests -- --nocapture --test-threads=8
```

Expected: AES-GCM legacy decrypt, deterministic new encryption, blind indexes, optional fields, and admission PII behavior pass without printing plaintext identity values or keys.

- [ ] **Step 3: Run crate-owned session database tests**

Run from the repository root:

```bash
./scripts/test_backend_school.sh --package school-auth session_schema_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_repository_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_cache_tests -- --nocapture --test-threads=8
./scripts/test_backend_school.sh --package school-auth session_service_tests -- --nocapture --test-threads=8
```

Expected: login, stored bcrypt verification, password change, token hashing/rotation, cache invalidation, expiry, and concurrency behavior pass on disposable PostgreSQL.

- [ ] **Step 4: Run application-owned HTTP, profile, staff, and realtime auth tests**

Run:

```bash
./scripts/test_backend_school.sh modules::auth::session_http_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::profile_integration_tests -- --nocapture
./scripts/test_backend_school.sh modules::auth::staff_integration_tests -- --nocapture
./scripts/test_backend_school.sh modules::academic::websockets::security_tests -- --nocapture
```

Expected: cookie, CSRF, origin, session maintenance, password mutation, identity invalidation, and realtime authorization remain unchanged.

- [ ] **Step 5: Run frontend session guards and Playwright discovery**

Run:

```bash
cd frontend-school
node --test tests/static/session-auth-contract.test.mjs \
  tests/static/account-security.test.mjs \
  tests/static/auth-session-state.test.mjs \
  tests/static/realtime-idle.test.mjs \
  tests/static/auth-refresh-races.test.mjs
E2E_SESSION_USERNAME='dedicated-disposable-account' \
E2E_SESSION_PASSWORD='provided-at-runtime' \
npx playwright test --list tests/e2e/login.spec.ts tests/e2e/session-security.spec.ts
cd ..
```

Expected: static tests pass and Playwright discovers both suites. Execute them against the authorized sandbox when the dedicated disposable account is available; never substitute an operator, production, normal E2E, or `SMOKE_*` account for the destructive session-security suite.

- [ ] **Step 6: Run the complete backend-school matrix and full disposable database suite**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo test --locked --test static_architecture
cargo check --locked --workspace --all-targets
RUSTFLAGS='-D warnings' cargo check --locked --bin backend-school
cd ..
./scripts/test_backend_school.sh --locked -- --test-threads=8
```

Expected: formatting, all architecture rules, every target, deny-warnings binary build, and the full PostgreSQL-backed suite pass.

- [ ] **Step 7: Verify generated contracts without rewriting them**

Run:

```bash
cd frontend-school
npm run check:api-contracts
npm run test:api-contracts
npm run check:permissions
npm run test:permissions
cd ..
```

Expected: all contract checks pass and create no tracked changes.

- [ ] **Step 8: Prove migrations, generated artifacts, and topology are untouched**

Run:

```bash
wave4_base="$(git merge-base HEAD origin/main)"
git diff --exit-code "$wave4_base" -- backend-school/migrations backend-admin/migrations
git diff --exit-code "$wave4_base" -- \
  contracts/openapi/school-api.json \
  frontend-school/src/lib/api/generated/school-api.ts \
  contracts/permissions.json \
  contracts/permissions.lock.json \
  backend-school/crates/school-permissions/src/registry_generated.rs \
  frontend-school/src/lib/permissions/registry.generated.ts \
  podman-compose.yml nginx-configs .github/workflows
```

Expected: no output and exit 0.

- [ ] **Step 9: Perform the final diff, secret, and graph review**

Run:

```bash
git diff --check
git status --short
git diff "$(git merge-base HEAD origin/main)"...HEAD --stat
git diff "$(git merge-base HEAD origin/main)"...HEAD -- backend-school backend-admin
```

Review every feature flag, fixed vector, environment error, RNG call, HMAC constructor, nonce conversion, and lockfile change. Confirm no real credential, identity value, migration, contract, permission, SQL, or deployment change entered the diff.

---

### Task 6: Integrate, Deploy Both Backends, and Complete the Modernization Program

**Files:**
- Delete before integration: `docs/superpowers/plans/2026-09-20-rust-auth-crypto-major-migration.md`
- Delete before integration: `docs/superpowers/specs/2026-09-20-repository-dependency-modernization-design.md`

**Interfaces:**
- Consumes: the fully verified Wave 4 feature tree and accepted Wave 3 production baseline.
- Produces: one squash commit on `main`, successful backend-admin and backend-school deployments, exact-SHA production acceptance, and a completed four-wave dependency modernization program.

- [ ] **Step 1: Remove completed workflow artifacts and make the final feature commit**

After every implementation and local verification step passes:

```bash
git rm docs/superpowers/plans/2026-09-20-rust-auth-crypto-major-migration.md
git rm docs/superpowers/specs/2026-09-20-repository-dependency-modernization-design.md
git add -A
git commit -m "chore: complete rust authentication cryptography migration"
```

Do not delete either artifact earlier; they remain the reviewed source for the work until verification is complete.

- [ ] **Step 2: Recheck remote divergence and merge an advanced base on the feature branch**

Run:

```bash
git fetch origin
git status --short --branch
git rev-list --left-right --count origin/main...HEAD
```

If `origin/main` advanced, merge it into the feature branch, resolve conflicts there, and rerun every affected focused and matrix check. Stop if the tree is dirty or conflicts remain.

- [ ] **Step 3: Record the verified tree and squash into updated main**

Run:

```bash
feature_head="$(git rev-parse HEAD)"
feature_tree="$(git rev-parse HEAD^{tree})"
git switch main
git merge --ff-only origin/main
git merge --squash "$feature_head"
git commit -m "chore: modernize rust authentication cryptography dependencies"
integrated_tree="$(git rev-parse HEAD^{tree})"
test "$feature_tree" = "$integrated_tree"
```

Expected: the tree IDs match exactly. If they do not, do not push; inspect and verify the actual integrated tree.

- [ ] **Step 4: Push main normally and identify exact-SHA workflow runs**

Run:

```bash
git push origin main
release_sha="$(git rev-parse HEAD)"
gh run list --commit "$release_sha" --limit 20
```

Never force-push `main`.

- [ ] **Step 5: Require all applicable exact-SHA CI and deployments**

Watch the exact integrated SHA and require success for:

- `API Contract`
- `Permission Contract`
- `Documentation`
- `Installer Verification`
- `Deploy Backend Admin`
- `Deploy School Release`

The school workflow must select a backend-bearing scope, build the exact SHA, retain unchanged migrations, pass readiness and all-tenant migration audit, run authenticated login/session/CSRF/realtime and academic smoke, restore the proxy, and finish `accept-release`. The admin workflow must build the exact SHA, deploy, pass `/ready`, and verify service identity. Queued serialization on the shared runtime concurrency group is expected.

- [ ] **Step 6: Run bounded public acceptance checks**

Run without printing credentials or response headers that may contain tokens:

```bash
curl --fail --silent --show-error https://admin-api.schoolorbit.app/ready | jq -e \
  '.status == "ready" and .database == "connected"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/health | jq -e \
  '.status == "healthy"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/ready | jq -e \
  '.status == "ready" and .controlPlane == "connected" and .filePlatform == "ready"'
curl --fail --silent --show-error https://school-api.schoolorbit.app/deployment-status | jq .
curl --fail --silent --show-error --output /dev/null https://snwsb.schoolorbit.app/
curl --fail --silent --show-error --output /dev/null https://admin.schoolorbit.app/
```

Download `school-release-state` from the exact `Deploy School Release` run and verify both `releaseId` and `backendAcceptedSha` equal `release_sha`. Require successful deploy-backend and accept-release jobs; HTTP 200 alone is not acceptance.

- [ ] **Step 7: Close the rollback window only after exact-SHA acceptance**

Keep the feature branch and pre-Wave-4 backend images until both deployments, public readiness, authenticated school smoke, and accepted-release artifact checks pass. Do not rotate any key or force users to re-encrypt, rehash, or log in again as part of this deployment.

After acceptance, report:

- exact integrated SHA;
- all six direct target versions and explicit feature decisions;
- fixed JWT, bcrypt, AES-GCM, HMAC, SHA-256, Base64, and randomness evidence;
- every retained older transitive crypto/Rand owner from `cargo tree -i`;
- local database, frontend static/discovery, CI, deployment, authenticated smoke, and accepted-release results;
- confirmation that migrations, OpenAPI, permissions, SQL, keys, and stored formats were unchanged;
- confirmation that all four dependency-modernization waves are now accepted in production.
