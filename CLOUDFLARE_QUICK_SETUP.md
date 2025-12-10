# Cloudflare API Token - Quick Setup for Free Plan

ขั้นตอนย่อสำหรับ Free Plan (ใช้เวลาแค่ 2 นาที)

---

## 🚀 Quick Setup (Free Plan)

### Step 1: Create Custom Token

1. Login → Profile (มุมขวาบน) → **My Profile**
2. เมนูซ้าย → **API Tokens**
3. คลิก **Create Token**
4. คลิก **Create Custom Token** (ด้านล่าง)

---

### Step 2: ตั้งค่า Token

**Token name:**
```
schoolorbit-backend
```

**Permissions (3 อัน):**

#### 1. Account Resources
```
Account | Workers Scripts | Edit
```

#### 2. Zone Resources  
```
Zone | DNS | Edit
Zone | Zone | Read
```

**Screenshot permissions:**
```
┌─────────────────────────────────────────┐
│ Account Resources                        │
├─────────────────────────────────────────┤
│ Workers Scripts                    Edit  │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Zone Resources                           │
├─────────────────────────────────────────┤
│ DNS                                Edit  │
│ Zone                               Read  │
└─────────────────────────────────────────┘
```

---

### Step 3: Zone

**Zone Resources:**
- Include → **Specific zone**
- เลือก dropdown → `schoolorbit.app` (domain ของคุณ)

---

### Step 4: Create

1. คลิก **Continue to summary**
2. Review
3. คลิก **Create Token**
4. **Copy token ทันที!** ⚠️

```
Token: abc123xyz_VeryLongRandomString...
```

---

### Step 5: เก็บใน .env

```bash
# backend-admin/.env
CLOUDFLARE_API_TOKEN=abc123xyz_VeryLongRandomString...
```

---

## ✅ Test Token

```bash
curl -X GET "https://api.cloudflare.com/client/v4/user/tokens/verify" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

**Expected:**
```json
{
  "success": true,
  "result": {
    "status": "active"
  }
}
```

---

## 🎯 สรุป Permissions ที่จำเป็น (Free Plan)

| Permission | ใช้สำหรับ | Required |
|------------|-----------|----------|
| Workers Scripts (Edit) | Deploy Workers | ✅ Yes |
| DNS (Edit) | Create/Update DNS records | ✅ Yes |
| Zone (Read) | Read zone info | ✅ Yes |
| ~~Workers Routes~~ | ไม่มีใน Free Plan | ❌ ไม่จำเป็น |

---

## ❓ FAQ

### Workers Routes ไม่มีให้เลือก?

**ปกติ!** Workers Routes มักจะมีกับ:
- Paid plans
- หรือ account ที่เคยใช้ Workers มาแล้ว

**สำหรับ Free plan → ข้ามไปได้**

---

### Template "Edit Cloudflare Workers" ง่ายกว่าไหม?

**ใช่!** แต่ต้องเพิ่ม DNS permission:

1. เลือก template **"Edit Cloudflare Workers"**
2. **Use template**
3. **เพิ่ม** permission: DNS → Edit
4. Zone: เลือก `schoolorbit.app`
5. Create

---

### ต้องใช้ Global API Key แทนไหม?

**ไม่แนะนำ!** Global API Key มี full access
- ❌ Dangerous (access ทุกอย่าง)
- ✅ ใช้ Custom Token แทน (จำกัด permissions)

---

**เสร็จแล้ว!** 🎉 ใช้เวลาแค่ 2 นาที
