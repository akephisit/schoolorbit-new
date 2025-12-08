# Neon API Setup Guide

คำแนะนำทีละขั้นตอนสำหรับการหา Neon PostgreSQL API credentials

---

## 🎯 Neon คืออะไร?

**Neon** เป็น Serverless PostgreSQL database ที่รองรับ:
- ✅ Auto-scaling
- ✅ Branching (like Git)
- ✅ Pay-per-use
- ✅ API สำหรับสร้าง database programmatically

**Website:** https://neon.tech

---

## 📋 สิ่งที่ต้องการ

จาก Neon คุณต้องเอา:
1. **NEON_API_KEY** - API key สำหรับเรียก API
2. **NEON_PROJECT_ID** - ID ของ project
3. **NEON_HOST** - Database host endpoint
4. **NEON_USER** - Database username
5. **NEON_PASSWORD** - Database password

---

## 🚀 Step-by-Step Guide

### Step 1: สร้าง Account

1. ไป https://neon.tech
2. คลิก **Sign up** (ใช้ GitHub account ได้)
3. Verify email
4. Login

---

### Step 2: สร้าง Project

1. Dashboard → **Create a project**
2. ตั้งค่า:
   - **Project name**: `schoolorbit` (หรือชื่ออื่น)
   - **Region**: เลือกใกล้ที่สุด (เช่น `aws-ap-southeast-1` สำหรับ Singapore)
   - **Postgres version**: `16` (latest)
3. คลิก **Create project**

---

### Step 3: หา NEON_PROJECT_ID

**วิธีที่ 1: จาก URL**
```
https://console.neon.tech/app/projects/crimson-frost-12345678
                                        ^^^^^^^^^^^^^^^^^^^
                                        นี่คือ PROJECT_ID
```

**วิธีที่ 2: จาก Dashboard**
1. Project settings (⚙️)
2. **General** tab
3. เห็น **Project ID**: `crimson-frost-12345678`

**Copy เก็บไว้:**
```
NEON_PROJECT_ID=crimson-frost-12345678
```

---

### Step 4: หา NEON_API_KEY

1. Profile icon (มุมขวาบน) → **Account settings**
2. เมนูซ้าย → **API keys**
3. คลิก **Create new API key**
4. ตั้งชื่อ: `schoolorbit-backend`
5. คลิก **Create**
6. **Copy API key ทันที** (จะไม่แสดงอีก!)

**Copy เก็บไว้:**
```
NEON_API_KEY=neon_api_ABCxyz123...
```

---

### Step 5: หา NEON_HOST, USER, PASSWORD

**หลังจากสร้าง project:**

1. Dashboard → Project → **Quickstart**
2. เห็น **Connection string**:

```bash
postgresql://alex:AbC123xYz@ep-cool-darkness-123456.us-east-2.aws.neon.tech/neondb?sslmode=require
```

**แยกข้อมูล:**
- **Host**: `ep-cool-darkness-123456.us-east-2.aws.neon.tech`
- **User**: `alex` (หรือ `neondb_owner`)
- **Password**: `AbC123xYz`
- **Database**: `neondb`

**Copy เก็บไว้:**
```
NEON_HOST=ep-cool-darkness-123456.us-east-2.aws.neon.tech
NEON_USER=alex
NEON_PASSWORD=AbC123xYz
```

---

## 📝 ใส่ Environment Variables

### backend-school/.env

```bash
# Neon PostgreSQL API (for creating school databases)
NEON_API_KEY=neon_api_ABCxyz123...
NEON_PROJECT_ID=crimson-frost-12345678
NEON_HOST=ep-cool-darkness-123456.us-east-2.aws.neon.tech
NEON_USER=alex
NEON_PASSWORD=AbC123xYz
```

### Portainer Stack Environment

ใน Portainer → Stacks → backend-school → Environment:

