# 🔧 Nginx Configuration Update for File Upload

## ปัญหาที่พบ
- **502 Bad Gateway** เมื่ออัปโหลดไฟล์
- Nginx ไม่มี `client_max_body_size` (default = 1MB)
- ไม่มี timeout settings สำหรับการอัปโหลดไฟล์ขนาดใหญ่

## การแก้ไข

### ✅ **สิ่งที่เพิ่มเข้ามา**

#### 1. Global Upload Settings (ใน server block)
```nginx
client_max_body_size 20M;           # เพิ่ม limit เป็น 20MB
client_body_timeout 300s;           # Timeout 5 นาที
client_header_timeout 300s;
proxy_connect_timeout 300s;
proxy_send_timeout 300s;
proxy_read_timeout 300s;
```

#### 2. File Upload Specific Location (เพิ่มใหม่)
```nginx
location /api/files/ {
    # รองรับ /api/files/upload, /api/files/:id ทั้งหมด
    client_max_body_size 20M;
    proxy_request_buffering off;    # ปิด buffering เพื่อ stream upload
    proxy_http_version 1.1;
    # ... CORS headers
}
```

#### 3. Proxy Headers สำหรับ Upload
```nginx
proxy_set_header Content-Length $content_length;  # สำคัญ!
proxy_set_header X-Forwarded-Host $server_name;
```

---

## 📋 วิธีนำไปใช้บน VPS

### Step 1: Backup Config เดิม
```bash
sudo cp /etc/nginx/sites-enabled/school-api.schoolorbit.app \
       /etc/nginx/sites-enabled/school-api.schoolorbit.app.backup
```

### Step 2: แก้ไข Config
```bash
sudo nano /etc/nginx/sites-enabled/school-api.schoolorbit.app
```

**หรือ** คัดลอกจากไฟล์:
```bash
sudo cp nginx-configs/school-api.schoolorbit.app.conf \
       /etc/nginx/sites-enabled/school-api.schoolorbit.app
```

### Step 3: ทดสอบ Syntax
```bash
sudo nginx -t
```

ต้องเห็น:
```
nginx: configuration file /etc/nginx/nginx.conf test is successful
```

### Step 4: Reload Nginx
```bash
sudo systemctl reload nginx
```

หรือ (ถ้า reload ไม่ได้):
```bash
sudo systemctl restart nginx
```

### Step 5: ตรวจสอบ Status
```bash
sudo systemctl status nginx
```

---

## 🧪 ทดสอบหลัง Update

### Test 1: ตรวจสอบ CORS
```bash
curl -I -X OPTIONS https://school-api.schoolorbit.app/api/files/upload \
  -H "Origin: https://school.schoolorbit.app" \
  -H "Access-Control-Request-Method: POST"
```

ควรได้:
```
HTTP/2 204
access-control-allow-origin: https://school.schoolorbit.app
access-control-allow-credentials: true
```

### Test 2: ทดสอบ Upload ไฟล์เล็ก
```bash
# สร้างไฟล์ทดสอบ 100KB
dd if=/dev/zero of=/tmp/test.jpg bs=1024 count=100

# ทดสอบ upload
curl -X POST https://school-api.schoolorbit.app/api/files/upload \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -F "file=@/tmp/test.jpg" \
  -F "file_type=profile_image"
```

### Test 3: ทดสอบไฟล์ใหญ่ (5MB)
```bash
dd if=/dev/zero of=/tmp/test-large.jpg bs=1024 count=5120
curl -X POST https://school-api.schoolorbit.app/api/files/upload \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -F "file=@/tmp/test-large.jpg" \
  -F "file_type=document"
```

---

## 🔍 Troubleshooting

### ถ้ายังได้ 502 Bad Gateway
1. **ตรวจสอบ Backend รันอยู่หรือไม่**
   ```bash
   sudo docker ps | grep backend-school
   ```

2. **ดู Nginx Error Log**
   ```bash
   sudo tail -f /var/log/nginx/error.log
   ```

3. **ดู Backend Log**
   ```bash
   sudo docker logs schoolorbit-backend-school --tail 50
   ```

### ถ้าได้ 413 Payload Too Large
- เพิ่ม `client_max_body_size` มากขึ้น (เช่น 50M)
- Restart nginx แทน reload

### ถ้าได้ 504 Gateway Timeout
- เพิ่ม `proxy_read_timeout` มากขึ้น (เช่น 600s)

---

## 📊 Comparison: Before vs After

| Setting | Before | After | เปลี่ยนอะไร |
|---------|--------|-------|-------------|
| Max Upload Size | 1MB (default) | 20MB | ✅ +1900% |
| Upload Timeout | 60s (default) | 300s | ✅ +400% |
| Proxy Buffering | On | Off (for `/api/files/`) | ✅ Stream mode |
| Content-Length Header | ❌ ไม่ส่ง | ✅ ส่ง | ✅ Backend รู้ขนาดไฟล์ |
| CORS for Upload | ✅ (จาก location /) | ✅ (specific) | ✅ รองรับดีขึ้น |

---

## ⚙️ Configuration Details

### File Size Limits แยกตาม Location

| Endpoint | Max Size | เหตุผล |
|----------|----------|--------|
| `/api/files/*` | 20MB | Upload endpoint หลัก |
| `/` (other APIs) | 5MB | API ปกติอาจมีการส่งข้อมูล |
| Global | 20MB | Fallback |

### Timeout Settings

| Type | Value | ใช้เมื่อ |
|------|-------|---------|
| `client_body_timeout` | 300s | Client อ่านข้อมูลช้า |
| `proxy_read_timeout` | 300s | Backend ประมวลผลนาน |
| `proxy_send_timeout` | 300s | ส่งข้อมูลไป backend ช้า |

---

## 🎯 Best Practices

1. **Monitoring**: ติดตาม error logs หลัง deploy
   ```bash
   sudo tail -f /var/log/nginx/error.log | grep "client_max_body_size"
   ```

2. **Metrics**: ดู request size distribution
   ```bash
   sudo tail -1000 /var/log/nginx/access.log | awk '{print $10}' | sort -n
   ```

3. **Security**: จำกัด upload ตาม authentication
   - Backend ต้อง validate token
   - ตรวจสอบ file type และ content

4. **Performance**: 
   - ใช้ CDN สำหรับ static files
   - Enable gzip compression (ถ้ายังไม่มี)

---

## 📝 Notes

- Config นี้ใช้ Docker service name `schoolorbit-backend-school` (ถ้าชื่อต่างให้แก้)
- Port 8081 คือ backend-school port (ถ้าต่างให้แก้)
- CORS map `$allow_origin` ต้องมีอยู่แล้วใน config
- SSL certificates ต้อง valid และ auto-renew

---

## 🚀 Next Steps

1. ✅ Update nginx config
2. ✅ Test upload functionality
3. ⏳ Monitor logs for errors
4. ⏳ Adjust limits based on usage
5. ⏳ Add rate limiting (optional)
6. ⏳ Setup monitoring/alerting

---

**Last Updated:** 2026-01-10
**Author:** Antigravity AI Assistant
