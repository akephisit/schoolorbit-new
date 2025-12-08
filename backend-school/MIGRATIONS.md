# Backend-School Migration System

Backend-school จัดการ database schema เอง ผ่าน SQLx migrations

---

## 🎯 Philosophy

- **Decoupled**: แยกจาก backend-admin
- **Version Controlled**: Migration history ชัดเจน
- **Auto-migrate**: Deploy ใหม่ = schema ใหม่อัตโนมัติ
- **Safe**: Rollback ได้ถ้ามีปัญหา

---

## 📁 Structure

```
backend-school/
├── migrations/
│   ├── 20250101000000_initial_schema.sql      # Initial tables
│   ├── 20250115000000_add_attendance.sql      # Future: Attendance feature
│   └── 20250201000000_add_grades.sql          # Future: Grades feature
├── src/
│   ├── main.rs                                # Auto-run migrations here
│   └── ...
└── Cargo.toml
```

---

## 🚀 How It Works

### 1. backend-admin Creates Database

เมื่อสร้างโรงเรียนใหม่:
```
backend-admin:
  → สร้าง Database (minimal)
  → เพิ่ม UUID extension
  → สร้าง _sqlx_migrations table
  → Deploy Worker
```

### 2. backend-school Runs Migrations

เมื่อ Worker start ครั้งแรก:
```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Connect to school database
    let pool = PgPoolOptions::new()
        .connect(&env::var("DATABASE_URL")?)
        .await?;
    
    // Auto-run all pending migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;
    
    println!("✅ Database up-to-date");
    
    // Start HTTP server...
    Ok(())
}
```

---

## 📝 Creating Migrations

### Using SQLx CLI

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Create new migration
cd backend-school
sqlx migrate add create_students

# This creates:
# migrations/20250108123456_create_students.sql
```

### Migration File Format

```sql
-- Add migration script here

CREATE TABLE students (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    ...
);

-- Indexes
CREATE INDEX idx_students_name ON students(name);
```

---

## 🔄 Migration Workflow

### Development

```bash
# 1. Create migration
sqlx migrate add feature_name

# 2. Write SQL
vim migrations/TIMESTAMP_feature_name.sql

# 3. Test locally
sqlx migrate run

# 4. Commit to git
git add migrations/
git commit -m "feat: add feature_name"

# 5. Deploy
# → Auto-runs on startup
```

### Production Deploy

```bash
# Push to main
git push origin main

# Cloudflare Workers auto-deploys
# → Pulls latest code
# → Runs pending migrations
# → Starts server
```

---

## ✅ Migration Safety

### Backwards Compatible Migrations

✅ **Safe:**
```sql
-- Add new column (with default)
ALTER TABLE students ADD COLUMN email VARCHAR(255) DEFAULT '';

-- Add new table
CREATE TABLE attendance (...);

-- Add index
CREATE INDEX idx_name ON table(column);
```

❌ **Unsafe (require downtime):**
```sql
-- Drop column (data loss)
ALTER TABLE students DROP COLUMN email;

-- Rename column (breaks old code)
ALTER TABLE students RENAME COLUMN name TO full_name;

-- Change column type
ALTER TABLE students ALTER COLUMN age TYPE VARCHAR;
```

### Best Practices

1. **Always add migrations, never edit**
   ```bash
   ❌ Edit: migrations/001_old.sql
   ✅ Create: migrations/002_fix.sql
   ```

2. **Test on staging first**
   ```bash
   # Staging database
   DATABASE_URL=staging sqlx migrate run
   
   # Production database
   DATABASE_URL=prod sqlx migrate run
   ```

3. **Add rollback plans**
   ```sql
   -- UP migration
   ALTER TABLE students ADD COLUMN email VARCHAR(255);
   
   -- Document DOWN (in comments)
   -- ALTER TABLE students DROP COLUMN email;
   ```

---

## 🗂️ Current Schema (v1)

### Tables

- **admin_users** - School administrators
- **students** - Student records
- **teachers** - Teacher records
- **classes** - Class/Room information  
- **attendance** - Daily attendance tracking
- **grades** - Student grades/scores
- **announcements** - School announcements

### Views

- **active_students**
- **active_teachers**
- **active_classes**

---

## 🔍 Checking Migration Status

### View Applied Migrations

```sql
SELECT * FROM _sqlx_migrations ORDER BY version;
```

Output:
```
version | description        | installed_on | success
--------|-------------------|--------------|--------
1       | initial schema    | 2025-01-01   | true
2       | add attendance    | 2025-01-15   | true
```

### Check Pending Migrations

```bash
sqlx migrate info
```

---

## 🐛 Troubleshooting

### "Migration already applied"

```bash
# Check status
sqlx migrate info

# Force revert (DANGEROUS!)
sqlx migrate revert
```

### "Database connection failed"

```bash
# Check DATABASE_URL
echo $DATABASE_URL

# Test connection
psql $DATABASE_URL
```

### "Migration failed mid-way"

```sql
-- Manually check and fix
SELECT * FROM _sqlx_migrations WHERE success = false;

-- Remove failed migration
DELETE FROM _sqlx_migrations WHERE version = X;

-- Re-run
sqlx migrate run
```

---

## 📚 SQLx Resources

- Docs: https://docs.rs/sqlx/latest/sqlx/
- Migrations: https://docs.rs/sqlx/latest/sqlx/macro.migrate.html
- CLI: https://github.com/launchbadge/sqlx/tree/main/sqlx-cli

---

## ✅ Checklist

Deployment checklist:
- [ ] Migrations tested locally
- [ ] Migrations tested on staging
- [ ] Backwards compatible
- [ ] Rolled back on error
- [ ] Schema documented
- [ ] Indexes added where needed
- [ ] Foreign keys have ON DELETE
- [ ] Default values set

---

**🎉 Migrations handled by backend-school!**
