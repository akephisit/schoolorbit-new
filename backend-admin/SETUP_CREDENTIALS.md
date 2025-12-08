# Setting Up Multi-Database Auto-Deployment

คู่มือการตั้งค่า API credentials สำหรับระบบ auto-deployment

---

## 🔑 Required Credentials

ต้องมี credentials 2 ชุด:
1. **Neon PostgreSQL API** - สำหรับสร้าง database
2. **Cloudflare API** - สำหรับ deploy Workers

---

## 📋 Step-by-Step Setup

### 1. Neon PostgreSQL Setup

#### 1.1 Create Neon Account
1. ไปที่ https://neon.tech
2. Sign up (ฟรี)
3. Create new project: `SchoolOrbit`

#### 1.2 Get API Key
1. ไปที่ https://console.neon.tech/app/settings/api-keys
2. Click "Generate new API key"
3. Copy API key (เก็บไว้ใน .env)

#### 1.3 Get Project Info
1. เปิด project `SchoolOrbit`
2. Settings → General
3. Copy:
   - **Project ID** (ตัวอย่าง: `bright-wave-12345`)
   - **Endpoint** (ตัวอย่าง: `ep-abc-xyz.us-east-2.aws.neon.tech`)

#### 1.4 Get Connection Info
1. Dashboard → Connection Details
2. Copy:
   - **User**: `neondb_owner`
   - **Password**: (สร้าง password ใหม่ถ้าต้องการ)
   - **Host**: เหมือน Endpoint ข้างบน

---

### 2. Cloudflare Setup

#### 2.1 Create Cloudflare Account
1. ไปที่ https://cloudflare.com
2. Sign up
3. Add domain: `schoolorbit.app`

#### 2.2 Create API Token
1. ไปที่ https://dash.cloudflare.com/profile/api-tokens
2. Click "Create Token"
3. Use template: "Edit Cloudflare Workers"
4. **Permissions ที่ต้องการ:**
   - Account → Workers Scripts → Edit
   - Zone → DNS → Edit
   - Zone → Zone → Read
5. **Zone Resources:**
   - Include → Specific zone → schoolorbit.app
6. Click "Continue to summary" → "Create Token"
7. **COPY TOKEN** (จะแสดงครั้งเดียว!)

#### 2.3 Get Account ID
1. ไปที่ https://dash.cloudflare.com
2. Click "Workers & Pages"
3. ขวามือจะมี **Account ID** → Copy

#### 2.4 Get Zone ID
1. ไปที่ Websites → schoolorbit.app
2. Scroll ลงด้านล่าง → API section
3. Copy **Zone ID**

---

## 🔧 Configure Backend

### Edit `.env` file

```bash
# Neon PostgreSQL (admin database)
DATABASE_URL=postgresql://neondb_owner:YOUR_PASSWORD@ep-abc-xyz.us-east-2.aws.neon.tech/schoolorbit_admin?sslmode=require

# JWT
JWT_SECRET=your-super-secret-key-change-in-production

# Neon API (for creating school databases)
NEON_API_KEY=neon_api_1a2b3c4d5e6f...
NEON_PROJECT_ID=bright-wave-12345
NEON_HOST=ep-abc-xyz.us-east-2.aws.neon.tech
NEON_USER=neondb_owner
NEON_PASSWORD=YOUR_PASSWORD

# Cloudflare API (for Workers deployment)
CLOUDFLARE_API_TOKEN=your_cloudflare_token_here
CLOUDFLARE_ACCOUNT_ID=a1b2c3d4e5f6...
CLOUDFLARE_ZONE_ID=z1y2x3w4v5u6...
```

---

## ✅ Verify Setup

### Test Neon Connection

```bash
# Test admin database connection
psql "postgresql://neondb_owner:PASSWORD@ep-xyz.aws.neon.tech/schoolorbit_admin?sslmode=require"
```

### Test Neon API

```bash
curl -X GET https://console.neon.tech/api/v2/projects/YOUR_PROJECT_ID \
  -H "Authorization: Bearer YOUR_API_KEY"
```