```
NEON_API_KEY=neon_api_ABCxyz123...
NEON_PROJECT_ID=crimson-frost-12345678
NEON_HOST=ep-cool-darkness-123456.us-east-2.aws.neon.tech
NEON_USER=alex
NEON_PASSWORD=AbC123xYz
```

---

## ✅ ทดสอบ API

### 1. ทดสอบ API Key

```bash
curl -X GET \
  'https://console.neon.tech/api/v2/projects' \
  -H "Authorization: Bearer YOUR_API_KEY"
```

**Expected response:**
```json
{
  "projects": [
    {
      "id": "crimson-frost-12345678",
      "name": "schoolorbit",
      ...
    }
  ]
}
```

### 2. ทดสอบ Database Connection

```bash
psql "postgresql://alex:AbC123xYz@ep-cool-darkness-123456.us-east-2.aws.neon.tech/neondb?sslmode=require"
```

**Expected:**
```
psql (16.x)
SSL connection (protocol: TLSv1.3, ...)
Type "help" for help.

neondb=>
```

---

## 🔒 Security Best Practices

### 1. Protect API Keys

❌ **อย่าทำ:**
```bash
# ❌ Commit to Git
git add .env
git commit -m "add credentials"

# ❌ Share publicly
echo "NEON_API_KEY=xxx" >> README.md
```

✅ **ทำ:**
```bash
# ✅ ใช้ .env และ .gitignore
echo "NEON_API_KEY=xxx" >> .env
echo ".env" >> .gitignore

# ✅ ใช้ secrets ใน production
# Portainer, GitHub Secrets, etc.
```

### 2. Rotate Keys

- **Rotate API keys** ทุก 90 วัน
- **Rotate passwords** ทุก 6 เดือน
- **Revoke** old keys ทันที

### 3. Use Read-Only Keys

- ถ้าเป็นไปได้ ใช้ read-only API keys
- แยก permissions ระหว่าง development/production

---

## 📊 Neon Limits

### Free Tier (Hobby)

- ✅ 1 project
- ✅ 10 branches per project
- ✅ 3 GB storage
- ✅ Shared compute
- ⏱️ Auto-suspend after 5 mins inactive

### Paid Tier (Scale)

- Starting at $19/month
- Unlimited projects
- More compute
- No auto-suspend

**ดู pricing:** https://neon.tech/pricing

---

## 🐛 Troubleshooting

### "Invalid API key"

```bash
# Check API key format
echo $NEON_API_KEY | grep "neon_api_"

# Re-create API key
# Account settings → API keys → Create new
```

### "Project not found"

```bash
# Check project ID
curl -H "Authorization: Bearer $NEON_API_KEY" \
  https://console.neon.tech/api/v2/projects

# Find your project ID in response
```

### "Connection timeout"

```bash
# Check host endpoint
nslookup ep-cool-darkness-123456.us-east-2.aws.neon.tech

# Check firewall
# Neon requires outbound HTTPS (443) and PostgreSQL (5432)
```

---

## 📚 Additional Resources

### Neon API Documentation

- **API Docs**: https://api-docs.neon.tech/reference/getting-started-with-neon-api
- **SDKs**: https://neon.tech/docs/reference/sdk
- **Examples**: https://github.com/neondatabase/examples

### Support

- **Discord**: https://discord.gg/neon
- **GitHub**: https://github.com/neondatabase/neon
- **Docs**: https://neon.tech/docs

---

## 🎯 Quick Reference

```bash
# Environment variables needed:
NEON_API_KEY=neon_api_...          # From Account Settings → API Keys
NEON_PROJECT_ID=crimson-frost-...  # From Project Settings
NEON_HOST=ep-xxx.aws.neon.tech     # From Connection String
NEON_USER=username                  # From Connection String
NEON_PASSWORD=password              # From Connection String

# API endpoint:
https://console.neon.tech/api/v2/

# Test:
curl https://console.neon.tech/api/v2/projects \
  -H "Authorization: Bearer $NEON_API_KEY"
```

---

**Neon API พร้อมใช้งาน!** 🚀
