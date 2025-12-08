# Inter-Service Communication Guide

คู่มือการทำงานร่วมกันระหว่าง backend-admin และ backend-school

---

## 🎯 Architecture

```
User
  │
  ↓ POST /api/v1/schools
backend-admin (Port 8080)
  │
  ├─→ Create Neon Database
  │
  ├─→ POST /api/v1/init-database
  │   backend-school (Port 8081)
  │     │
  │     ├─→ Connect to Database
  │     ├─→ Run Migrations (students, teachers, etc.)
  │     └─→ Return Success + Table List
  │
  ├─→ Deploy Cloudflare Worker
  │
  ├─→ Create DNS + Routes
  │
  └─→ Return Response
User
```

---

## 🔧 Services

### backend-admin (Port 8080)
**Role:** Orchestrator
- สร้าง schools
- จัดการ Neon database provisioning
- Deploy Workers
- เรียก backend-school เพื่อ init database

### backend-school (Port 8081)
**Role:** Database Migration Service
- รับ connection string
- Run migrations
- Return success status

---

## 🚀 Running Both Services

### Terminal 1: backend-school

```bash
cd backend-school
PORT=8081 cargo run --release
```

Output:
```
🏫 Starting Backend-School Template Service...
   This service handles database initialization for new schools
✅ Service ready on http://0.0.0.0:8081
   POST /api/v1/init-database - Initialize new school database
```

### Terminal 2: backend-admin

```bash
cd backend-admin
BACKEND_SCHOOL_URL=http://localhost:8081 cargo run --release
```

Output:
```
🚀 Starting SchoolOrbit Backend Admin Service...
✅ Connected to Neon PostgreSQL
...
```

---

## 📡 API Flow

### 1. User Creates School

```bash
curl -X POST http://localhost:8080/api/v1/schools \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "name": "โรงเรียนทดสอบ",
    "subdomain": "test-school",
    "adminNationalId": "1234567890123",
    "adminPassword": "test123"
  }'
```

### 2. backend-admin Calls backend-school

```rust
// Internal API call
POST http://localhost:8081/api/v1/init-database
{
  "databaseUrl": "postgresql://user:pass@host/schoolorbit_test_school"
}
```

### 3. backend-school Returns

```json
{
  "success": true,
  "message": "Database initialized successfully",
  "tablesCreated": [
    "admin_users",
    "students",
    "teachers",
    "classes",
    "attendance",
    "grades",
    "announcements"
  ]
}
```

### 4. backend-admin Continues

- Update school record
- Deploy Worker
- Create DNS
- Return to user

---

## 🔒 Security

### Authentication

backend-school ควร protected ใน production:

```rust
// Add auth middleware
async fn require_api_key(
    headers: HeaderMap,
    request: Request,
    next: Next
) -> Response {
    let api_key = env::var("BACKEND_SCHOOL_API_KEY")?;
    
    match headers.get("X-API-Key") {
        Some(key) if key == api_key => next.run(request).await,
        _ => StatusCode::UNAUTHORIZED.into_response()
    }
}
```

### Network

- **Development:** localhost communication
- **Production:** Internal VPC network หรือ VPN
- **Never expose backend-school to public internet**

---

## 🌐 Production Deployment

### Option 1: Same Server

```bash
# backend-school
PORT=8081 ./backend-school &

# backend-admin  
BACKEND_SCHOOL_URL=http://localhost:8081 ./backend-admin
```

### Option 2: Separate Servers (VPC)

```bash
# Server A (backend-school)
PORT=8081 ./backend-school

# Server B (backend-admin)
BACKEND_SCHOOL_URL=http://10.0.1.5:8081 ./backend-admin
```

### Option 3: Docker Compose

```yaml
version: '3.8'
services:
  backend-school:
    build: ./backend-school
    ports:
      - "8081:8081"
    networks:
      - internal
      
  backend-admin:
    build: ./backend-admin
    ports:
      - "8080:8080"
    environment:
      - BACKEND_SCHOOL_URL=http://backend-school:8081
    networks:
      - internal
      - public
    depends_on:
      - backend-school

networks:
  internal:
  public:
```

---

## 🧪 Testing

### Test backend-school Independently

```bash
# Start service
cd backend-school
cargo run

# Test endpoint
curl -X POST http://localhost:8081/api/v1/init-database \
  -H "Content-Type: application/json" \
  -d '{
    "databaseUrl": "postgresql://user:pass@host/test_db"
  }'
```

Expected Response:
```json
{
  "success": true,
  "message": "Database initialized successfully",
  "tablesCreated": ["admin_users", "students", ...]
}
```

### Test Full Flow

```bash
# 1. Start both services
cd backend-school && cargo run &
cd backend-admin && cargo run &

# 2. Create school
curl -X POST http://localhost:8080/api/v1/schools \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{"name": "Test", "subdomain": "test", ...}'

# 3. Check logs
# backend-admin: "🔧 Requesting backend-school..."
# backend-school: "📊 Initializing database..."
# backend-admin: "✅ Database initialized successfully"
```

---

## 🐛 Troubleshooting

### "Failed to call backend-school: Connection refused"

```bash
# Check backend-school is running
curl http://localhost:8081/health

# Check BACKEND_SCHOOL_URL
echo $BACKEND_SCHOOL_URL
```

### "Backend-school returned error: 500"

```bash
# Check backend-school logs
# Look for migration errors
```

### "Migration execution failed"

```bash
# Verify database exists
psql "postgresql://..." -c "\dt"

# Check migration SQL syntax
cd backend-school/migrations
cat 20250101000000_initial_schema.sql
```

---

## 📊 Monitoring

### Health Checks

```bash
# backend-school
curl http://localhost:8081/health

# backend-admin
curl http://localhost:8080/health
```

### Logs

```bash
# backend-school shows:
📊 Initializing database: postgresql://...
  🔗 Connected to database
  🔧 Running migrations...
  ✅ Migrations completed

# backend-admin shows:
🚀 Starting deployment for school: โรงเรียนทดสอบ
  📊 Creating database...
  🔧 Requesting backend-school to initialize database...
  ✅ Database initialized successfully
  📊 Tables created: admin_users, students, ...
```

---

## ✅ Checklist

- [ ] backend-school running on port 8081
- [ ] backend-admin configured with BACKEND_SCHOOL_URL
- [ ] Migration files present in backend-school/migrations/
- [ ] Network connectivity between services
- [ ] API endpoint tested independently
- [ ] Full flow tested (create school)
- [ ] Logs visible from both services
- [ ] Error handling works

---

**🎉 Inter-service communication ready!**
