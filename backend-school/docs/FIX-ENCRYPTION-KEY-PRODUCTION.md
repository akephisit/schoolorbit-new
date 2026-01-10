# 🔐 Encryption Key Issue - Production Fix

## ปัญหา
```
❌ unrecognized configuration parameter "app.encryption_key"
```

**สาเหตุ:** Connection pooling ทำให้ encryption key หายไประหว่าง connections

---

## ✅ **วิธีแก้ (Production)**

### **Step 1: Set Encryption Key ที่ Database Role**

รัน script นี้บน production server:

```bash
# SSH เข้า VPS
ssh user@your-vps

# เข้า Docker container
docker exec -it schoolorbit-backend-school bash

# ตั้งค่า environment variables
export ENCRYPTION_KEY="$(grep ENCRYPTION_KEY /app/.env | cut -d '=' -f2)"
export ADMIN_DATABASE_URL="$(grep ADMIN_DATABASE_URL /app/.env | cut -d '=' -f2)"
export DB_USER="school_user"  # หรือตาม .env

# รัน script
cd /app
chmod +x scripts/set_encryption_role.sh
./scripts/set_encryption_role.sh
```

**หรือ** รันจากภายนอก:

```bash
# ถ้าไม่ใช้ Docker
cd /path/to/backend-school
export ENCRYPTION_KEY="your-encryption-key"
export ADMIN_DATABASE_URL="postgres://..."
export DB_USER="school_user"
./scripts/set_encryption_role.sh
```

---

### **Step 2: Restart Backend**

```bash
# ถ้าใช้ Docker
docker restart schoolorbit-backend-school

# ถ้าใช้ systemd
sudo systemctl restart schoolorbit-backend-school
```

---

## 🔍 **ตรวจสอบว่าแก้ไขสำเร็จ**

### Test 1: ตรวจสอบ Role Setting
```sql
-- Connect to tenant database
psql "your-tenant-database-url"

-- Check role configuration
SHOW app.encryption_key;
```

ควรเห็น encryption key ที่ตั้งไว้

### Test 2: ทดสอบ Login
```bash
curl -X POST https://school-api.schoolorbit.app/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"nationalId":"1234567890123","password":"password"}'
```

ไม่ควรเห็น error `unrecognized configuration parameter` อีก

---

## 🛠️ **Alternative: Manual SQL Fix**

ถ้า script ไม่ทำงาน ให้รัน SQL นี้ในแต่ละ tenant database:

```sql
-- Replace 'school_user' with your actual DB_USER
-- Replace 'your-key' with actual ENCRYPTION_KEY
ALTER ROLE school_user SET app.encryption_key = 'your-key-here';

-- Verify
SELECT rolname, rolconfig 
FROM pg_roles 
WHERE rolname = 'school_user';
```

---

## 📊 **What This Does**

| Before | After |
|--------|-------|
| Encryption key set per connection | Encryption key set at role level |
| `after_connect` hook每次都run | Automatic for all connections |
| Random failures | ✅ Consistent |

---

## ⚠️ **Important Notes**

1. **Neon.tech Users:** 
   - Neon อาจไม่รองรับ `ALTER ROLE ... SET`
   - ถ้าใช้ Neon ต้องแก้โค้ดให้ทำ lazy initialization แทน

2. **Multiple Databases:**
   - Script จะ loop ทุก active tenant databases
   - ต้อง run เพียงครั้งเดียว

3. **After Fix:**
   - สามารถลบ `after_connect` hook ออกได้ (optional)
   - Restart backend เพื่อให้ pool ใช้ connection ใหม่

---

## 🚨 **Troubleshooting**

### Error: "permission denied to set parameter"
```bash
# ต้อง run ด้วย superuser
psql "your-database-url" -c "ALTER ROLE school_user SET app.encryption_key = 'key';"
```

### Error: "role does not exist"
```bash
# สร้าง role ก่อน
CREATE ROLE school_user LOGIN PASSWORD 'password';
```

### Neon.tech Specific
ถ้าใช้ Neon.tech อาจไม่รองรับ ให้ใช้วิธีอื่น:
- Option A: ย้ายไป dedicated PostgreSQL
- Option B: Lazy set encryption key ในทุก query
- Option C: ใช้ connection string parameter

---

## 📝 **Prevention**

เพิ่มใน `.env`:
```bash
# Ensure these are set
ENCRYPTION_KEY=your-32-char-minimum-key
DB_USER=school_user
ADMIN_DATABASE_URL=postgres://...
```

Add healthcheck:
```rust
// In pool_manager.rs or similar
async fn verify_encryption_key(pool: &PgPool) -> Result<(), Error> {
    sqlx::query("SHOW app.encryption_key")
        .fetch_one(pool)
        .await?;
    Ok(())
}
```

---

**หลังรัน script แล้ว ต้อง restart backend ด้วยนะครับ!** 🔄
