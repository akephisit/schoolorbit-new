# API Examples - backend-admin

## Authentication

### Login with National ID

**Endpoint:** `POST /api/v1/auth/login`

**Request:**
```json
{
  "nationalId": "1234567890123",
  "password": "your-password"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "user": {
      "id": "uuid-here",
      "nationalId": "1234567890123",
      "name": "ชื่อผู้ใช้",
      "role": "super_admin"
    }
  }
}
```

**Validation Rules:**
- National ID must be exactly 13 digits
- All characters must be numeric
- Password required

---

## Create Admin User

**Request Body:**
```json
{
  "nationalId": "1234567890123",
  "password": "secure-password",
  "name": "ชื่อผู้ดูแล"
}
```

---

## Notes

- All timestamps are in UTC
- JWT token expires in 24 hours
- National ID format: 13 digits (Thai National ID)
- Password must be strong (validation can be added)
