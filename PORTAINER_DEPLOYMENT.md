# Portainer Deployment Guide

วิธี deploy backend-admin และ backend-school แยก stack บน Portainer

---

## 🎯 Architecture

```
Portainer Stacks:
├── Stack 1: backend-school (Port 8081)
│   └── Database lifecycle management
│
├── Stack 2: backend-admin (Port 8080)
│   └── Orchestration & management
│
└── Shared Network: schoolorbit-network
    (ให้ services คุยกันได้)
```

---

## 📋 Pre-requisites

### 1. สร้าง Docker Network (ครั้งแรกอย่างเดียว)

```bash
docker network create schoolorbit-network
```

หรือใน Portainer:
- Networks → Add network
- Name: `schoolorbit-network`
- Driver: `bridge`

---

## 🚀 Deployment Steps

### Step 1: Deploy backend-school (ก่อน)

**Portainer → Stacks → Add stack**

**Stack name:** `backend-school`

**Build method:** Repository

**Repository:**
- URL: `https://github.com/your-org/schoolorbit-new`
- Reference: `main`
- Compose path: `backend-school/docker-compose.yml`

**Environment variables:**
```env
NEON_API_KEY=your_neon_api_key
NEON_PROJECT_ID=your_project_id
NEON_HOST=ep-xxx.aws.neon.tech
NEON_USER=neondb_owner
NEON_PASSWORD=your_password
```

**Deploy!**

---

### Step 2: Deploy backend-admin (หลัง)

**Portainer → Stacks → Add stack**

**Stack name:** `backend-admin`

**Build method:** Repository

**Repository:**
- URL: `https://github.com/your-org/schoolorbit-new`
- Reference: `main`
- Compose path: `backend-admin/docker-compose.yml`

**Environment variables:**
```env
DATABASE_URL=postgresql://user:pass@host/schoolorbit_admin
JWT_SECRET=your-super-secret-key
CLOUDFLARE_API_TOKEN=your_cloudflare_token
CLOUDFLARE_ACCOUNT_ID=your_account_id
CLOUDFLARE_ZONE_ID=your_zone_id
```

**Deploy!**

---

## ✅ Verification

### 1. Check Containers

```bash
docker ps | grep schoolorbit
```

Expected:
```
schoolorbit-backend-school   Up    0.0.0.0:8081->8081/tcp
schoolorbit-backend-admin    Up    0.0.0.0:8080->8080/tcp
```

### 2. Check Health

```bash
# Backend-school
curl http://localhost:8081/health

# Backend-admin
curl http://localhost:8080/health
```

### 3. Check Logs

```bash
# Via Portainer UI
Stacks → backend-school → Logs
Stacks → backend-admin → Logs

# Via Docker
docker logs schoolorbit-backend-school
docker logs schoolorbit-backend-admin
```

### 4. Test Communication

```bash
# Create a test school (should call backend-school internally)
curl -X POST http://localhost:8080/api/v1/schools \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "name": "Test School",
    "subdomain": "test"
  }'
```

---

## 🔄 Update Deployment

### Via Portainer

1. Stacks → Select stack
2. Editor → Pull latest
3. Update the stack

### Via Git Webhook (แนะนำ)

**Setup:**
1. Stacks → Select stack → Webhook
2. Enable webhook
3. Copy webhook URL

**GitHub:**
1. Repository → Settings → Webhooks
2. Add webhook
3. Paste Portainer webhook URL
4. Events: `push` to `main`

**ผลลัพธ์:** Push code → Auto deploy! 🚀

---

## 🐛 Troubleshooting

### Container ไม่ start

```bash
# Check logs
docker logs schoolorbit-backend-school --tail 100

# Common issues:
# - Network ไม่มี (สร้าง schoolorbit-network)
# - Environment variables ผิด
# - Port ถูกใช้แล้ว
```

### Services คุยกันไม่ได้

```bash
# Check network
docker network inspect schoolorbit-network

# ต้องเห็น containers ทั้ง 2
# - schoolorbit-backend-school
# - schoolorbit-backend-admin

# Test connectivity
docker exec schoolorbit-backend-admin \
  curl http://backend-school:8081/health
```

### Database connection failed

```bash
# Check DATABASE_URL
docker exec schoolorbit-backend-admin env | grep DATABASE_URL

# Test connection
docker exec schoolorbit-backend-admin \
  psql "$DATABASE_URL" -c "SELECT 1"
```

---

## 📊 Monitoring

### Portainer Dashboard

- Containers → View resources
- CPU usage
- Memory usage
- Network I/O

### Health Checks

Portainer จะ restart container อัตโนมัติ ถ้า health check fail:
- ✅ Healthy: สีเขียว
- ⚠️ Unhealthy: สีเหลือง
- ❌ Failed: restart

---

## 🔒 Security

### Production Checklist

- [ ] ใช้ secrets แทน environment variables
- [ ] Enable HTTPS (reverse proxy)
- [ ] Limit network exposure (internal only)
- [ ] Regular updates
- [ ] Log monitoring
- [ ] Backup strategy

### Portainer Secrets

1. Settings → Secrets → Add secret
2. Name: `neon_api_key`, Value: `xxx`
3. Stack → Environment → Reference secret

```yaml
environment:
  - NEON_API_KEY=${NEON_API_KEY}
secrets:
  - neon_api_key
```

---

## 📝 Quick Reference

### URLs

- **backend-school**: `http://your-server:8081`
- **backend-admin**: `http://your-server:8080`
- **Portainer**: `http://your-server:9000`

### Ports

- `8080` - backend-admin
- `8081` - backend-school
- `9000` - Portainer

### Network

- Name: `schoolorbit-network`
- Type: Bridge
- Scope: Local

---

**Ready to deploy!** 🎉
