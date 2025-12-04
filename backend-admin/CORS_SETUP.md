# CORS Multi-Domain Configuration

## สิ่งที่เพิ่มเข้ามา

### 1. Custom CORS Middleware (`src/middleware/cors.rs`)
สร้าง `MultiCors` middleware ที่รองรับ **หลาย origins** โดย:
- รับ list ของ allowed origins จาก environment variable
- ตรวจสอบ Origin header ของแต่ละ request
- ตั้งค่า CORS headers ตาม origin ที่ได้รับอนุญาต
- รองรับ credentials, custom headers, และ max-age

### 2. Environment Variable Configuration
เพิ่ม `ALLOWED_ORIGINS` ใน `.env.example`:
```bash
ALLOWED_ORIGINS=http://localhost:5173,http://localhost:3000,https://admin.yourdomain.com
```

### 3. Main Application Update
อัปเดต `main.rs` เพื่อใช้ `MultiCors` แทน built-in `Cors`:
```rust
let cors = MultiCors::from_env_string(&allowed_origins)
    .allow_headers(["Content-Type", "Authorization"])
    .allow_credentials(false)
    .max_age(Some(3600));

let app = Ohkami::with(cors, (...routes...));
```

## วิธีใช้งาน

### 1. เพิ่ม `ALLOWED_ORIGINS` ใน `.env`
```bash
# Development (multiple localhost ports)
ALLOWED_ORIGINS=http://localhost:5173,http://localhost:3000,http://localhost:8080

# Production
ALLOWED_ORIGINS=https://admin.yourschool.com,https://school.yourschool.com,https://app.yourschool.com
```

### 2. Restart Server
```bash
cd backend-admin
cargo run
```

### 3. ตรวจสอบ Log
Server จะแสดง allowed origins เมื่อ start:
```
🔐 CORS allowed origins: http://localhost:5173,http://localhost:3000
```

## Features

✅ **Multi-Origin Support** - รองรับหลาย domains พร้อมกัน
✅ **Environment-Based** - config ผ่าน environment variable
✅ **Dynamic Checking** - ตรวจสอบ origin แต่ละ request
✅ **Security** - ไม่ใช้ wildcard (*) เมื่อ credentials enabled
✅ **Flexible** - เพิ่ม/ลด domains ได้โดยไม่ต้อง rebuild code

## การปรับแต่งเพิ่มเติม

### เปิด Credentials
```rust
let cors = MultiCors::from_env_string(&allowed_origins)
    .allow_credentials(true)  // เปลี่ยนเป็น true
    .allow_headers(["Content-Type", "Authorization"])
    .max_age(Some(3600));
```

### เพิ่ม Custom Headers
```rust
let cors = MultiCors::from_env_string(&allowed_origins)
    .allow_headers([
        "Content-Type", 
        "Authorization",
        "X-Custom-Header",
        "Accept"
    ])
    .max_age(Some(3600));
```

### เปลี่ยน Max Age (Preflight Cache)
```rust
let cors = MultiCors::from_env_string(&allowed_origins)
    .max_age(Some(7200))  // 2 hours
    // or
    .max_age(None)  // ไม่มี cache
```

## Technical Details

### Thread-Local Storage
เนื่องจาก Ohkami Request/Response ไม่มี `memo` field เราใช้ `thread_local!` เก็บ allowed origin ระหว่าง `fore()` และ `back()` methods

### Header API
ใช้ Ohkami's header API:
- `.set().access_control_allow_origin(String)`
- `.set().access_control_allow_headers(String)`
- `.set().access_control_allow_methods(&str)`
- `.set().access_control_max_age(String)`

### Clone Required
`MultiCors` ต้อง implement `Clone` trait เพราะ `FangAction` ต้องการ
