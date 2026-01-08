# 🔐 Data Encryption Guide

## Overview

ระบบเข้ารหัสข้อมูลส่วนบุคคลที่ละเอียดอ่อนด้วย **AES-256 encryption** ผ่าน PostgreSQL pgcrypto extension เพื่อปฏิบัติตาม PDPA

## Encrypted Data

### ข้อมูลที่เข้ารหัส:
1. **`users.national_id`** - เลขบัตรประชาชน (BYTEA)
2. **`student_info.medical_conditions`** - ข้อมูลโรคประจำตัว (BYTEA)

## Setup

### 1. Generate Encryption Key

```bash
# Generate a secure random key (32 characters minimum)
openssl rand -base64 32

# Example output:
# Kv8H2J9mP4nR6sT1wU3xY5zA7bC9dE0fG2hI4jK6lM8=
```

### 2. Set Environment Variable

```bash
# Development (.env file)
ENCRYPTION_KEY="your-generated-key-here"

# Production (system env or secrets manager)
export ENCRYPTION_KEY="your-generated-key-here"
```

**⚠️ SECURITY WARNING:**
- **NEVER** commit encryption key to version control
- Store in secure secrets manager (e.g., AWS Secrets Manager, HashiCorp Vault)
- Rotate keys every 90 days
- Backup keys securely (separate from database)

### 3. Run Migrations

```bash
# Run migration to setup encryption
cd backend-school
sqlx migrate run

# The migration will:
# - Enable pgcrypto extension
# - ALTER columns to BYTEA type
# - Add encryption helper functions
```

## Usage in Application

### Set Encryption Key in Database Session

Every transaction that needs encryption must set the key:

```rust
use crate::utils::encryption;

// At start of handler/transaction
let key = encryption::get_encryption_key()?;
sqlx::query(&format!("SET LOCAL app.encryption_key = '{}'", key))
    .execute(&pool)
    .await?;

// Then proceed with encrypted operations
```

### Inserting Encrypted Data

```rust
// Insert with encryption
sqlx::query(
    "INSERT INTO users (national_id, first_name, last_name) 
     VALUES (pgp_sym_encrypt($1, current_setting('app.encryption_key')), $2, $3)"
)
.bind("1234567890123")
.bind("John")
.bind("Doe")
.execute(&pool)
.await?;
```

### Querying Encrypted Data

```rust
// Select with decryption
let result = sqlx::query_as::<_, User>(
    "SELECT 
        id,
        pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
        first_name,
        last_name
     FROM users 
     WHERE id = $1"
)
.bind(user_id)
.fetch_one(&pool)
.await?;
```

### Using Helper Functions

```rust
use crate::utils::encryption::{encrypt_sql, decrypt_sql, set_encryption_key_sql};

// Set key
let key = encryption::get_encryption_key()?;
sqlx::query(&set_encryption_key_sql(&key))
    .execute(&pool)
    .await?;

// Insert (using helper - note: for documentation, manual is safer)
let query = format!(
    "INSERT INTO users (national_id) VALUES ({})",
    encrypt_sql("value", 1)
);

// Select (using helper)
let query = format!(
    "SELECT id, {} as national_id FROM users",
    decrypt_sql("national_id")
);
```

## Key Rotation

### When to Rotate:
- Every 90 days (recommended)
- After security incident
- After team member departure
- When key may have been compromised

### How to Rotate:

```bash
# 1. Generate new key
NEW_KEY=$(openssl rand -base64 32)

# 2. Re-encrypt data with new key
psql -d schoolorbit <<EOF
  -- Decrypt with old key, encrypt with new key
  UPDATE users 
  SET national_id = pgp_sym_encrypt(
    pgp_sym_decrypt(national_id, '$OLD_KEY'),
    '$NEW_KEY'
  )
  WHERE national_id IS NOT NULL;
  
  UPDATE student_info
  SET medical_conditions = pgp_sym_encrypt(
    pgp_sym_decrypt(medical_conditions, '$OLD_KEY'),
    '$NEW_KEY'
  )
  WHERE medical_conditions IS NOT NULL;
EOF

# 3. Update environment variable
export ENCRYPTION_KEY="$NEW_KEY"

# 4. Restart application
```

## Performance Considerations

### Encryption Overhead:
- **Encryption**: ~10-20ms per operation
- **Decryption**: ~10-20ms per operation

### Optimization Tips:

1. **Cache decrypted values** in application layer (not in database)
2. **Batch operations** when possible
3. **Use connection pooling** to reuse encryption key setting
4. **Consider column-level caching** for frequently accessed data

### Index Limitations:

⚠️ **Cannot index encrypted columns directly**

For searchable encryption:
```sql
-- Create hash index for exact match searches
CREATE INDEX idx_users_national_id_hash 
ON users(encode(digest(pgp_sym_decrypt(national_id, 'key'), 'sha256'), 'hex'));
```

## Backup and Recovery

### Backup Strategy:

```bash
# 1. Backup database (includes encrypted data)
pg_dump schoolorbit > backup.sql

# 2. Backup encryption key separately
echo "$ENCRYPTION_KEY" > encryption_key.txt
# Store this in secure location (NOT with database backup)
```

### Recovery:

```bash
# 1. Restore database
psql schoolorbit < backup.sql

# 2. Restore encryption key
export ENCRYPTION_KEY=$(cat encryption_key.txt)

# 3. Verify decryption works
psql -d schoolorbit -c "
  SELECT pgp_sym_decrypt(national_id, '$ENCRYPTION_KEY') 
  FROM users 
  LIMIT 1;
"
```

## Troubleshooting

### Error: "ENCRYPTION_KEY not set"
```bash
export ENCRYPTION_KEY="your-key"
```

### Error: "wrong key or corrupt data"
- Encryption key is incorrect
- Data was encrypted with different key
- Check key in environment matches the one used for encryption

### Error: "function pgp_sym_encrypt does not exist"
```sql
-- Enable pgcrypto extension
CREATE EXTENSION IF NOT EXISTS pgcrypto;
```

## Security Checklist

- [ ] Encryption key is at least 32 characters
- [ ] Key is stored in secrets manager (not .env committed to git)
- [ ] Key is backed up securely (separate from database)
- [ ] Key rotation schedule is defined (recommend 90 days)
- [ ] Application handles decryption errors gracefully
- [ ] Audit logging records access to encrypted data
- [ ] Backups include both encrypted data AND encryption key
- [ ] Team members with key access are documented
- [ ] Incident response plan includes key compromise scenario

## References

- [PostgreSQL pgcrypto Documentation](https://www.postgresql.org/docs/current/pgcrypto.html)
- [PDPA Thailand - Data Security](https://www.pdpc.or.th/)
- [NIST Encryption Standards](https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines)
