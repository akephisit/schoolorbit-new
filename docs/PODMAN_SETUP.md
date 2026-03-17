# 📘 คู่มือติดตั้ง Server (Podman Edition)

**Technology Stack:** Debian/Ubuntu + Podman + Cockpit + Nginx + Cloudflare
**Goal:** ติดตั้งระบบ Production-grade ที่ปลอดภัย เบา และจัดการง่ายด้วย GUI มาตรฐาน Linux

---

### 📌 STEP 1 — อัปเดตระบบ & ติดตั้ง Tools

SSH เข้า Server แล้วรันคำสั่งทีละบรรทัด:

```bash
# 1. อัปเดต Package ทั้งหมด
sudo apt update && sudo apt upgrade -y

# 2. ติดตั้ง Podman, Compose และ UI (Cockpit)
# cockpit-podman คือ plugin ที่ทำให้เราจัดการ container ผ่านเว็บได้
sudo apt install -y podman podman-compose cockpit cockpit-podman

# 3. เปิดใช้งาน Cockpit (UI จัดการ Server)
sudo systemctl enable --now cockpit.socket

# 4. อนุญาตให้ Podman เปิด Port 80/443 ได้ 
# (ปกติ Linux ห้าม User ธรรมดาเปิด Port ต่ำกว่า 1024 เพื่อความปลอดภัย)
echo 'net.ipv4.ip_unprivileged_port_start=80' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

---

### 📌 STEP 2 — สร้างโครงสร้างโฟลเดอร์

เราจะเก็บทุกอย่างไว้ใน `/opt/stack` และเปลี่ยนสิทธิ์ให้ User เราเป็นเจ้าของ

```bash
# สร้างโฟลเดอร์สำหรับ Nginx และ SSL
sudo mkdir -p /opt/stack/nginx/conf.d
sudo mkdir -p /opt/stack/nginx/ssl
sudo mkdir -p /opt/stack/nginx/certbot

# เปลี่ยนเจ้าของโฟลเดอร์ให้เป็น User ปัจจุบัน (แทน root)
# เพื่อให้ Podman ที่รันโดย User เรา สามารถเขียนไฟล์ได้
sudo chown -R $USER:$USER /opt/stack

# เข้าไปที่โฟลเดอร์ทำงาน
cd /opt/stack
```

---

### 📌 STEP 3 — สร้าง compose.yml

ไฟล์นี้คือหัวใจสำคัญ ใช้จัดการ Nginx Service (Podman สามารถอ่านไฟล์ `docker-compose.yml` ได้ แต่แนะนำให้ใช้ชื่อ `compose.yml` หรือ `podman-compose.yml`)

สร้างไฟล์:
```bash
nano compose.yml
```

วางเนื้อหา:
```yaml
version: '3.8'

services:
  nginx:
    # ใช้ Nginx Alpine เพื่อความเล็กและเร็ว
    image: docker.io/library/nginx:stable-alpine
    container_name: schoolorbit-nginx
    restart: always # ให้เริ่มทำงานใหม่เสมอถ้ามันตาย
    ports:
      - "80:80"   # HTTP
      - "443:443" # HTTPS
    volumes:
      # Mount config จาก Host เข้าไปใน Container
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
      # พื้นที่สำหรับเก็บ SSL Certificate
      - ./nginx/ssl:/etc/letsencrypt
      - ./nginx/certbot:/var/www/certbot
    networks:
      - web

networks:
  web:
    driver: bridge
```
*(กด Ctrl+O -> Enter -> Ctrl+X เพื่อบันทึก)*

---

### 📌 STEP 4 — สร้าง Config Nginx แรกเริ่ม

สร้างไฟล์ config ตัวแรกเพื่อใช้ยืนยันตัวตนกับ Let's Encrypt (Certbot)

สร้างไฟล์:
```bash
nano /opt/stack/nginx/conf.d/default.conf
```

วางเนื้อหา:
```nginx
server {
    listen 80;
    server_name _;  # รับทุก Domain ที่เข้ามา

    # ส่วนสำคัญ! ให้ Certbot เข้ามาตรวจสอบไฟล์ Challenge ที่นี่
    location /.well-known/acme-challenge/ {
        root /var/www/certbot;
    }

    # Redirect ไปด่า (ล้อเล่น) หรือจัดการอย่างอื่นถ้าไม่ใช่ challenge
    location / {
        return 200 "Nginx on Podman is running!";
    }
}
```

---

### 📌 STEP 5 — รัน Podman Compose

สั่งรัน Container:
```bash
cd /opt/stack
podman-compose up -d
```

ตรวจสอบว่ารันอยู่ไหม:
```bash
podman ps
```

---

### 📌 STEP 6 — เข้าใช้งาน UI (Cockpit)

ตอนนี้คุณสามารถจัดการ Server ทั้งเครื่องผ่านหน้าเว็บได้แล้ว

1.  เปิด Browser เข้าไปที่: `https://<IP-SERVER-ของ-คุณ>:9090`
    *   *(ถ้า Browser เตือนว่าไม่ปลอดภัย ให้กด Advanced -> Proceed ได้เลย เพราะเป็น Self-signed certificate ของ Cockpit เอง)*
