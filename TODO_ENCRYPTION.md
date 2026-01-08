# 🚨 TODO: Encryption Updates Required

## Status: ⚠️ Incomplete

### ✅ Completed:
- [x] Migration 019 (encryption setup)
- [x] Utils (encryption helpers)
- [x] Auth handler (login with decrypt)
- [x] Documentation

### ⏳ Pending (Critical):

#### handlers/staff.rs

**Line ~575 - Add after permission check in create_staff:**
```rust
// Setup encryption key for encrypted columns
if let Err(e) = crate::utils::encryption::setup_encryption_key(&pool).await {
    eprintln!("❌ Encryption setup failed: {}", e);
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "System error"})),
    ).into_response();
}
```

**Line ~610 - Update existing user check:**
```rust
// OLD:
"SELECT id, status FROM users WHERE national_id = $1"

// NEW:
"SELECT id, status FROM users 
 WHERE pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) = $1"
```

**Line ~703 - Update INSERT statement:**
```rust
// OLD:
"INSERT INTO users (
    national_id, email, password_hash, ...
) VALUES ($1, $2, $3, ...)"

// NEW:
"INSERT INTO users (
    national_id, email, password_hash, ...
) VALUES (
    pgp_sym_encrypt($1, current_setting('app.encryption_key')),
    $2, $3, ...
)"
```

**Line ~371 - Update SELECT in get_staff_profile:**
```rust
// OLD:
"SELECT id, national_id, email, ..."

// NEW:
"SELECT id, 
    pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
    email, ..."
```

**Add encryption setup at start of get_staff_profile (after permission check):**
```rust
if let Err(e) = crate::utils::encryption::setup_encryption_key(&pool).await {
    return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "System error"}))).into_response();
}
```

---

#### handlers/students.rs

Similar changes needed for:
- create_student()
- get_student()  
- update_student()

**Additional:** encrypt `medical_conditions` field

---

## Quick Implementation

### Option 1: Manual (Recommended)
1. Open file in editor
2. Search for TODO locations above
3. Apply changes manually
4. Test with `cargo check`

### Option 2: Pattern Search & Replace
```bash
# Find all national_id references
rg "national_id" backend-school/src/handlers/staff.rs -n

# For each INSERT/SELECT/WHERE:
# - Add pgp_sym_encrypt() for VALUES
# - Add pgp_sym_decrypt() for SELECT/WHERE
```

---

## Testing

After updates, verify:
```bash
# 1. Build
cargo check

# 2. Test create staff (should encrypt)
curl -X POST .../api/staff -d '{...}'

# 3. Test get staff (should decrypt)
curl .../api/staff/:id

# 4. Check database (should see encrypted BYTEA)
psql -c "SELECT national_id FROM users LIMIT 1"
# Should see: \x... (binary data)
```

---

## Estimated Time
- handlers/staff.rs: 30-45 min
- handlers/students.rs: 30-45 min
- Testing: 15-30 min
- **Total: 1.5-2 hours**

---

## Support
See complete examples in:
- `docs/ENCRYPTION_IMPLEMENTATION.md`
- `backend-school/src/handlers/auth.rs` (login function)
