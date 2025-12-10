# Troubleshooting Container Restart Loop

คำแนะนำสำหรับแก้ปัญหา container restarting

---

## 🔍 ตรวจสอบ Logs

### 1. ดู Logs ของ Container

```bash
# backend-admin
docker logs schoolorbit-backend-admin --tail 100

# backend-school
docker logs schoolorbit-backend-school --tail 100

# หรือดู real-time
docker logs -f schoolorbit-backend-admin
```

**มองหา:**
- Error messages
- Panic messages
- "thread 'main' panicked at"
- Connection errors
- Environment variable errors

---

## 🐛 สาเหตุที่พบบ่อย

### 1. Missing Environment Variables ❌

**Symptoms:**
```
thread 'main' panicked at 'DATABASE_URL not set'
Error: "JWT_SECRET not set"
```

**แก้:**

ใน Portainer → Stacks → Select stack → Environment variables:

**backend-admin ต้องมี:**
```env
DATABASE_URL=postgresql://user:pass@host/schoolorbit_admin
JWT_SECRET=your-secret-key
CLOUDFLARE_API_TOKEN=xxx
CLOUDFLARE_ACCOUNT_ID=xxx
CLOUDFLARE_ZONE_ID=xxx
```

**backend-school ต้องมี:**
```env
NEON_API_KEY=neon_api_xxx
NEON_PROJECT_ID=xxx
NEON_HOST=ep-xxx.aws.neon.tech
NEON_USER=xxx
NEON_PASSWORD=xxx
```

---

### 2. Database Connection Failed ❌

**Symptoms:**
```
Failed to connect to database
Connection refused (os error 111)
could not translate host name to address
```

**แก้:**

#### ตรวจสอบ DATABASE_URL format:

```bash
# ✅ ถูกต้อง
postgresql://user:password@host:5432/database

# ❌ ผิด (missing port, wrong host)
postgresql://user:password@localhost/database
```

#### ถ้าใช้ Portainer PostgreSQL stack:

```env
# ใช้ service name (ใน network เดียวกัน)
DATABASE_URL=postgresql://admin_user:password@postgres:5432/schoolorbit_admin
```

#### ทดสอบ connection จาก container:

```bash
# เข้า container
docker exec -it schoolorbit-backend-admin sh

# ทดสอบ ping database
ping postgres

# ทดสอบ connect
apk add postgresql-client
psql "$DATABASE_URL"
```

---

### 3. Network Issues ❌

**Symptoms:**
```
Could not connect to backend-school
Name or service not known
```

**แก้:**

ตรวจสอบว่า containers อยู่ใน network เดียวกัน:

```bash
# ดู network
docker network inspect stack_web

# ควรเห็น:
# - schoolorbit-backend-admin
# - schoolorbit-backend-school
# (และ services อื่นๆ)
```

**ถ้าไม่อยู่ network เดียวกัน:**

1. Portainer → Stacks → backend-admin
2. Edit docker-compose.yml
3. ตรวจสอบ:
```yaml
networks:
  web_network:
    external: true
    name: stack_web  # ต้องตรงกับ network ที่มีอยู่
```

---

### 4. Port Already in Use ❌

**Symptoms:**
```
Error: Address already in use (os error 98)
bind: address already in use
```

**แก้:**

```bash
# ดูว่า port ถูกใช้โดยอะไร
sudo netstat -tlnp | grep :8080
sudo netstat -tlnp | grep :8081

# หรือเปลี่ยน port ใน docker-compose.yml
ports:
  - "8082:8080"  # map host:8082 → container:8080
```

---

### 5. Missing Dependencies in Image ❌

**Symptoms:**
```
/app/target/release/backend-admin: not found
sh: backend-admin: not found
```

**แก้:**

ตรวจสอบว่า image build สำเร็จ:

```bash
# Pull image
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest

# ตรวจสอบ binary อยู่ไหม
docker run --rm ghcr.io/akephisit/schoolorbit-backend-admin:latest ls -la /app/target/release/

# ควรเห็น backend-admin
```

---

## 🔧 Debug Steps