2.  Login ด้วย **User/Password ของ Linux** (ที่ใช้ SSH เข้าไปนั่นแหละ)
3.  กดเมนู **"Podman Containers"** ด้านซ้าย
    *   คุณจะเห็น Nginx รันอยู่ สามารถดู Logs, CPU, RAM ได้ทันที

    > **⚠️ ปัญหา Login Cockpit ไม่ได้?**
    > โดยปกติ Cockpit จะไม่อนุญาตให้ `root` ล็อกอินเพื่อความปลอดภัย
    >
    > **วิธีแก้ (อนุญาตให้ root เข้าใช้งาน):**
    > 1. แก้ไขไฟล์ `disallowed-users`:
    >    ```bash
    >    nano /etc/cockpit/disallowed-users
    >    ```
    > 2. ลบบรรทัดที่มีคำว่า `root` ออก แล้วบันทึกไฟล์ (Ctrl+O -> Enter -> Ctrl+X)
    > 3. จากนั้นสั่ง restart cockpit:
    >    ```bash
    >    systemctl restart cockpit
    >    ```
    > 4. ลอง login ใหม่อีกครั้งด้วย user `root` และรหัสผ่านที่ตั้งไว้

---

### 📌 STEP 7 — ตั้งค่า DNS Cloudflare

ไปที่ Cloudflare Dashboard และเพิ่ม **A Record** แยกตาม Service ที่คุณมี:

1.  **Backend School:**
    *   Name: `school-api` (เช่น `school-api.schoolorbit.app`)
    *   IPv4: `<IP-SERVER-ของ-คุณ>`
    *   Proxy: **OFF (สีเทา)**

2.  **Backend Admin (ถ้ามี):**
    *   Name: `admin-api` (เช่น `admin-api.schoolorbit.app`)
    *   IPv4: `<IP-SERVER-ของ-คุณ>`
    *   Proxy: **OFF (สีเทา)**

*(หลังจากทำ SSL เสร็จในขั้นต่อไปแล้ว ค่อยกลับมาเปิด Proxy เป็นสีส้มได้ครับ)*

---

### 📌 STEP 8 — ขอ SSL Certificate ฟรี (แบบ Multi-domain)

ใช้ Podman รัน Certbot เพื่อขอใบรับรองทีเดียวให้ครบทุก Domain

```bash
# แก้ -d ให้ครบทุก domain ที่คุณใช้
podman run --rm \
  -v /opt/stack/nginx/ssl:/etc/letsencrypt \
  -v /opt/stack/nginx/certbot:/var/www/certbot \
  docker.io/certbot/certbot certonly \
  --webroot -w /var/www/certbot \
  -d school-api.schoolorbit.app \
  -d admin-api.schoolorbit.app \
  --non-interactive \
  --agree-tos \
  -m kruakemaths@gmail.com
```

*   **-d:** ใส่เพิ่มได้เรื่อยๆ ถ้าคุณมีหลาย Subdomain

---

### 📌 STEP 9 — อัปเดต Nginx Config ของจริง (รองรับ SSE & Uploads)

เราจะสร้างไฟล์ Config แยกกันเพื่อให้จัดการง่าย หรือรวมไว้ไฟล์เดียวก็ได้ ในที่นี้แนะนำไฟล์เดียวแต่แยก Server Block

สร้างไฟล์:
```bash
nano /opt/stack/nginx/conf.d/schoolorbit.conf
```

**ตัวอย่าง Config (รองรับ SSE, File Upload, CORS แบบครบเครื่อง):**

