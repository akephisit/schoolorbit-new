# 🔧 Encryption Implementation Guide for Handlers

## Overview
This document provides code patterns for implementing encryption in all handlers that use `national_id` and `medical_conditions`.

---

## 🎯 Pattern 1: Setting Up Encryption Key

**Add at the start of every handler that needs encryption:**

```rust
// Set encryption key in session
if let Err(e) = crate::utils::encryption::setup_encryption_key(&pool).await {
    eprintln!("❌ Encryption setup failed: {}", e);
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "System error"
        })),
    ).into_response();
}
```

---

## 🎯 Pattern 2: INSERT with Encryption

**For creating users/staff/students:**

```rust
// OLD (before encryption):
sqlx::query(
    "INSERT INTO users (national_id, first_name, last_name) 
     VALUES ($1, $2, $3)"
)
.bind(&national_id)
.bind(&first_name)
.bind(&last_name)
.execute(&pool)
.await?;

// NEW (with encryption):
sqlx::query(
    "INSERT INTO users (national_id, first_name, last_name) 
     VALUES (
        pgp_sym_encrypt($1, current_setting('app.encryption_key')),
        $2, 
        $3
     )"
)
.bind(&national_id)
.bind(&first_name)
.bind(&last_name)
.execute(&pool)
.await?;
```

---

## 🎯 Pattern 3: SELECT with Decryption

**For reading encrypted data:**

```rust
// OLD:
sqlx::query_as::<_, User>(
    "SELECT * FROM users WHERE id = $1"
)
.bind(user_id)
.fetch_one(&pool)
.await?;

// NEW (explicit columns with decryption):
sqlx::query_as::<_, User>(
    "SELECT 
        id,
        pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
        first_name,
        last_name,
        email,
        phone,
        user_type,
        status,
        created_at,
        updated_at
     FROM users 
     WHERE id = $1"
)
.bind(user_id)
.fetch_one(&pool)
.await?;
```

---

## 🎯 Pattern 4: UPDATE with Encryption

**For updating encrypted fields:**

```rust
// OLD:
sqlx::query(
    "UPDATE users SET national_id = $1 WHERE id = $2"
)
.bind(&new_national_id)
.bind(user_id)
.execute(&pool)
.await?;

// NEW:
sqlx::query(
    "UPDATE users 
     SET national_id = pgp_sym_encrypt($1, current_setting('app.encryption_key'))
     WHERE id = $2"
)
.bind(&new_national_id)
.bind(user_id)
.execute(&pool)
.await?;
```

---

## 🎯 Pattern 5: WHERE Clause with Encrypted Column

**For searching by encrypted field:**

```rust
// OLD:
"SELECT * FROM users WHERE national_id = $1"

// NEW:
"SELECT 
    id,
    pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
    ...
 FROM users 
 WHERE pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) = $1"
```

---

## 📝 Files That Need Updates

### Priority 1: Critical (User Creation/Login)
- [x] `handlers/auth.rs` - **login()** ✅ DONE

### Priority 2: Staff Management 
- [ ] `handlers/staff.rs` - **create_staff()**
  - Line ~703: INSERT INTO users (national_id, ...)
  - Encrypt national_id before INSERT
  
- [ ] `handlers/staff.rs` - **get_staff()**
  - Line ~371: SELECT ... FROM users
  - Decrypt national_id on SELECT
  
- [ ] `handlers/staff.rs` - **update_staff()**
  - Check if updating national_id
  - Encrypt if present

### Priority 3: Student Management
- [ ] `handlers/students.rs` - **create_student()**
  - Encrypt national_id on INSERT
  
- [ ] `handlers/students.rs` - **get_student()**
  - Decrypt national_id on SELECT
  - Decrypt medical_conditions on SELECT
  
- [ ] `handlers/students.rs` - **update_student()**
  - Encrypt national_id if updating
  - Encrypt medical_conditions if updating

---

## 🔍 Quick Search Commands

Find all files needing updates:

```bash
# Find INSERT statements with national_id
grep -rn "INSERT INTO users.*national_id" backend-school/src/handlers/

# Find INSERT statements with medical_conditions  
grep -rn "INSERT INTO student_info.*medical_conditions" backend-school/src/handlers/

# Find SELECT * (should be explicit)
grep -rn "SELECT \*" backend-school/src/handlers/
```

---

## ⚡ Quick Fix Script

For rapid implementation, you can use this pattern:

```rust
// 1. Add at top of handler
use crate::utils::encryption::setup_encryption_key;

// 2. Setup encryption (after getting pool)
setup_encryption_key(&pool).await.map_err(|e| {
    eprintln!("Encryption error: {}", e);
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "System error"})))
})?;

// 3. For INSERT/UPDATE - wrap with pgp_sym_encrypt
pgp_sym_encrypt($N, current_setting('app.encryption_key'))

// 4. For SELECT - use pgp_sym_decrypt
pgp_sym_decrypt(column_name, current_setting('app.encryption_key')) as column_name
```

---

## ✅ Testing Checklist

After implementing:

- [ ] Run `cargo check` - should pass
- [ ] Test login with encrypted data
- [ ] Test create user/staff/student
- [ ] Test get user/staff/student  
- [ ] Test update user/staff/student
- [ ] Verify encrypted data in database
- [ ] Verify decrypted data in API responses

---

## 🚨 Common Pitfalls

### 1. **Forgetting to set encryption key**
```rust
// ❌ Wrong - will fail
sqlx::query("SELECT pgp_sym_decrypt(...)")

// ✅ Correct - set key first
setup_encryption_key(&pool).await?;
sqlx::query("SELECT pgp_sym_decrypt(...)")
```

### 2. **Using SELECT ***
```rust
// ❌ Wrong - won't decrypt
SELECT * FROM users

// ✅ Correct - explicit with decryption
SELECT id, pgp_sym_decrypt(national_id, ...) as national_id, ...
```

### 3. **Binding encrypted value**
```rust
// ❌ Wrong - double encryption
.bind(pgp_sym_encrypt(&value))

// ✅ Correct - let SQL handle it
// In query: pgp_sym_encrypt($1, ...)
.bind(&value)
```

---

## 📊 Estimated Time

- **handlers/auth.rs**: ✅ Done (30 min)
- **handlers/staff.rs**: 45-60 min
- **handlers/students.rs**: 45-60 min
- **Testing**: 30 min
- **Total**: ~3 hours

---

## 🎯 Next Steps

1. Update `handlers/staff.rs`:
   - create_staff()
   - get_staff()
   - update_staff()

2. Update `handlers/students.rs`:
   - create_student()
   - get_student()
   - update_student()

3. Test all endpoints

4. Commit changes
