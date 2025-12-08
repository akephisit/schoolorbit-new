# School Management API Documentation

## Authentication Required

ทุก endpoint ต้อง login ก่อน (มี auth_token cookie)

---

## Endpoints

### 1. Create School

**POST** `/api/v1/schools`

สร้างโรงเรียนใหม่

**Request Body:**
```json
{
  "name": "โรงเรียนตัวอย่าง",
  "subdomain": "example-school",
  "adminNationalId": "1234567890123",
  "adminPassword": "password123"
}
```

**Validation:**
- `adminNationalId` must be exactly 13 digits (Thai national ID)
- `subdomain` must be lowercase, alphanumeric, and hyphens only
- `subdomain` must be unique

```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "name": "โรงเรียนตัวอย่าง",
    "subdomain": "example-school",
    "dbName": "schoolorbit_example-school",
    "dbConnectionString": null,
    "status": "active",
    "config": {},
    "createdAt": "2025-12-08T10:00:00Z",
    "updatedAt": "2025-12-08T10:00:00Z"
  }
}
```

**Validation:**
- `subdomain` must be lowercase, alphanumeric, and hyphens only
- `subdomain` must be unique

---

### 2. List Schools

**GET** `/api/v1/schools?page=1&limit=10`

รายการโรงเรียนทั้งหมด (มี pagination)

**Query Parameters:**
- `page` (optional, default: 1) - หน้าที่
- `limit` (optional, default: 10) - จำนวนต่อหน้า

**Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "schools": [...],
    "total": 100,
    "page": 1,
    "limit": 10,
    "totalPages": 10
  }
}
```

---

### 3. Get School by ID

**GET** `/api/v1/schools/:id`

ดูข้อมูลโรงเรียนตาม ID

**Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "name": "โรงเรียนตัวอย่าง",
    ...
  }
}
```

**Response (404 Not Found):**
```json
{
  "error": "School not found"
}
```

---

### 4. Update School

**PUT** `/api/v1/schools/:id`

อัพเดทข้อมูลโรงเรียน

**Request Body (all fields optional):**
```json
{
  "name": "ชื่อใหม่",
  "status": "inactive",
  "config": {
    "schoolYear": "2024",
    "logo": "https://..."
  }
}
```

**Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": "uuid",
    "name": "ชื่อใหม่",
    ...
  }
}
```

---

### 5. Delete School

**DELETE** `/api/v1/schools/:id`

ลบโรงเรียน

**Response (200 OK):**
```json
{
  "success": true,
  "message": "School deleted"
}
```

**Response (404 Not Found):**
```json
{
  "error": "School not found"
}
```

---

## Testing with cURL

### Login First
```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "nationalId": "1234567890123",
    "password": "test123"
  }' \
  -c cookies.txt
```

### Create School
```bash
curl -X POST http://localhost:8080/api/v1/schools \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "name": "โรงเรียนตัวอย่าง",
    "subdomain": "example-school",
    "adminNationalId": "1234567890123",
    "adminPassword": "password123"
  }'
```

### List Schools
```bash
curl http://localhost:8080/api/v1/schools?page=1&limit=10 \
  -b cookies.txt
```

### Get School
```bash
curl http://localhost:8080/api/v1/schools/{school-id} \
  -b cookies.txt
```

### Update School
```bash
curl -X PUT http://localhost:8080/api/v1/schools/{school-id} \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{
    "name": "ชื่อใหม่",
    "status": "active"
  }'
```

### Delete School
```bash
curl -X DELETE http://localhost:8080/api/v1/schools/{school-id} \
  -b cookies.txt
```

---

## Error Responses

### 400 Bad Request
```json
{
  "error": "Validation error message"
}
```

### 401 Unauthorized
```json
{
  "error": "No auth token in cookie"
}
```

### 404 Not Found
```json
{
  "error": "School not found"
}
```

### 500 Internal Server Error
```json
{
  "error": "Database error message"
}
```

---

## Database Schema

```sql
CREATE TABLE schools (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    subdomain VARCHAR(100) UNIQUE NOT NULL,
    db_name VARCHAR(100) NOT NULL,
    db_connection_string TEXT,
    status VARCHAR(50) DEFAULT 'active',
    config JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## Next Steps

1. **Frontend Integration**: สร้าง UI สำหรับจัดการโรงเรียน
2. **Authentication Middleware**: เพิ่ม auth guard สำหรับ protected routes
3. **Database Provisioning**: สร้าง database อัตโนมัติสำหรับ school ใหม่
4. **Admin Assignment**: ระบบกำหนด admin ให้โรงเรียน
