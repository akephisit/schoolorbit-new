# Cloudflare API Setup Guide

คำแนะนำทีละขั้นตอนสำหรับการหา Cloudflare API credentials

---

## 🎯 Cloudflare คืออะไร?

**Cloudflare** เป็น CDN และ Edge Platform ที่รองรับ:
- ✅ Cloudflare Workers (Serverless functions)
- ✅ DNS Management
- ✅ Custom domains
- ✅ SSL/TLS certificates
- ✅ DDoS protection

**Website:** https://cloudflare.com

---

## 📋 สิ่งที่ต้องการ

จาก Cloudflare คุณต้องเอา 3 ค่า:
1. **CLOUDFLARE_API_TOKEN** - API token สำหรับเรียก API
2. **CLOUDFLARE_ACCOUNT_ID** - ID ของ account
3. **CLOUDFLARE_ZONE_ID** - ID ของ domain (zone)

---

## 🚀 Step-by-Step Guide

### Step 1: สร้าง Account และเพิ่ม Domain

1. ไป https://cloudflare.com
2. คลิก **Sign up** (ใช้ email)
3. Verify email และ login
4. **Add a site**:
   - Domain name: `schoolorbit.app` (domain ของคุณ)
   - Plan: **Free** (เลือก Free plan)
5. Cloudflare จะ scan DNS records
6. **Update nameservers** ที่ domain registrar:
   ```
   ns1.cloudflare.com
   ns2.cloudflare.com
   ```
7. รอ DNS propagate (~5-30 นาที)

---

### Step 2: หา CLOUDFLARE_ZONE_ID

**หลังจาก add domain แล้ว:**

1. Dashboard → เลือก domain (`schoolorbit.app`)
2. Scroll down → ด้านขวามือ → **API** section
3. เห็น **Zone ID**: `abc123def456...`

**Copy เก็บไว้:**
```
CLOUDFLARE_ZONE_ID=abc123def456...
```

**หรือดูจาก URL:**
```
https://dash.cloudflare.com/abc123.../zones/def456.../dns
                                            ^^^^^^^^
                                            Zone ID
```

---

### Step 3: หา CLOUDFLARE_ACCOUNT_ID

**วิธีที่ 1: จาก Dashboard**

1. Dashboard → Click โปรไฟล์ (มุมขวาบน)
2. เลือกใดๆ domain
3. ดู URL:
   ```
   https://dash.cloudflare.com/1234567890abcdef/...
                                ^^^^^^^^^^^^^^^^
                                Account ID
   ```

**วิธีที่ 2: จาก Workers & Pages**

1. Dashboard → **Workers & Pages**
2. URL จะเป็น:
   ```
   https://dash.cloudflare.com/1234567890abcdef/workers
                                ^^^^^^^^^^^^^^^^
                                Account ID
   ```

**Copy เก็บไว้:**
```
CLOUDFLARE_ACCOUNT_ID=1234567890abcdef
```

---

### Step 4: สร้าง CLOUDFLARE_API_TOKEN

**Important: ต้องสร้าง Custom API Token (ไม่ใช่ Global API Key)**

#### 4.1 ไปที่ API Tokens

1. Profile icon (มุมขวาบน) → **My Profile**
2. เมนูซ้าย → **API Tokens**
3. คลิก **Create Token**

#### 4.2 เลือก Template

**เลือก custom template:**
- คลิก **Create Custom Token**

**หรือใช้ template:**
- **Edit Cloudflare Workers** (มี permissions พื้นฐาน)
- แล้ว customize เพิ่ม

#### 4.3 ตั้งค่า Permissions (Minimal - ใช้ได้จริง)

**ตั้งชื่อ:**
```
Token name: schoolorbit-backend
```

**Permissions ที่จำเป็น (เลือกเฉพาะที่มี):**

### Option 1: สำหรับ Free Plan (แนะนำ) ✅

| Resource Type | Resource | Permission |
|---------------|----------|------------|
| Account | Workers Scripts | Edit |
| Zone | DNS | Edit |
| Zone | Zone | Read |

**วิธีตั้งค่า:**

1. **Account Resources**
   - Permissions: **Workers Scripts** → **Edit**
   
2. **Zone Resources** 
   - Permissions: **DNS** → **Edit**
   - Permissions: **Zone** → **Read**

3. **Account Resources** (Optional - ถ้ามี)
   - Permissions: **Worker Tail** → **Read** (optional)

### Option 2: ถ้ามี Paid Plan

| Resource Type | Resource | Permission |
|---------------|----------|------------|
| Account | Workers Scripts | Edit |
| Account | Workers KV Storage | Edit |
| Zone | Workers Routes | Edit |
| Zone | DNS | Edit |
| Zone | Zone | Read |

**Note:** Workers Routes มักจะมีเฉพาะใน Paid plans หรือต้อง enable Workers ก่อน

---

### ตัวเลือกอื่น: ใช้ Template "Edit Cloudflare Workers"

**ง่ายกว่า - ใช้ template สำเร็จรูป:**

1. ที่หน้า API Tokens → **Create Token**
2. เลือก template: **"Edit Cloudflare Workers"**
3. คลิก **Use template**
4. แก้ไข:
   - Zone Resources → เลือก **Specific zone** → `schoolorbit.app`
   - เพิ่ม Permission: **DNS** → **Edit**
5. **Continue to summary**
6. **Create Token**

Template นี้จะมี permissions พื้นฐานที่จำเป็นแล้ว!

#### 4.4 Zone Resources

**Include:**
- Specific zone → เลือก `schoolorbit.app`

**หรือ All zones** (ถ้ามีหลาย domains)

#### 4.5 IP Address Filtering (Optional)

