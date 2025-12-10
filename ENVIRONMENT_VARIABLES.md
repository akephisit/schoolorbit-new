# Environment Variables Reference

รายการ environment variables ที่ใช้ในแต่ละ service

---

## ✅ backend-admin

### Required Variables (ต้องมี)

```bash
# Database Connection
DATABASE_URL=postgresql://user:password@host:5432/schoolorbit_admin
# Format: postgresql://USER:PASSWORD@HOST:PORT/DATABASE
# Example: postgresql://admin:pass123@postgres:5432/schoolorbit_admin

# JWT Authentication
JWT_SECRET=your-super-secret-jwt-key-change-this
# ใช้สำหรับ sign/verify JWT tokens
# ต้องเป็น string ยาวๆ และซับซ้อน

# Cloudflare API (for auto-deployment)
CLOUDFLARE_API_TOKEN=your-cloudflare-api-token
CLOUDFLARE_ACCOUNT_ID=your-cloudflare-account-id  
CLOUDFLARE_ZONE_ID=your-zone-id-for-schoolorbit-app

# Backend-School Service URL
BACKEND_SCHOOL_URL=http://backend-school:8081
# Local: http://localhost:8081
# Docker: http://backend-school:8081 (service name)
```

### Optional Variables

```bash
# Server Configuration
PORT=8080                    # Default: 8080
RUST_LOG=info               # Logging level: error, warn, info, debug, trace
```

---

## ✅ backend-school

### Required Variables (ต้องมี)

```bash
# Neon PostgreSQL API
NEON_API_KEY=neon_api_xxxxxxxxxxxxxxx
# Get from: https://console.neon.tech → Account Settings → API Keys

NEON_PROJECT_ID=crimson-frost-12345678
# Get from: Project URL หรือ Project Settings

NEON_HOST=ep-cool-darkness-123456.us-east-2.aws.neon.tech
# Get from: Connection String (เฉพาะ host part)

NEON_USER=neondb_owner
# Get from: Connection String (username)
# Default: neondb_owner

NEON_PASSWORD=your-neon-password
# Get from: Connection String (password)
```

### Optional Variables

```bash
# Server Configuration
PORT=8081                    # Default: 8081
RUST_LOG=info               # Logging level
```

---

## 📋 Verification Checklist

### backend-admin ✓

ใช้ env variables ทั้งหมด:
- ✅ `DATABASE_URL` - main.rs:26
- ✅ `JWT_SECRET` - auth.rs:33, 57
- ✅ `CLOUDFLARE_API_TOKEN` - cloudflare.rs:39
- ✅ `CLOUDFLARE_ACCOUNT_ID` - cloudflare.rs:41
- ✅ `CLOUDFLARE_ZONE_ID` - deployment.rs:47
- ✅ `BACKEND_SCHOOL_URL` - deployment.rs:142
- ✅ `PORT` - (optional, default 8080)
- ✅ `RUST_LOG` - (optional, for logging)

**.env.example ครบถ้วน:** ✅

---

### backend-school ✓

ใช้ env variables ทั้งหมด:
- ✅ `NEON_API_KEY` - neon.rs:40
- ✅ `NEON_PROJECT_ID` - neon.rs:42
- ✅ `NEON_HOST` - neon.rs:97
- ✅ `NEON_USER` - neon.rs:99
- ✅ `NEON_PASSWORD` - neon.rs:100
- ✅ `PORT` - main.rs:42
- ✅ `RUST_LOG` - (optional, for logging)

**.env.example ครบถ้วน:** ✅

---

## 🐳 Docker Compose / Portainer

### backend-admin stack

```yaml
environment:
  - DATABASE_URL=postgresql://admin:pass@postgres:5432/schoolorbit_admin
  - JWT_SECRET=change-this-in-production
  - PORT=8080
  - RUST_LOG=info
  - CLOUDFLARE_API_TOKEN=your_token
  - CLOUDFLARE_ACCOUNT_ID=your_account_id
  - CLOUDFLARE_ZONE_ID=your_zone_id
  - BACKEND_SCHOOL_URL=http://backend-school:8081
```

### backend-school stack

```yaml
environment:
  - PORT=8081
  - RUST_LOG=info
  - NEON_API_KEY=neon_api_xxx
  - NEON_PROJECT_ID=crimson-frost-xxx
  - NEON_HOST=ep-xxx.aws.neon.tech
  - NEON_USER=neondb_owner
  - NEON_PASSWORD=your_password
```

---

## 🔒 Security Notes

### ❌ อย่าทำ:
```bash
# ❌ Commit .env to git
git add .env
git commit -m "add env"

# ❌ Hard-code secrets in source code
const JWT_SECRET = "my-secret";

# ❌ Share .env publicly
echo $DATABASE_URL  # in public chat/forum
```

### ✅ ทำ:
```bash
# ✅ Use .env and .gitignore
echo ".env" >> .gitignore

# ✅ Use different secrets per environment
# dev: .env.development
# prod: Portainer secrets / GitHub secrets

# ✅ Rotate secrets regularly
# JWT_SECRET: every 90 days
# API tokens: every 6 months
```

---

## 🧪 Testing

### Test Local Development

```bash
# Copy example
cp .env.example .env

# Edit with real values
vim .env

# Test backend-admin
cd backend-admin
cargo run

# Should see:
# ✅ Connected to database
# ✅ Server running on 0.0.0.0:8080
```

### Test Docker

```bash
# With docker-compose
cd backend-admin
docker-compose up

# Should NOT see:
# ❌ "environment variable not found"
# ❌ "panicked at 'DATABASE_URL must be set'"
```

---

## 🐛 Common Errors

### "DATABASE_URL must be set"

```bash
# Missing in environment
# Add to .env or Portainer stack
DATABASE_URL=postgresql://...
```

### "JWT_SECRET not set"

```bash
# Missing JWT_SECRET
# Add to .env
JWT_SECRET=your-secret-key-here
```

### "NEON_API_KEY not set"

```bash
# backend-school missing API key
# Add to .env
NEON_API_KEY=neon_api_xxx
```

### Connection Refused

```bash
# Wrong DATABASE_URL format or host
# Check:
DATABASE_URL=postgresql://user:pass@CORRECT_HOST:5432/db
#                                    ^^^^^^^^^^^^
# Docker: use service name (postgres)
# Local: use localhost
```

---

## 📚 See Also

- [NEON_API_SETUP.md](./NEON_API_SETUP.md) - How to get Neon credentials
- [PORTAINER_DEPLOYMENT.md](./PORTAINER_DEPLOYMENT.md) - Deployment guide
- [CONTAINER_RESTART_TROUBLESHOOTING.md](./CONTAINER_RESTART_TROUBLESHOOTING.md) - Debug restart loops

---

**Environment variables documented!** ✅
