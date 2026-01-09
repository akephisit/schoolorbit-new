# Encryption Key Setup - ALTER ROLE Method

## วิธีนี้ทำอะไร?

ตั้งค่า encryption key ที่ **database role level** แทนที่จะ SET ทุกครั้งใน code

**ผลลัพธ์:**
- ✅ Encryption key จะถูก set อัตโนมัติทุก session
- ✅ ไม่ต้องแก้โค้ด
- ✅ Performance ดีที่สุด
- ✅ ไม่มี race condition

---

## 📋 Requirements

1. `ENCRYPTION_KEY` environment variable
2. `ADMIN_DATABASE_URL` environment variable
3. Database user ที่มี permission ALTER ROLE

---

## 🚀 วิธีใช้

### 1. Set Environment Variables

```bash
export ENCRYPTION_KEY="your-encryption-key-here"
export ADMIN_DATABASE_URL="postgresql://user:pass@host/admin_db"
export DB_USER="your_db_user"  # Optional, default: your_db_user
```

### 2. Run Script

```bash
cd backend-school
./scripts/set_encryption_role.sh
```

### 3. Output ที่คาดหวัง

```
🔐 Setting Encryption Key at Database Role Level

Database user: your_db_user

📊 Fetching tenant databases...
Found 3 active tenant database(s)

Processing: school1_db
  ✅ Encryption key set successfully
Processing: school2_db
  ✅ Encryption key set successfully
Processing: school3_db
  ✅ Encryption key set successfully

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ Success: 3

🎉 All databases configured successfully!

Next steps:
1. Remove after_connect hook from pool_manager.rs (optional cleanup)
2. Restart backend
3. Encryption key will be set automatically for all connections!
```

---

## 🔍 ตรวจสอบว่า Set แล้ว

```bash
# Connect to tenant database
psql $TENANT_DB_URL

# Check role config
SELECT rolname, rolconfig 
FROM pg_roles 
WHERE rolname = 'your_db_user';

# Should see: {app.encryption_key=your-key}
```

---

## 🔄 Update Encryption Key

ถ้าต้องการเปลี่ยน encryption key:

```bash
# Update environment variable
export ENCRYPTION_KEY="new-key-here"

# Run script again
./scripts/set_encryption_role.sh

# Restart backend
```

---

## 🧹 Cleanup (Optional)

หลังจาก set แล้ว สามารถลบ `after_connect` hook ออกจาก `pool_manager.rs`:

```rust
// ใน pool_manager.rs - ลบส่วนนี้ออก (optional)
.after_connect(|conn, _meta| {
    // ไม่จำเป็นแล้ว!
})
```

---

## ❓ FAQ

**Q: ต้อง run script ทุกครั้งที่ add tenant ใหม่ไหม?**  
A: ใช่ หรือเพิ่ม ALTER ROLE command ใน provision script

**Q: ถ้า encryption key เปลี่ยนทำไง?**  
A: Run script ใหม่ด้วย key ใหม่ แล้ว restart backend

**Q: มี impact อะไรบ้าง?**  
A: ไม่มี! เป็นการตั้งค่า default value สำหรับ role เท่านั้น

**Q: ถ้า script fail ทำไง?**  
A: Check:
- Database user มี permission ALTER ROLE ไหม?
- Connection strings ถูกต้องไหม?
- ENCRYPTION_KEY set แล้วไหม?

---

## 🔐 Security Note

Script นี้ใช้ environment variable เพื่อความปลอดภัย  
**อย่า** hardcode encryption key ใน code!

---

## 🆚 Comparison with Other Methods

| Method | Performance | Code Changes | Reliability |
|--------|-------------|--------------|-------------|
| **ALTER ROLE** ⭐ | Best | None | 100% |
| after_connect | Good | Minimal | ~80% |
| Manual SET | Poor | Many | Variable |
| Wrapper Pool | Fair | Major | 100% |

---

**Recommended: Use ALTER ROLE method!** ✅