### Step 1: ดู Logs

```bash
# ดู 100 บรรทัดล่าสุด
docker logs schoolorbit-backend-admin --tail 100 > admin_logs.txt
docker logs schoolorbit-backend-school --tail 100 > school_logs.txt

# อ่านไฟล์
cat admin_logs.txt
cat school_logs.txt
```

### Step 2: Run Interactive

```bash
# ลอง run แบบ interactive (ไม่ restart)
docker run --rm -it \
  -e DATABASE_URL=postgresql://... \
  -e JWT_SECRET=test \
  ghcr.io/akephisit/schoolorbit-backend-admin:latest

# ดู error ตรงๆ
```

### Step 3: Check Environment

```bash
# ดู env vars ที่ set ไว้
docker exec schoolorbit-backend-admin env | grep -E "DATABASE|JWT|CLOUDFLARE"
```

### Step 4: Check Network

```bash
# ดู network connectivity
docker exec schoolorbit-backend-admin ping backend-school
docker exec schoolorbit-backend-admin ping postgres
```

---

## 📋 Checklist

ตรวจสอบทีละข้อ:

### backend-admin:
- [ ] Environment variables ครบ (DATABASE_URL, JWT_SECRET, etc.)
- [ ] DATABASE_URL ถูกต้อง และ connect ได้
- [ ] อยู่ใน network stack_web
- [ ] Port 8080 ไม่ซ้ำ
- [ ] Image build สำเร็จ
- [ ] Logs ไม่มี panic/error

### backend-school:
- [ ] Environment variables ครบ (NEON_API_KEY, etc.)
- [ ] อยู่ใน network stack_web
- [ ] Port 8081 ไม่ซ้ำ
- [ ] Image build สำเร็จ
- [ ] Logs ไม่มี panic/error

---

## 🎯 Common Fixes

### Fix 1: Add Missing ENV

Portainer → Stack → Environment:

```env
# เพิ่มตัวนี้ถ้ายังไม่มี
DATABASE_URL=postgresql://admin_user:password@postgres:5432/schoolorbit_admin
JWT_SECRET=change-this-secret-key
CLOUDFLARE_API_TOKEN=your_token
CLOUDFLARE_ACCOUNT_ID=your_account_id
CLOUDFLARE_ZONE_ID=your_zone_id
BACKEND_SCHOOL_URL=http://backend-school:8081
```

### Fix 2: Fix Network

```yaml
# ใน docker-compose.yml ของทั้ง 2 services
networks:
  web_network:
    external: true
    name: stack_web
```

### Fix 3: Restart with Clean State

```bash
# Stop all
docker stop schoolorbit-backend-admin schoolorbit-backend-school

# Remove containers
docker rm schoolorbit-backend-admin schoolorbit-backend-school

# Pull latest images
docker pull ghcr.io/akephisit/schoolorbit-backend-admin:latest
docker pull ghcr.io/akephisit/schoolorbit-backend-school:latest

# Start again (via Portainer)
```

---

## 📞 Need More Help?

**ส่ง logs มาให้ดู:**

```bash
# Copy logs
docker logs schoolorbit-backend-admin --tail 50 > admin_error.log
docker logs schoolorbit-backend-school --tail 50 > school_error.log

# หา error line
grep -i "error\|panic\|failed" admin_error.log
grep -i "error\|panic\|failed" school_error.log
```

**ข้อมูลที่ต้องการ:**
1. Logs ทั้งหมด (50-100 บรรทัดล่าสุด)
2. Environment variables ที่ set (ซ่อน password)
3. Docker network inspect output
4. Docker compose file

---

**Run คำสั่งนี้แล้วส่งผล:**

```bash
# Quick diagnostic
echo "=== Backend Admin Logs ===" && \
docker logs schoolorbit-backend-admin --tail 30 && \
echo -e "\n=== Backend School Logs ===" && \
docker logs schoolorbit-backend-school --tail 30 && \
echo -e "\n=== Network Info ===" && \
docker network inspect stack_web | grep -A 5 "schoolorbit"
```
