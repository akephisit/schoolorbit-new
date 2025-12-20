# Frontend Admin - Cloudflare Pages Deployment Guide

## Overview

`frontend-admin` ถูก deploy บน **Cloudflare Pages** โดยใช้ SvelteKit กับ `@sveltejs/adapter-cloudflare`

## การตั้งค่า Environment Variables

### วิธีที่ 1: ผ่าน Cloudflare Dashboard

1. เข้า Cloudflare Dashboard
2. ไปที่ **Pages** → เลือก project `schoolorbit-frontend-admin`
3. ไปที่ **Settings** → **Environment variables**
4. เพิ่ม environment variables ต่อไปนี้:

#### Production Environment
```
PUBLIC_API_URL=https://admin-api.schoolorbit.app
BACKEND_SCHOOL_URL=https://school-api.schoolorbit.app
INTERNAL_API_SECRET=<ใส่ secret ที่ตรงกับ backend-school>
```

#### Preview Environment (ถ้าต้องการ)
```
PUBLIC_API_URL=https://admin-api-preview.schoolorbit.app
BACKEND_SCHOOL_URL=https://school-api-preview.schoolorbit.app
INTERNAL_API_SECRET=<ใส่ secret สำหรับ preview>
```

5. กด **Save** แล้วทำการ **Redeploy** project

### วิธีที่ 2: ผ่าน wrangler.json

ไฟล์ `wrangler.json` มีค่า default สำหรับ production แล้ว แต่คุณควร**เปลี่ยน `INTERNAL_API_SECRET`** ให้ตรงกับที่ตั้งใน backend-school:

```json
{
  "vars": {
    "PUBLIC_API_URL": "https://admin-api.schoolorbit.app",
    "BACKEND_SCHOOL_URL": "https://school-api.schoolorbit.app",
    "INTERNAL_API_SECRET": "your-actual-secret-here"
  }
}
```

> [!WARNING]
> **อย่า commit secret จริงลง Git!** 
> ควรใช้วิธีที่ 1 (Cloudflare Dashboard) สำหรับค่า production จริง

## การตรวจสอบ Environment Variables

หลังจาก deploy แล้ว สามารถตรวจสอบได้โดย:

1. เปิด browser console ที่ `https://admin.schoolorbit.app/dashboard/migrations`
2. ดูที่ Network tab
3. ตรวจสอบ request ไปที่ `/api/migration/status`
4. ถ้า environment variables ไม่ถูกต้อง จะได้ response:
   ```json
   {
     "error": "Migration service not configured",
     "details": "BACKEND_SCHOOL_URL environment variable is missing"
   }
   ```

## Troubleshooting

### Error: "Failed to fetch migration status"

**สาเหตุ:**
- `BACKEND_SCHOOL_URL` หรือ `INTERNAL_API_SECRET` ไม่ได้ตั้งค่า
- Backend-school service ไม่สามารถเข้าถึงได้
- Secret ไม่ตรงกัน

**วิธีแก้:**
1. ตรวจสอบ environment variables ใน Cloudflare Dashboard
2. ตรวจสอบว่า backend-school service รันอยู่
3. ตรวจสอบ logs ของ Cloudflare Pages ใน Functions → Logs

### Error 503: "Migration service not configured"

**สาเหตุ:**
Environment variables ไม่ได้ตั้งค่าใน Cloudflare

**วิธีแก้:**
ตั้งค่า environment variables ตามขั้นตอนด้านบน แล้ว redeploy

### CORS Errors

**สาเหตุ:**
Backend-school ไม่อนุญาตให้เข้าถึงจาก frontend domain

**วิธีแก้:**
ตรวจสอบว่า backend-school มี CORS headers ที่ถูกต้อง (แต่ไม่น่าจะเป็นปัญหาเพราะเป็น server-side API route)

## Local Development

สำหรับการพัฒนาในเครื่อง:

1. Copy `.env.example` เป็น `.env`:
   ```bash
   cp .env.example .env
   ```

2. แก้ไขค่าใน `.env`:
   ```env
   PUBLIC_API_URL=http://localhost:8080
   BACKEND_SCHOOL_URL=http://localhost:8081
   INTERNAL_API_SECRET=your-local-secret
   ```

3. รัน dev server:
   ```bash
   npm run dev
   ```

## Deployment Process

1. Push code ไป GitHub
2. Cloudflare Pages จะ auto-deploy ทันที
3. ตรวจสอบ deployment status ใน Cloudflare Dashboard
4. ทดสอบที่ `https://admin.schoolorbit.app`

## Related Files

- [`wrangler.json`](file:///home/kruakemaths/github/schoolorbit-new/frontend-admin/wrangler.json) - Cloudflare configuration
- [`svelte.config.js`](file:///home/kruakemaths/github/schoolorbit-new/frontend-admin/svelte.config.js) - SvelteKit adapter config
- [`.env.example`](file:///home/kruakemaths/github/schoolorbit-new/frontend-admin/.env.example) - Environment variables template