```nginx
# --------------------------------------------------------
# 1. MAP SECTION: ประกาศกฎการอนุญาตที่นี่ที่เดียว
# --------------------------------------------------------
map $http_origin $allow_origin {
    default ""; # ค่าเริ่มต้นคือ "ไม่ให้เข้า"

    # ✅ กฎที่ 1: อนุญาต *.schoolorbit.app ทั้งหมด (Regex) และยอมรับ port อะไรก็ได้
    "~^https://([\w-]+\.)?schoolorbit\.app(:[0-9]+)?$" $http_origin;

    # ✅ กฎที่ 2: อนุญาต Localhost (เผื่อ Dev ในเครื่องตัวเอง)
    "http://localhost:3000" $http_origin;
    "http://127.0.0.1:3000" $http_origin;
}

# --------------------------------------------------------
# SERVER 1: SCHOOL API (backend-school)
# --------------------------------------------------------
server {
    listen 80;
    server_name school-api.schoolorbit.app;
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl;
    server_name school-api.schoolorbit.app;

    ssl_certificate /etc/letsencrypt/live/school-api.schoolorbit.app/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/school-api.schoolorbit.app/privkey.pem;

    # Global Performance Settings
    client_max_body_size 20M;
    proxy_read_timeout 300s;

    # Security Headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Vary Origin always;

# 🆕 SSE ENDPOINTS (Matches any path ending in /stream)
    location ~ /stream$ {
        proxy_pass http://schoolorbit-backend-school:8081;
        # SSE Optimization
        proxy_buffering off;
        proxy_cache off;
        proxy_http_version 1.1; # สำคัญสำหรับ Connection keep-alive
        proxy_set_header Connection "";
        chunked_transfer_encoding on;
        
        # Timeouts (Long polling needs long timeout)
        proxy_read_timeout 24h;
        proxy_send_timeout 24h;
        # Standard Proxy Headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        # CORS
        add_header 'Access-Control-Allow-Origin' $allow_origin always;
        add_header 'Access-Control-Allow-Credentials' 'true' always;
        add_header 'Access-Control-Allow-Methods' 'GET, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
        
        # Preflight
        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' $allow_origin always;
            add_header 'Access-Control-Allow-Credentials' 'true' always;
            add_header 'Access-Control-Allow-Methods' 'GET, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
            add_header 'Access-Control-Max-Age' 1728000;
            add_header 'Content-Type' 'text/plain; charset=utf-8';
            add_header 'Content-Length' 0;
            return 204;
        }
    }

    # 🆕 FILE UPLOAD (backend-school)
    location /api/files/ {
        proxy_pass http://schoolorbit-backend-school:8081;
        client_max_body_size 50M;
        proxy_request_buffering off;
        
        add_header 'Access-Control-Allow-Origin' $allow_origin always;
        add_header 'Access-Control-Allow-Credentials' 'true' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, OPTIONS' always;
    }

    # 🆕 WEBSOCKETS (TimeTable)
    location /ws/ {
        proxy_pass http://schoolorbit-backend-school:8081;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
        
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        add_header 'Access-Control-Allow-Origin' $allow_origin always;
    }

    # NORMAL API (backend-school)
    location / {
        proxy_pass http://schoolorbit-backend-school:8081;

        add_header 'Access-Control-Allow-Origin' $allow_origin always;
        add_header 'Access-Control-Allow-Credentials' 'true' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, PUT, PATCH, DELETE, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
        add_header 'Access-Control-Expose-Headers' 'Content-Length,Content-Range' always;

        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' $allow_origin always;
            add_header 'Access-Control-Allow-Credentials' 'true' always;
            add_header 'Access-Control-Allow-Methods' 'GET, POST, PUT, PATCH, DELETE, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
            add_header 'Access-Control-Max-Age' 1728000;
            add_header 'Content-Type' 'text/plain; charset=utf-8';
            add_header 'Content-Length' 0;
            return 204;
        }

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# ========================================================
# SERVER 2: ADMIN API (backend-admin)
# ========================================================
server {
    listen 80;
    server_name admin-api.schoolorbit.app;
    location /.well-known/acme-challenge/ { root /var/www/certbot; }
    location / { return 301 https://$host$request_uri; }
}

server {
    listen 443 ssl;
    server_name admin-api.schoolorbit.app;

    ssl_certificate /etc/letsencrypt/live/school-api.schoolorbit.app/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/school-api.schoolorbit.app/privkey.pem;

    # Global Timeouts
    proxy_read_timeout 300s;

    # 🆕 SSE ENDPOINTS (backend-admin)
    location ~ /stream$ {
        proxy_pass http://schoolorbit-backend-admin:8080;
        
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 24h;
        proxy_connect_timeout 60s;
        proxy_send_timeout 24h;
        chunked_transfer_encoding on;
        proxy_set_header Connection "";
        
        # CORS Headers from Map
        add_header 'Access-Control-Allow-Origin' $allow_origin always;
        add_header 'Access-Control-Allow-Credentials' 'true' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, DELETE, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;

        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' $allow_origin always;
            add_header 'Access-Control-Allow-Credentials' 'true' always;
            add_header 'Access-Control-Allow-Methods' 'GET, POST, DELETE, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
            return 204;
        }
        
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # NORMAL API (backend-admin)
    location / {
        proxy_pass http://schoolorbit-backend-admin:8080;
        
        add_header 'Access-Control-Allow-Origin' $allow_origin always;
        add_header 'Access-Control-Allow-Credentials' 'true' always;
        add_header 'Access-Control-Allow-Methods' 'GET, POST, PUT, PATCH, DELETE, OPTIONS' always;
        add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;

        if ($request_method = 'OPTIONS') {
            add_header 'Access-Control-Allow-Origin' $allow_origin always;
            add_header 'Access-Control-Allow-Credentials' 'true' always;
            add_header 'Access-Control-Allow-Methods' 'GET, POST, PUT, PATCH, DELETE, OPTIONS' always;
            add_header 'Access-Control-Allow-Headers' 'DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization' always;
            add_header 'Access-Control-Max-Age' 1728000;
            add_header 'Content-Type' 'text/plain; charset=utf-8';
            add_header 'Content-Length' 0;
            return 204;
        }

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

บันทึกไฟล์ แล้วสั่ง Reload Nginx:
```bash
podman exec schoolorbit-nginx nginx -s reload
```

---

### 📌 STEP 10 — ตั้งเวลาต่ออายุอัตโนมัติ (Auto Renew)

Linux มีระบบตั้งเวลาชื่อ `cron` ใช้ตัวนี้สั่งงานให้เราทุกวัน

> **⚠️ ถ้าพิมพ์ `crontab -e` แล้วเจอ `command not found`?**
> ให้ติดตั้ง cron ก่อนครับ:
> ```bash
> sudo apt update && sudo apt install -y cron
> sudo systemctl enable --now cron
> ```

พิมพ์คำสั่งเพื่อแก้ไขตารางเวลา:
```bash
crontab -e
```

ไปบรรทัดสุดท้าย แล้วเพิ่ม:
```bash
# ตรวจสอบทุกวัน เวลาตี 3
0 3 * * * podman run --rm -v /opt/stack/nginx/ssl:/etc/letsencrypt -v /opt/stack/nginx/certbot:/var/www/certbot certbot/certbot renew --quiet --deploy-hook "podman exec schoolorbit-nginx nginx -s reload"
```
*(คำสั่งนี้จะ Renew เฉพาะเมื่อใกล้หมดอายุ และถ้า Renew สำเร็จจะสั่ง Reload Nginx ให้ทันที)*

---

### 🔥 การเชื่อมต่อ Backend (App) กับ Nginx

**แนะนำ:** ให้ใช้รูปแบบ **"Single Stack"** คือจับ Backend มารวมในไฟล์ `compose.yml` ของ Nginx เลย เพื่อความง่ายในการจัดการ Network

ให้แก้ไขไฟล์ `/opt/stack/compose.yml` เพิ่ม Service Backend เข้าไป:

```yaml
version: '3.8'

