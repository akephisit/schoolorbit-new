# Cloudflare Workers Deployment Guide

Frontend-admin พร้อม deploy บน Cloudflare Workers แล้ว!

## 📋 Prerequisites

- Cloudflare account
- Wrangler CLI installed: `npm install -g wrangler`
- Domain configured in Cloudflare

---

## 🚀 Deployment Steps

### 1. Login to Cloudflare

```bash
wrangler login
```

### 2. Configure wrangler.json

แก้ไฟล์ `wrangler.json`:

```json
{
  "name": "schoolorbit-admin",
  "main": "./.svelte-kit/output/server/index.js",
  "compatibility_date": "2024-12-01",
  "compatibility_flags": ["nodejs_compat"],
  "routes": [
    {
      "pattern": "admin.schoolorbit.app",
      "custom_domain": true
    }
  ],
  "assets": {
    "directory": ".svelte-kit/output/client",
    "binding": "ASSETS"
  },
  "vars": {
    "PUBLIC_API_URL": "https://admin-api.schoolorbit.app"
  }
}
```

### 3. Build

```bash
npm run build
```

### 4. Deploy

```bash
npx wrangler deploy
```

---

## 🔧 Environment Variables

### Production (.env.production)

```bash
PUBLIC_API_URL=https://admin-api.schoolorbit.app
```

### Development (.env.development)

```bash
PUBLIC_API_URL=http://localhost:8080
```

---

## 📝 wrangler.json Configuration

| Field | Description |
|-------|-------------|
| `name` | ชื่อ Worker |
| `main` | Entry point (SvelteKit output) |
| `compatibility_date` | Cloudflare runtime version |
| `compatibility_flags` | Enable Node.js compatibility |
| `routes` | Custom domain routing |
| `assets` | Static files (CSS, JS, images) |
| `vars` | Environment variables |

---

## 🌐 Custom Domain Setup

### 1. Add Domain to Cloudflare

1. ไปที่ Cloudflare Dashboard
2. **Websites** → **Add a site**
3. เพิ่ม `schoolorbit.app`
4. Update nameservers ตาม Cloudflare

### 2. Add DNS Record

1. **DNS** → **Records** → **Add record**
2. Type: `A` or `AAAA`
3. Name: `admin`
4. Content: `192.0.2.1` (dummy, Workers จะ override)
5. Proxy status: **Proxied** (orange cloud)

### 3. Configure Routes

```json
"routes": [
  {
    "pattern": "admin.schoolorbit.app",
    "custom_domain": true
  }
]
```

---

## 🔄 CI/CD with GitHub Actions

สร้าง `.github/workflows/deploy-frontend.yml`:

```yaml
name: Deploy Frontend to Cloudflare

on:
  push:
    branches: [main]
    paths:
      - 'frontend-admin/**'
  workflow_dispatch:

defaults:
  run:
    working-directory: frontend-admin

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: frontend-admin/package-lock.json
      
      - name: Install dependencies
        run: npm ci
      
      - name: Build
        run: npm run build
        env:
          PUBLIC_API_URL: ${{ secrets.PUBLIC_API_URL }}
      
      - name: Deploy to Cloudflare Workers
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          workingDirectory: frontend-admin
```

### GitHub Secrets

เพิ่มใน repository secrets:
- `CLOUDFLARE_API_TOKEN` - API token จาก Cloudflare
- `PUBLIC_API_URL` - Backend API URL

---

## 🧪 Testing

### Local Preview

```bash
npm run build
npx wrangler dev
```

### Production Test

```bash
# Deploy
npx wrangler deploy

# Open in browser
open https://admin.schoolorbit.app
```

---

## 🛠️ Troubleshooting

### Build Error: "Cannot find module"

```bash
# Clean and rebuild
rm -rf .svelte-kit node_modules
npm install
npm run build
```

### Worker Error: "Script not found"

ตรวจสอบ `wrangler.json`:
```json
"main": "./.svelte-kit/output/server/index.js"
```

### 502 Bad Gateway

- ตรวจสอบ `compatibility_flags: ["nodejs_compat"]`
- ตรวจสอบ build output มี `.svelte-kit/output/server/index.js`

### Environment Variables Not Working

```bash
# ตรวจสอบ
npx wrangler secret list

# เพิ่ม secret
npx wrangler secret put PUBLIC_API_URL
```

---

## 📊 Monitoring

### View Logs

```bash
npx wrangler tail
```

### Analytics

- Cloudflare Dashboard → Workers & Pages → Analytics

---

## ✅ Checklist

- [ ] Installed `@sveltejs/adapter-cloudflare`
- [ ] Updated `svelte.config.js`
- [ ] Configured `wrangler.json`
- [ ] Set environment variables
- [ ] Custom domain added to Cloudflare
- [ ] DNS records configured
- [ ] Build successful (`npm run build`)
- [ ] Deploy successful (`wrangler deploy`)
- [ ] Test at production URL
- [ ] CI/CD configured (optional)

---

## 🎯 Production Checklist

- [ ] HTTPS enabled (automatic with Cloudflare)
- [ ] Custom domain working
- [ ] API calls to backend working
- [ ] Authentication working
- [ ] Static assets loading
- [ ] No console errors
- [ ] Performance optimized

**🎉 Ready for production!**
