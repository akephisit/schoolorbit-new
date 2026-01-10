# Refactoring Guide: Switch from pgcrypto to Application-Level Encryption

This guide shows how to refactor all 14 places that use `pgp_sym_decrypt`.

## Summary of Changes

**Before:**
```sql
pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id
```

**After:**
```rust
// 1. Query: SELECT national_id directly (encrypted)
let mut user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
    .bind(id).fetch_one(pool).await?;

// 2. Decrypt in Rust
user.national_id = field_encryption::decrypt(&user.national_id)?;
```

---

## Pattern 1: Simple SELECT by ID

**Files:** auth.rs (line 313), students.rs (line 167), staff.rs (line 144), menu.rs (line 109), feature_toggles.rs (line 503), menu_admin.rs (line 855)

**Before:**
```rust
let user: User = sqlx::query_as(
    "SELECT id, pgp_sym_decrypt(national_id, ...) as national_id, ...
     FROM users WHERE id = $1"
).bind(id).fetch_one(pool).await?;
```

**After:**
```rust
use crate::utils::field_encryption;

let mut user: User = sqlx::query_as(
    "SELECT id, national_id, ... FROM users WHERE id = $1"
).bind(id).fetch_one(pool).await?;

// Decrypt after fetch
user.national_id = field_encryption::decrypt(&user.national_id)
    .map_err(|e| format!("Decrypt failed: {}", e))?;
```

---

## Pattern 2: WHERE with national_id

**Files:** auth.rs (line 113), staff.rs (line 637)

**Before:**
```rust
sqlx::query("... WHERE pgp_sym_decrypt(national_id, ...) = $1")
    .bind(&plaintext_id).execute(pool).await?;
```

**After:**
```rust
// Encrypt before query
let encrypted_id = field_encryption::encrypt(&plaintext_id)?;

sqlx::query("... WHERE national_id = $1")
    .bind(&encrypted_id).execute(pool).await?;
```

---

## Pattern 3: Complex JOIN queries

**Files:** students.rs (lines 296+300, 804+808), staff.rs (line 397)

**Before:**
```sql
SELECT u.id, pgp_sym_decrypt(u.national_id, ...) as national_id,
       s.medical_conditions, pgp_sym_decrypt(s.medical_conditions, ...) as medical_conditions
FROM users u JOIN students s ...
```

**After:**
```rust
// Define struct to match query
#[derive(sqlx::FromRow)]
struct StudentRow {
    id: Uuid,
    national_id: String,  // Encrypted
    medical_conditions: Option<String>,  // Encrypted
    // ... other fields
}

// Query without decrypt
let mut row: StudentRow = sqlx::query_as(
    "SELECT u.id, u.national_id, s.medical_conditions, ...
     FROM users u JOIN students s ..."
).fetch_one(pool).await?;

// Decrypt after fetch
row.national_id = field_encryption::decrypt(&row.national_id)?;
if let Some(ref mc) = row.medical_conditions {
    row.medical_conditions = Some(field_encryption::decrypt(mc)?);
}
```

---

## Complete File-by-File Changes

### 1. **src/handlers/auth.rs** (3 places)

#### Line 17: Remove const SELECT_USER_BY_ID
```rust
// DELETE THIS - not needed anymore
```

#### Line 113: Login query
```rust
// Encrypt first
let encrypted_id = field_encryption::encrypt(&payload.national_id)?;

// Query
let user: LoginUser = sqlx::query_as(
    "SELECT ... FROM users WHERE national_id = $1"
).bind(&encrypted_id).fetch_one(pool).await?;
```

#### Line 313: Get profile
```rust
let mut user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
    .bind(user_id).fetch_one(pool).await?;
user.national_id = field_encryption::decrypt(&user.national_id)?;
```

---

### 2. **src/handlers/students.rs** (5 places)

All follow Pattern 1 or Pattern 3 above.

---

### 3. **src/handlers/staff.rs** (3 places)

Similar patterns.

---

### 4. **src/handlers/menu.rs** (1 place)

Pattern 1.

---

### 5. **src/handlers/feature_toggles.rs** (1 place)

Pattern 1.

---

### 6. **src/handlers/menu_admin.rs** (1 place)

Pattern 1.

---

## Helper Function (Optional)

Add to `utils/decrypt_helpers.rs`:

```rust
pub async fn fetch_user_and_decrypt(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<User, String> {
    let mut user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    
    user.national_id = field_encryption::decrypt(&user.national_id)?;
    Ok(user)
}
```

---

## Testing Checklist

After changes:
- [ ] Run migration 021
- [ ] cargo check passes
- [ ] Login works
- [ ] Profile fetch works
- [ ] Student/Staff creation works with encrypted national_id
- [ ] All 14 locations updated

---

## Estimated Time

- Migration: 5 min
- Code changes: 30-45 min (14 places)
- Testing: 15 min
- **Total: ~1 hour**

---

Next: Start with migration, then fix files one by one!