ควรได้ response:
```json
{
  "project": {
    "id": "bright-wave-12345",
    "name": "SchoolOrbit",
    ...
  }
}
```

### Test Cloudflare API

```bash
curl -X GET "https://api.cloudflare.com/client/v4/user/tokens/verify" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

ควรได้:
```json
{
  "success": true,
  "result": {
    "status": "active"
  }
}
```

---

## 🚀 Test Auto-Deployment

### 1. Start Backend

```bash
cd backend-admin
cargo run --release
```

### 2. Create Test School

```bash
# Login first
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "nationalId": "1234567890123",
    "password": "test123"
  }' \
  -c cookies.txt

# Create school (auto-deploy!)
curl -X POST http://localhost:8080/api/v1/schools \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "name": "โรงเรียนทดสอบ",
    "subdomain": "test-school",
    "adminNationalId": "9876543210987",
    "adminPassword": "test123"
  }'
```

### 3. Monitor Deployment

ดู backend logs:
```
🚀 Starting deployment for school: โรงเรียนทดสอบ
  📊 Creating database...
  🔧 Running migrations...
  💾 Updating school record...
  ☁️  Deploying Cloudflare Worker...
  🌐 Creating DNS record...
  🛣️  Creating Workers route...
✅ Deployment completed for โรงเรียนทดสอบ
   URL: https://test-school.schoolorbit.app
```

### 4. Verify Deployment

```bash
# Check database created
psql "postgresql://neondb_owner:PASSWORD@NEON_HOST/schoolorbit_test_school?sslmode=require"

# Check Worker deployed
curl https://test-school.schoolorbit.app
```

---

## 🔒 Security Best Practices

### 1. Environment Variables
- ❌ Never commit `.env` to git
- ✅ Use `.env.example` as template
- ✅ Use secrets manager in production

### 2. API Tokens
- ✅ Create separate tokens for dev/production
- ✅ Set token expiration
- ✅ Use minimum required permissions
- ✅ Rotate tokens regularly

### 3. Database Credentials
- ✅ Use strong passwords
- ✅ Enable SSL mode (sslmode=require)
- ✅ Restrict IP access if possible

---

## 📊 Monitoring & Limits

### Neon Limits (Free Tier)
- ✅ 10 projects
- ✅ 3 GB storage per project
- ✅ Unlimited databases per project

**Note:** แต่ละโรงเรียน = 1 database ใน project เดียวกัน

### Cloudflare Limits (Free Tier)
- ✅ 100,000 requests/day per Worker
- ✅ Unlimited Workers

---

## 🐛 Troubleshooting

### "NEON_API_KEY not set"
```bash
# Check .env file exists
ls -la backend-admin/.env

# Check env loaded
cargo run 2>&1 | grep NEON
```

### "Database creation failed: 403"
- API key หมดอายุ
- Project quota เต็ม
- Check permissions

### "Cloudflare API error: 10000"
- Token invalid
- Permissions ไม่ครบ
- Re-create token

### "DNS creation failed"
- Subdomain มีอยู่แล้ว
- Zone ID ผิด
- Token ไม่มี DNS edit permission

---

## 📚 API Documentation

### Neon API
- Docs: https://neon.tech/docs/reference/api-reference
- Endpoint: `https://console.neon.tech/api/v2`

### Cloudflare API
- Docs: https://developers.cloudflare.com/api
- Endpoint: `https://api.cloudflare.com/client/v4`

---

## ✅ Setup Checklist

- [ ] Neon account created
- [ ] Neon API key generated
- [ ] Project ID copied
- [ ] Connection details copied
- [ ] Cloudflare account created
- [ ] Domain added to Cloudflare
- [ ] API token created with correct permissions
- [ ] Account ID copied
- [ ] Zone ID copied
- [ ] `.env` file configured
- [ ] Neon connection tested
- [ ] Cloudflare API tested
- [ ] Test school created successfully

---

**🎉 Ready to auto-deploy schools!**
