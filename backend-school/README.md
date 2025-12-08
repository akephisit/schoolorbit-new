# Backend-School Service

Database lifecycle management service for SchoolOrbit

## 🎯 Purpose

จัดการ database ของแต่ละโรงเรียน:
- Create databases via Neon API
- Run initial migrations
- Database provisioning

## 📦 Deployment

### Docker Compose (Standalone)

```bash
cd backend-school

# Copy environment file
cp .env.example .env
# Edit .env with your credentials

# Deploy
docker-compose up -d

# Check logs
docker-compose logs -f
```

### Portainer

See [PORTAINER_DEPLOYMENT.md](../PORTAINER_DEPLOYMENT.md)

## 🔧 Configuration

### Environment Variables

```env
PORT=8081
NEON_API_KEY=your_key
NEON_PROJECT_ID=your_project
NEON_HOST=ep-xxx.neon.tech
NEON_USER=neondb_owner
NEON_PASSWORD=your_password
```

## 🚀 API Endpoints

### POST /api/v1/create-school-database

Create and initialize a new school database

**Request:**
```json
{
  "schoolName": "Test School",
  "subdomain": "test"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Database created and initialized",
  "database_name": "schoolorbit_test",
  "connection_string": "postgresql://...",
  "tables_created": ["admin_users", "students", ...]
}
```

### GET /health

Health check endpoint

## 📊 Monitoring

```bash
# Health check
curl http://localhost:8081/health

# Logs
docker logs backend-school -f
```

## 🔗 Dependencies

- Neon PostgreSQL API
- SQLx for migrations

## 🏗️ Build

```bash
# Local
cargo build --release

# Docker
docker build -f Dockerfile -t backend-school .
```