services:
  # --------------------
  # 1. FRONT DOOR (NGINX)
  # --------------------
  nginx:
    image: docker.io/library/nginx:stable-alpine
    container_name: schoolorbit-nginx
    restart: always
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx/conf.d:/etc/nginx/conf.d:ro
      - ./nginx/ssl:/etc/letsencrypt
      - ./nginx/certbot:/var/www/certbot
    networks:
      - web

  # --------------------
  # 2. BACKEND SERVICES
  # --------------------
  backend-school:
    image: ghcr.io/akephisit/schoolorbit-backend-school:latest
    container_name: schoolorbit-backend-school
    restart: unless-stopped
    ports:
      - "8081:8081"
    environment:
      # Admin database for school mapping (required!)
      - ADMIN_DATABASE_URL=${ADMIN_DATABASE_URL}
      # Secrets
      - JWT_SECRET=${JWT_SECRET}
      - INTERNAL_API_SECRET=${INTERNAL_API_SECRET}
      - DEPLOY_KEY=${DEPLOY_KEY:-local-dev-key-change-me}
      - ENCRYPTION_KEY=${ENCRYPTION_KEY}
      - DB_USER=${DB_USER}
      # Server config
      - RUST_LOG=${RUST_LOG:-info}
      - HOST=${HOST:-0.0.0.0}
      - PORT=${SCHOOL_PORT:-8081}
      # Cloudflare R2 Configuration (required for file uploads)
      - R2_ACCOUNT_ID=${R2_ACCOUNT_ID}
      - R2_ACCESS_KEY_ID=${R2_ACCESS_KEY_ID}
      - R2_SECRET_ACCESS_KEY=${R2_SECRET_ACCESS_KEY}
      - R2_BUCKET_NAME=${R2_BUCKET_NAME:-schoolorbit-files}
      - R2_PUBLIC_URL=${R2_PUBLIC_URL}
      - R2_REGION=${R2_REGION:-auto}
      - AWS_REGION=auto
      # Optional: CDN URL for file serving
      - CDN_URL=${CDN_URL}
      # File Upload Limits
      - MAX_FILE_SIZE_MB=${MAX_FILE_SIZE_MB:-10}
      - MAX_PROFILE_IMAGE_SIZE_MB=${MAX_PROFILE_IMAGE_SIZE_MB:-5}
      - MAX_DOCUMENT_SIZE_MB=${MAX_DOCUMENT_SIZE_MB:-20}
      # Allowed File Types
      - ALLOWED_IMAGE_TYPES=${ALLOWED_IMAGE_TYPES:-jpg,jpeg,png,webp,gif}
      - ALLOWED_DOCUMENT_TYPES=${ALLOWED_DOCUMENT_TYPES:-pdf,doc,docx,xls,xlsx}
      # Web Push Configuration
      - VAPID_PUBLIC_KEY=${VAPID_PUBLIC_KEY}
      - VAPID_PRIVATE_KEY=${VAPID_PRIVATE_KEY}
      - VAPID_SUBJECT=${VAPID_SUBJECT:-mailto:kruakemaths@gmail.com}
      # Note: DATABASE_URL is no longer needed (multi-tenant architecture)
      # Note: ALLOWED_ORIGINS is only for backend-admin
    networks:
      - web
    healthcheck:
      test: [ "CMD", "curl", "-f", "http://localhost:8081/health" ]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  backend-admin:
    image: ghcr.io/akephisit/schoolorbit-backend-admin:latest
    container_name: schoolorbit-backend-admin
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      # Database
      - DATABASE_URL=${DATABASE_URL}
      - JWT_SECRET=${JWT_SECRET}

      # Neon API (for tenant database provisioning)
      - NEON_API_KEY=${NEON_API_KEY}
      - NEON_PROJECT_ID=${NEON_PROJECT_ID}
      - NEON_BRANCH_ID=${NEON_BRANCH_ID:-main}
      - NEON_HOST=${NEON_HOST}
      - NEON_DB_PASSWORD=${NEON_DB_PASSWORD}

      # Cloudflare (for Workers deployment & DNS)
      - CLOUDFLARE_API_TOKEN=${CLOUDFLARE_API_TOKEN}
      - CLOUDFLARE_ZONE_ID=${CLOUDFLARE_ZONE_ID}
      - CLOUDFLARE_ACCOUNT_ID=${CLOUDFLARE_ACCOUNT_ID}
      - BASE_DOMAIN=${BASE_DOMAIN:-schoolorbit.app}

      # Backend-School Communication
      - BACKEND_SCHOOL_URL=${BACKEND_SCHOOL_URL:-http://backend-school:8081}
      - INTERNAL_API_SECRET=${INTERNAL_API_SECRET}

      # Deployment (GitHub Actions)
      - API_URL=${API_URL}
      - GITHUB_TOKEN=${GITHUB_TOKEN}
      - GITHUB_REPO=${GITHUB_REPO:-akephisit/schoolorbit-new}

      # General
      - ALLOWED_ORIGINS=${ALLOWED_ORIGINS}
      - RUST_LOG=${RUST_LOG:-info}
      - PORT=${ADMIN_PORT:-8080}
    networks:
      - web
    healthcheck:
      test: [ "CMD", "curl", "-f", "http://localhost:8080/health" ]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

networks:
  web:
    driver: bridge
```

เมื่อแก้ไขเสร็จ ให้เตรียมไฟล์ Environment Variables:
```bash
nano /opt/stack/.env
```
*(นำค่า Config ทั้งหมด เช่น DATABASE_URL, JWT_SECRET, PORT มาวางที่นี่)*

จากนั้นสั่ง Update Stack:
```bash
cd /opt/stack
podman-compose up -d
```
จากนั้น Nginx จะมองเห็น Backend ทันที โดยไม่ต้อง Reload ใหม่ (แต่ถ้า Reload อีกรอบเพื่อความชัวร์ก็ได้)

---
✅ **เสร็จสมบูรณ์!** ตอนนี้ Web Server ของคุณพร้อมใช้งานแบบ Secure และ Manage ง่ายแล้วครับ
