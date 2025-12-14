# วิธีเพิ่ม Cloudflare API Token Permissions

## ปัญหา

เมื่อ deploy Worker ใน GitHub Actions ขึ้น error:

```
Authentication error [code: 10000]
A request to (/zones/.../workers/routes) failed.
Please ensure it has the correct permissions.
```

**สาเหตุ:** API Token ไม่มี permission สำหรับสร้าง Worker Routes

---

## วิธีแก้ไข: เพิ่ม Permission ให้ API Token

### ขั้นตอนที่ 1: เข้าสู่ Cloudflare Dashboard

1. ไปที่: https://dash.cloudflare.com/profile/api-tokens
2. Login ด้วย account ที่มี zone `schoolorbit.app`

### ขั้นตอนที่ 2: สร้าง API Token ใหม่ (หรือแก้ไขตัวเดิม)

#### ตัวเลือก A: สร้าง Token ใหม่ (แนะนำ)

1. คลิก **Create Token**
2. เลือก **Custom token**
3. ตั้งชื่อ Token: `SchoolOrbit Worker Deployment`

#### ตัวเลือก B: แก้ไข Token เดิม

1. หา Token ที่ใช้อยู่ (ดูชื่อใน GitHub Secrets)
2. คลิก **Edit** ข้าง Token นั้น

### ขั้นตอนที่ 3: กำหนด Permissions

เพิ่ม Permissions ทั้ง 2 อย่างนี้:

#### Permission 1: Workers Scripts (Account-level)

```
Permissions:
├─ Account
│  └─ Workers Scripts ...................... Edit
```

**คลิก:**
- Permissions dropdown → **Account**
- เลือก **Workers Scripts**
- Access level: **Edit**

#### Permission 2: Workers Routes (Zone-level)

```
Permissions:
├─ Zone
│  └─ Workers Routes ....................... Edit
```

**คลิก:**
- Permissions dropdown → **Zone**
- เลือก **Workers Routes**
- Access level: **Edit**

**Zone Resources:**
- เลือก **Specific zone**
- เลือก zone: **schoolorbit.app**

### ขั้นตอนที่ 4: (Optional) เพิ่ม DNS Permission

ถ้าต้องการให้ Wrangler จัดการ DNS ด้วย:

```
Permissions:
├─ Zone
│  └─ DNS .............................. Edit
```

**คลิก:**
- Permissions dropdown → **Zone**
- เลือก **DNS**
- Access level: **Edit**
- Zone: **schoolorbit.app**

### ขั้นตอนที่ 5: กำหนด Additional Settings

#### IP Filtering (Optional)

- ปกติไม่ต้องกำหนด (ใช้ได้ทุก IP)
- ถ้าต้องการความปลอดภัยสูง: เพิ่ม GitHub Actions IP ranges

#### TTL (Token Expiry)

- ค่า Default: ไม่หมดอายุ
- แนะนำ: 1 ปี (เพื่อความปลอดภัย)

### ขั้นตอนที่ 6: สร้าง Token

1. คลิก **Continue to summary**
2. ตรวจสอบ Permissions:
   ```
   ✅ Account > Workers Scripts > Edit
   ✅ Zone > Workers Routes > Edit (schoolorbit.app)
   ✅ Zone > DNS > Edit (schoolorbit.app) [Optional]
   ```
3. คลิก **Create Token**
4. **⚠️ Copy Token ทันที!** (จะแสดงครั้งเดียว)

---

## อัพเดท GitHub Secret

### ขั้นตอนที่ 1: ไปที่ GitHub Repository

1. ไปที่: https://github.com/akephisit/schoolorbit-new
2. คลิก **Settings** (ด้านบนขวา)

### ขั้นตอนที่ 2: เข้าสู่ Secrets

1. เมนูซ้าย: **Secrets and variables** → **Actions**
2. ดู Secrets list

### ขั้นตอนที่ 3: อัพเดท CLOUDFLARE_API_TOKEN

1. หา `CLOUDFLARE_API_TOKEN`
2. คลิก **Update** (icon ดินสอ)
3. **Paste** Token ใหม่ที่ copy มา
4. คลิก **Update secret**

---

## ทดสอบ

### ขั้นตอนที่ 1: Pull Code ล่าสุด

```bash
cd /path/to/schoolorbit-new
git pull origin main
```

### ขั้นตอนที่ 2: สร้างโรงเรียนใหม่

1. เปิด Admin Dashboard: https://admin.schoolorbit.app
2. Login
3. สร้างโรงเรียนใหม่

### ขั้นตอนที่ 3: ตรวจสอบ Deployment

1. ไปที่: https://github.com/akephisit/schoolorbit-new/actions
2. ดู workflow run ล่าสุด
3. ควรเห็น:
   ```
   ✅ Success! Uploaded X files
   ✅ Deployed schoolorbit-school-[subdomain]
   ✅ Worker route created: [subdomain].schoolorbit.app/*
   ```

### ขั้นตอนที่ 4: ทดสอบ Custom Domain

```bash
curl https://[subdomain].schoolorbit.app
```

ควรได้ response จาก SvelteKit app (ไม่ใช่ Cloudflare error page)

---

## สรุป Permissions ที่ต้องการ

| Permission | Resource | Zone/Account | Access |
|------------|----------|--------------|--------|
| Workers Scripts | Account | All accounts | Edit |
| Workers Routes | Zone | schoolorbit.app | Edit |
| DNS (Optional) | Zone | schoolorbit.app | Edit |

---

## Troubleshooting

### Error: "Authentication error [code: 10000]"

**สาเหตุ:** Token ไม่มี permission
**แก้ไข:** ตรวจสอบ permissions ตามขั้นตอนด้านบน

### Error: "Zone not found"

**สาเหตุ:** Token ไม่ได้เลือก zone `schoolorbit.app`
**แก้ไข:** แก้ไข Token → Zone Resources → เลือก `schoolorbit.app`

### Deploy สำเร็จแต่เข้า domain ไม่ได้

**สาเหตุ:** DNS record ยังไม่มี หรือไม่ถูก proxied
**แก้ไข:**
1. ไป Cloudflare Dashboard → DNS
2. ตรวจสอบว่ามี record สำหรับ subdomain
3. ตรวจสอบว่า Proxied = ☁️ (Orange cloud)

---

## อ้างอิง

- [Cloudflare API Tokens](https://developers.cloudflare.com/fundamentals/api/get-started/create-token/)
- [Workers Routes](https://developers.cloudflare.com/workers/configuration/routing/routes/)
- [Wrangler Configuration](https://developers.cloudflare.com/workers/wrangler/configuration/)
