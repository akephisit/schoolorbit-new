# SQLx Compile-Time Verification Issue

ปัญหาและวิธีแก้สำหรับ `sqlx::query!` macro ใน Docker builds

---

## 🐛 ปัญหา

```
error: error communicating with database: Connection refused
  --> src/bin/create_admin.rs:20:5
```

**สาเหตุ:**
- `sqlx::query!` macro ทำ **compile-time verification** กับ database
- ตอน build Docker image ไม่มี database ให้เชื่อมต่อ
- Build จึงล้มเหลว

---

## ✅ วิธีแก้

### Solution 1: Build Specific Binary (แนะนำ) ✅

**ใช้วิธีนี้แล้ว ใน Dockerfile:**

```dockerfile
# Build เฉพาะ main binary (skip binaries ที่มี query!)
RUN cargo build --release --bin backend-admin
```

**ผลลัพธ์:**
- ✅ Build main binary ที่ไม่มี `query!` ได้
- ✅ Skip `create_admin` binary (ใช้ run local เท่านั้น)
- ✅ Docker image เบาขึ้น

---

### Solution 2: ใช้ `query` แทน `query!` (Alternative)

**แก้ code ใน `create_admin.rs`:**

```rust
// ❌ Before: compile-time verification
sqlx::query!(
    r#"
    INSERT INTO admin_users (national_id, password_hash, name, role)
    VALUES ($1, $2, $3, 'super_admin')
    "#,
    national_id,
    password_hash,
    name
)

// ✅ After: runtime verification
sqlx::query(
    r#"
    INSERT INTO admin_users (national_id, password_hash, name, role)
    VALUES ($1, $2, $3, 'super_admin')
    "#,
)
.bind(national_id)
.bind(password_hash)
.bind(name)
```

**Trade-offs:**
- ✅ Build ได้เสมอ
- ❌ ไม่มี compile-time type checking
- ❌ Errors ค้นพบตอน runtime เท่านั้น

---

### Solution 3: SQLx Offline Mode (Advanced)

**ถ้าต้องการใช้ `query!` ใน Docker:**

#### 1. Generate sqlx-data.json (ครั้งเดียว)

```bash
# ต้องมี DATABASE_URL
export DATABASE_URL=postgresql://...

# Generate metadata
cargo sqlx prepare --workspace
```

**ได้ไฟล์:**
```
.sqlx/query-xxx.json
```

#### 2. Commit ไฟล์ .sqlx/

```bash
git add .sqlx/
git commit -m "chore: add sqlx offline data"
```

#### 3. Update Dockerfile

```dockerfile
# Set offline mode
ENV SQLX_OFFLINE=true

# Copy .sqlx files
COPY .sqlx ./.sqlx

# Build (ใช้ offline data)
RUN cargo build --release
```

**Trade-offs:**
- ✅ เก็บ `query!` compile-time checking ได้
- ✅ Build ใน Docker ได้
- ❌ ต้อง regenerate ทุกครั้งที่แก้ query
- ❌ ต้อง commit generated files

---

## 🎯 คำแนะนำ

### สำหรับ SchoolOrbit:

**ใช้ Solution 1:** Build specific binary ✅

**เพราะ:**
1. ✅ `create_admin` ไม่ต้องใช้ใน production
2. ✅ Run `create_admin` locally เท่านั้น (มี database อยู่แล้ว)
3. ✅ Dockerfile ง่าย ไม่ซับซ้อน
4. ✅ Build เร็ว

---

## 📝 การใช้งาน create_admin

### ใน Development (Local)

```bash
# Run create_admin locally (มี database)
cd backend-admin
cargo run --bin create_admin

# หรือ
cargo build --release --bin create_admin
./target/release/create_admin
```

### ใน Production (Docker)

```bash
# ไม่ต้อง run create_admin ใน Docker
# สร้าง admin user ผ่าน SQL script แทน

docker exec backend-admin psql $DATABASE_URL << EOF
INSERT INTO admin_users (national_id, password_hash, name, role)
VALUES ('1234567890123', '...hash...', 'Admin', 'super_admin');
EOF
```

**หรือใช้ migration:**

```sql
-- migrations/xxx_seed_admin.sql
INSERT INTO admin_users (national_id, password_hash, name, role)
VALUES ('1234567890123', 'bcrypt_hash_here', 'Default Admin', 'super_admin')
ON CONFLICT (national_id) DO NOTHING;
```

---

## 🔧 ทดสอบ Build

### Local Build (ทั้งหมด)

```bash
# Build all binaries (ต้องมี DATABASE_URL)
export DATABASE_URL=postgresql://...
cargo build --release

# สำเร็จ:
# - backend-admin
# - create_admin
```

### Docker Build (เฉพาะ main)

```bash
# Build in Docker (ไม่ต้อง DATABASE_URL)
docker build -f backend-admin/Dockerfile -t test .

# สำเร็จ:
# - backend-admin only
```

---

## 🐛 Troubleshooting

### "query! requires DATABASE_URL"

```bash
# Local development
export DATABASE_URL=postgresql://user:pass@localhost/db

# หรือใช้ .env
echo "DATABASE_URL=postgresql://..." > .env
```

### "Can't build in Docker"

```bash
# ใช้ --bin flag
RUN cargo build --release --bin backend-admin

# ไม่ใช้
RUN cargo build --release  # ❌ Build ทุก binary
```

---

## 📚 References

- SQLx Docs: https://docs.rs/sqlx/latest/sqlx/
- Offline Mode: https://docs.rs/sqlx/latest/sqlx/macro.query.html#offline-mode
- Cargo Targets: https://doc.rust-lang.org/cargo/reference/cargo-targets.html

---

**Problem solved!** ✅