- ถ้า deploy จาก server คงที่ → ระบุ IP
- ถ้า deploy จาก GitHub Actions → เว้นว่าง (Allow all IPs)

#### 4.6 TTL (Optional)

- ตั้ง expiration date (แนะนำ 1 ปี)
- หรือเว้นว่าง (ไม่มี expiration)

#### 4.7 Create Token

1. คลิก **Continue to summary**
2. Review permissions
3. คลิก **Create Token**
4. **Copy token ทันที!** (จะไม่แสดงอีก)

```
CLOUDFLARE_API_TOKEN=abc123xyz_veryLongToken...
```

**⚠️ เก็บ token ให้ดี - จะไม่แสดงอีกครั้ง!**

---

## ✅ ทดสอบ API Token

### 1. ทดสอบ Token ใช้ได้

```bash
curl -X GET "https://api.cloudflare.com/client/v4/user/tokens/verify" \
  -H "Authorization: Bearer YOUR_API_TOKEN" \
  -H "Content-Type: application/json"
```

**Expected response:**
```json
{
  "success": true,
  "result": {
    "id": "...",
    "status": "active"
  }
}
```

### 2. ทดสอบ List Zones

```bash
curl -X GET "https://api.cloudflare.com/client/v4/zones" \
  -H "Authorization: Bearer YOUR_API_TOKEN" \
  -H "Content-Type: application/json"
```

**Expected:**
```json
{
  "success": true,
  "result": [
    {
      "id": "your-zone-id",
      "name": "schoolorbit.app"
    }
  ]
}
```

---

## 📝 ใส่ Environment Variables

### backend-admin/.env

```bash
# Cloudflare API (for Workers deployment)
CLOUDFLARE_API_TOKEN=abc123xyz_veryLongToken...
CLOUDFLARE_ACCOUNT_ID=1234567890abcdef
CLOUDFLARE_ZONE_ID=abc123def456...
```

### Portainer Stack Environment

```
CLOUDFLARE_API_TOKEN=abc123xyz_veryLongToken...
CLOUDFLARE_ACCOUNT_ID=1234567890abcdef
CLOUDFLARE_ZONE_ID=abc123def456...
```

---

## 🎯 Quick Reference

### หา Account ID:
1. Dashboard → ดู URL
2. รูปแบบ: `https://dash.cloudflare.com/ACCOUNT_ID/...`

### หา Zone ID:
1. Dashboard → เลือก domain
2. Scroll down → ด้านขวา → API section → Zone ID

### สร้าง API Token:
1. Profile → API Tokens → Create Token
2. Permissions:
   - Account: Workers Scripts (Edit)
   - Account: Workers Routes (Edit)
   - Zone: Workers Routes (Edit)
   - Zone: DNS (Edit)
   - Zone: Zone (Read)
3. Zone: เลือก `schoolorbit.app`
4. Create → **Copy token ทันที!**

---

## 🔒 Security Best Practices

### ✅ ทำ:

```bash
# ✅ ใช้ Custom Token (มี permissions จำกัด)
# ✅ Set expiration date
# ✅ เก็บ token ใน .env (don't commit)
# ✅ ใช้ different tokens สำหรับ dev/prod
# ✅ Rotate tokens ทุก 6-12 เดือน
```

### ❌ อย่าทำ:

```bash
# ❌ ใช้ Global API Key (มี full access)
# ❌ Share token publicly
# ❌ Commit token to git
# ❌ ใช้ token เดียวกันทุก environment
```

---

## 🐛 Troubleshooting

### "Invalid API Token"

```bash
# ตรวจสอบ token format
echo $CLOUDFLARE_API_TOKEN | wc -c
# ควรยาว 40+ characters

# ทดสอบ token
curl -X GET "https://api.cloudflare.com/client/v4/user/tokens/verify" \
  -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN"
```

### "Insufficient permissions"

```bash
# Token ต้องมี permissions:
# - Workers Scripts: Edit
# - Workers Routes: Edit
# - DNS: Edit
# - Zone: Read

# Re-create token ด้วย permissions ที่ถูกต้อง
```

### "Zone not found"

```bash
# ตรวจสอบ Zone ID
curl -X GET "https://api.cloudflare.com/client/v4/zones" \
  -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" | jq

# หา zone ID ที่ถูกต้อง
```

---

## 📚 Additional Resources

### Cloudflare API Documentation

- **API Docs**: https://developers.cloudflare.com/api/
- **Workers**: https://developers.cloudflare.com/workers/
- **DNS**: https://developers.cloudflare.com/dns/
- **API Token Permissions**: https://developers.cloudflare.com/fundamentals/api/get-started/create-token/

### Support

- **Community**: https://community.cloudflare.com/
- **Discord**: https://discord.gg/cloudflaredev
- **Docs**: https://developers.cloudflare.com/

---

## 🎯 Complete Example

```bash
# หลังจากทำตามขั้นตอนแล้ว จะได้:

# 1. Account ID (จาก URL)
CLOUDFLARE_ACCOUNT_ID=1a2b3c4d5e6f7g8h

# 2. Zone ID (จาก domain settings)
CLOUDFLARE_ZONE_ID=9i0j1k2l3m4n5o6p

# 3. API Token (จาก Create Token)
CLOUDFLARE_API_TOKEN=abc123xyz_veryLongRandomStringHere...

# ใส่ใน .env
cat >> backend-admin/.env << EOF
CLOUDFLARE_API_TOKEN=abc123xyz_veryLongRandomStringHere...
CLOUDFLARE_ACCOUNT_ID=1a2b3c4d5e6f7g8h
CLOUDFLARE_ZONE_ID=9i0j1k2l3m4n5o6p
EOF
```

---

**Cloudflare API พร้อมใช้งาน!** 🚀
