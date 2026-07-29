# คู่มือติดตั้ง SchoolOrbit ด้วย Podman

คู่มือนี้ใช้สำหรับเตรียม production server ใหม่บน Debian/Ubuntu ด้วย Podman แบบ rootless โดยให้ repository เป็นเจ้าของ Compose และ Nginx configuration ที่ใช้งานจริง หากเป็นการดูแลระบบที่ติดตั้งแล้ว ให้ใช้ [Operations](./OPERATIONS.md) แทน

## ขอบเขตและโครงสร้างระบบ

ระบบ production ประกอบด้วย:

- `schoolorbit-backend-admin` ที่พอร์ต `8080` สำหรับ control plane และ admin database;
- `schoolorbit-backend-school` ที่พอร์ต `8081` สำหรับ tenant API และ tenant databases;
- `schoolorbit-nginx` ที่รับ HTTP/HTTPS แล้ว proxy ไปยัง backend ผ่าน Podman network;
- PostgreSQL/Neon ภายนอก server สำหรับ admin และ tenant databases;
- Cloudflare R2-compatible storage สำหรับไฟล์;
- frontend ที่ deploy แยกจาก backend ตาม tenant/subdomain.

[`podman-compose.yml`](../podman-compose.yml) เป็น source of truth ของ backend containers ห้ามคัดลอก Compose ทั้งไฟล์ไปไว้ในคู่มือนี้แล้วแก้แยกกัน

## 1. เตรียม Server

ใช้ Debian/Ubuntu รุ่นที่ยังได้รับ security updates และบัญชีผู้ดูแลปกติที่ใช้ `sudo` ได้:

1. ตั้งค่า SSH key และทดสอบ session ใหม่ก่อนปิด password login.
2. อัปเดตระบบและ reboot หาก kernel/package manager แจ้งว่าจำเป็น.
3. เปิด firewall สำหรับ SSH ก่อนเสมอ จากนั้นเปิด `80/tcp` และ `443/tcp`.
4. พอร์ต `8080` และ `8081` ควรถูกจำกัดด้วย host firewall ไม่ให้เข้าจากอินเทอร์เน็ตโดยตรง.
5. เปิด `9090/tcp` สำหรับ Cockpit เฉพาะ trusted IP/VPN หากต้องใช้ GUI.

```bash
sudo apt update
sudo apt upgrade -y
```

อย่ารัน application stack ด้วยบัญชี `root` และอย่าเปิดให้บัญชี `root` เข้า Cockpit ใช้ service user ปกติที่มี `sudo` เท่าที่จำเป็น

## 2. ติดตั้ง Podman และเครื่องมือ

```bash
sudo apt install -y \
  podman \
  podman-compose \
  cockpit \
  cockpit-podman \
  git \
  curl \
  ca-certificates

sudo systemctl enable --now cockpit.socket
sudo loginctl enable-linger "$USER"

podman --version
podman-compose version
```

Cockpit เปิดที่ `https://<server-ip>:9090` และใช้บัญชี Linux ปกติเดียวกับที่เป็นเจ้าของ rootless containers.

หาก rootless Nginx ต้อง bind พอร์ต `80`/`443` โดยตรง ต้องลด privileged-port boundary ของทั้งเครื่อง การตั้งค่านี้เหมาะกับ dedicated server เท่านั้น:

```bash
printf '%s\n' 'net.ipv4.ip_unprivileged_port_start=80' \
  | sudo tee /etc/sysctl.d/99-schoolorbit-rootless-ports.conf
sudo sysctl --system
```

หากนโยบายเครื่องไม่อนุญาต ให้ใช้ host-managed reverse proxy หรือ firewall port forwarding แทน อย่าเปลี่ยน container ทั้ง stack เป็น privileged เพื่อหลบข้อจำกัดนี้

## 3. เตรียม `/opt/stack`

GitHub deployment workflows ปัจจุบัน SSH เข้า server แล้วรันคำสั่งจาก `/opt/stack` ดังนั้น path นี้ต้องมี `podman-compose.yml` และ `.env`.

ติดตั้งใหม่:

```bash
sudo install -d -m 0750 -o "$USER" -g "$(id -gn)" /opt/stack
git clone git@github.com:akephisit/schoolorbit-new.git /opt/stack
cd /opt/stack
```

ถ้า repository เป็น private ให้เพิ่ม deploy key/read access ให้ service user ก่อน clone ห้ามฝัง GitHub token ใน clone URL หรือ shell history.

กรณี `/opt/stack` มี repository อยู่แล้ว:

```bash
cd /opt/stack
git status --short
git pull --ff-only
```

หยุดและตรวจสอบก่อน `git pull` หากมี local changes อย่า reset หรือทับ `.env`, certificate หรือไฟล์ production ที่ยังไม่ได้สำรอง

เตรียมพื้นที่ของ reverse proxy:

```bash
install -d -m 0750 \
  /opt/stack/nginx/conf.d \
  /opt/stack/nginx/ssl \
  /opt/stack/nginx/certbot
```

## 4. เตรียม Environment Variables

สร้างไฟล์จาก template แล้วจำกัดสิทธิ์:

```bash
cd /opt/stack
umask 077
cp .env.example .env
chmod 600 .env
```

แก้ `/opt/stack/.env` และแทนค่าตัวอย่างทั้งหมดด้วยค่าจาก secret manager ของ production กลุ่มสำคัญคือ:

- security: `JWT_SECRET`, `INTERNAL_API_SECRET`, `DEPLOY_KEY`;
- encryption: `ENCRYPTION_KEY`, `BLIND_INDEX_KEY`;
- internal services: `BACKEND_ADMIN_URL`, `BACKEND_SCHOOL_URL` และ timeout/retry variables;
- admin database/Neon: `DATABASE_URL`, `NEON_API_KEY`, `NEON_PROJECT_ID`, `NEON_BRANCH_ID`, `NEON_HOST`, `NEON_DB_PASSWORD`;
- deployment: `API_URL`, Cloudflare variables และ GitHub variables;
- File Platform: `R2_PUBLIC_BUCKET_NAME`, `R2_PRIVATE_BUCKET_NAME`, `R2_PUBLIC_URL`, credentials, `CLAMD_*`, `FILE_PRIVATE_GRANT_TTL_SECONDS` และ `FILE_RECONCILE_*`;
- notifications: `VAPID_PUBLIC_KEY`, `VAPID_PRIVATE_KEY`, `VAPID_SUBJECT`.

ข้อกำหนดสำคัญ:

- `INTERNAL_API_SECRET` ต้องตรงกันระหว่าง backend ทั้งสอง;
- `DEPLOY_KEY` ต้องตรงกับ server-only `DEPLOY_KEY` ที่ GitHub tenant deployment
  ใช้ในขั้น `Synchronize menu routes`; ห้ามส่ง key นี้เข้า Vite build หรือ Worker variables;
- `ENCRYPTION_KEY` และ `BLIND_INDEX_KEY` ต้องสำรองอย่างปลอดภัยและคงเดิมหลังมีข้อมูลแล้ว การเปลี่ยน key ต้องทำผ่านงาน re-encryption/reindex ที่ตรวจสอบแยกต่างหาก;
- ห้าม commit `.env`, แสดงค่า secret ใน log หรือส่งไฟล์นี้ผ่านช่องทางสนทนา;
- ใช้ URL ของ container/service บน network เดียวกัน ห้ามใช้ `localhost` เพื่อให้ container หนึ่งเรียกอีก container หนึ่ง.
- public/private R2 bucket ต้องเป็นคนละ bucket และ private bucket ห้ามเปิด public domain หรือ `r2.dev`;
- `CLAMD_ENDPOINT` ใช้ `clamd:3310`; Compose ไม่ publish port นี้ออกสู่ host และ backend-school จะไม่ ready จน scanner กับทั้งสอง bucket พร้อม.

ตรวจ syntax โดยไม่พิมพ์ค่าที่ resolve แล้วออกหน้าจอ:

```bash
cd /opt/stack
podman-compose -f podman-compose.yml config >/dev/null
```

## 5. เตรียม Nginx, DNS และ TLS

### DNS

สร้าง DNS records ของ API domains ให้ชี้มาที่ server เช่น:

- `school-api.schoolorbit.app`;
- admin API domain ที่เลือกใช้จริง.

เลือกเปิดหรือปิด Cloudflare proxy ตาม TLS, WebSocket, SSE และ caching policy ของ environment นั้น อย่าถือว่า proxy mode แบบใดถูกต้องกับทุกระบบ.

### TLS ครั้งแรก

ก่อนเริ่ม Nginx ขอ certificate ด้วย ACME client ที่องค์กรอนุมัติ ตัวอย่าง Certbot แบบ standalone:

```bash
podman run --rm -it \
  -p 80:80 \
  -v /opt/stack/nginx/ssl:/etc/letsencrypt \
  docker.io/certbot/certbot certonly \
  --standalone \
  -d school-api.schoolorbit.app \
  -d admin-api.schoolorbit.app
```

DNS ต้อง resolve มายัง server และพอร์ต `80` ต้องว่างระหว่างรัน เปลี่ยน domains ให้ตรง production จริง ตั้ง automated renewal และทดสอบ renewal ก่อน certificate หมดอายุ.

### Nginx configuration

ใช้ไฟล์ repository เป็น reference:

- [school-api.schoolorbit.app.conf](../nginx-configs/school-api.schoolorbit.app.conf) สำหรับ school API, uploads, SSE และ WebSocket;
- [admin-api.schoolorbit.app.conf](../nginx-configs/admin-api.schoolorbit.app.conf) สำหรับ admin API.

ตรวจ server names, CORS origins, certificate paths และ upload limits ก่อนนำไปไว้ใน `/opt/stack/nginx/conf.d`. เมื่อ Nginx รันเป็น container บน network เดียวกัน `proxy_pass` ต้องใช้ `schoolorbit-backend-school:8081` หรือ `schoolorbit-backend-admin:8080` ไม่ใช่ `localhost`.

อย่าให้ access log บันทึก token, cookie, national ID, raw query string หรือ WebSocket identity. คง `access_log off` สำหรับ `/ws/` ตาม reference ปัจจุบัน.

## 6. เริ่ม Backend Services

Login GHCR ด้วย credential ที่อ่าน package ได้ แล้ว validate/pull/start:

```bash
cd /opt/stack
podman login ghcr.io
podman-compose -f podman-compose.yml config >/dev/null
podman-compose -f podman-compose.yml pull
podman-compose -f podman-compose.yml up -d
podman ps
```

ชื่อ network จริงอาจถูก `podman-compose` เติม project prefix ให้ตรวจจาก backend container แทนการเดา:

```bash
schoolorbit_podman_network="$(
  podman inspect schoolorbit-backend-school \
    --format '{{range $network, $_ := .NetworkSettings.Networks}}{{$network}}{{"\n"}}{{end}}' \
    | head -n 1
)"
test -n "$schoolorbit_podman_network"
printf 'SchoolOrbit Podman network: %s\n' "$schoolorbit_podman_network"
podman network inspect "$schoolorbit_podman_network" >/dev/null
```

ตรวจ Nginx config บน network เดียวกับ backend:

```bash
podman run --rm \
  --network "$schoolorbit_podman_network" \
  -v /opt/stack/nginx/conf.d:/etc/nginx/conf.d:ro \
  -v /opt/stack/nginx/ssl:/etc/letsencrypt:ro \
  docker.io/library/nginx:stable-alpine \
  nginx -t
```

จากนั้นเริ่ม Nginx:

```bash
podman run -d \
  --name schoolorbit-nginx \
  --restart unless-stopped \
  --network "$schoolorbit_podman_network" \
  -p 80:80 \
  -p 443:443 \
  -v /opt/stack/nginx/conf.d:/etc/nginx/conf.d:ro \
  -v /opt/stack/nginx/ssl:/etc/letsencrypt:ro \
  -v /opt/stack/nginx/certbot:/var/www/certbot:ro \
  docker.io/library/nginx:stable-alpine
```

ถ้ามี `schoolorbit-nginx` อยู่แล้ว อย่ารันคำสั่งสร้างซ้ำ ให้ตรวจ config ด้วย `nginx -t` แล้ว reload:

```bash
podman exec schoolorbit-nginx nginx -t
podman exec schoolorbit-nginx nginx -s reload
```

## 7. ตรวจสอบหลังติดตั้ง

ตรวจ container, network และ log แบบจำกัดจำนวนบรรทัด:

```bash
podman ps
podman network inspect "$schoolorbit_podman_network"
podman logs --tail 100 schoolorbit-backend-admin
podman logs --tail 100 schoolorbit-backend-school
podman logs --tail 100 schoolorbit-nginx
```

ตรวจ liveness และ readiness จาก host:

```bash
curl -fsS http://127.0.0.1:8080/health
curl -fsS http://127.0.0.1:8080/ready
curl -fsS http://127.0.0.1:8081/health
curl -fsS http://127.0.0.1:8081/ready
```

- `/health` ยืนยันว่า process ยังทำงาน;
- `/ready` ยืนยัน dependency readiness และเป็น endpoint ที่ deployment workflow ใช้ตัดสินความสำเร็จ.

จากเครื่องภายนอกให้ตรวจ API domains ผ่าน HTTPS แล้วรัน [`scripts/smoke_test.sh`](../scripts/smoke_test.sh) ตาม [Testing](./TESTING.md). อย่าใส่ credentials ลง command history หาก environment มีวิธีโหลด secret file ที่ปลอดภัยกว่า.

## 8. เชื่อมต่อ GitHub Deployment

Backend workflows ปัจจุบัน:

- [deploy-backend-admin.yml](../.github/workflows/deploy-backend-admin.yml);
- [deploy-backend-school.yml](../.github/workflows/deploy-backend-school.yml).

Repository/organization secrets ต้องมีอย่างน้อย `SERVER_IP`, `SERVER_USER` และ `SSH_PRIVATE_KEY`; package login ใช้ GitHub token ภายใน workflow. Service user บน server ต้อง:

- SSH ด้วย key ได้;
- ใช้ rootless Podman และอ่าน `/opt/stack/.env` ได้;
- มี `/opt/stack/podman-compose.yml`;
- login `ghcr.io` ได้;
- จัดการ containers ทั้งสามตามชื่อที่กำหนด;
- เรียก readiness ที่ `127.0.0.1:8080/ready` และ `127.0.0.1:8081/ready` ได้.

Workflow จะ pull image, recreate backend ที่เกี่ยวข้อง, รอ `/ready`, เก็บ log เมื่อ readiness ล้มเหลว และ reload `schoolorbit-nginx`.

## 9. อัปเดตและย้อนกลับ

อัปเดตด้วยตนเอง:

```bash
cd /opt/stack
git pull --ff-only
podman-compose -f podman-compose.yml config >/dev/null
podman-compose -f podman-compose.yml pull
podman-compose -f podman-compose.yml up -d
```

จากนั้นตรวจ `/ready` ทั้งสอง service และ smoke test ก่อนลบ image เก่า.

ก่อน deploy ให้บันทึก image digest/commit SHA ที่ใช้งานอยู่:

```bash
podman image inspect \
  ghcr.io/akephisit/schoolorbit-backend-admin:latest \
  --format '{{.Digest}}'
podman image inspect \
  ghcr.io/akephisit/schoolorbit-backend-school:latest \
  --format '{{.Digest}}'
```

หากต้อง rollback ให้ pull SHA tag ที่ workflow เคย publish, tag เป็น `latest` ภายใน server, recreate เฉพาะ service ที่มีปัญหา แล้วตรวจ `/ready` อีกครั้ง อย่าลบ database, volume, `/opt/stack/.env`, encryption keys หรือ certificate ระหว่าง rollback.

## 10. แก้ปัญหาเบื้องต้น

- `podman-compose config` ล้มเหลว: ตรวจตัวแปรที่ขาดใน `.env` โดยไม่พิมพ์ค่า secret.
- backend-admin ไม่ ready: ตรวจ `DATABASE_URL`, Neon connectivity และ log 100 บรรทัดท้าย.
- backend-school ไม่ ready: ตรวจ backend-admin readiness, `BACKEND_ADMIN_URL`, internal secret และ container network.
- Nginx ได้ `502`: ตรวจว่า Nginx อยู่ network เดียวกับ backend และ `proxy_pass` ใช้ container name.
- TLS ล้มเหลว: ตรวจ DNS, certificate path, สิทธิ์อ่าน volume และ `podman exec schoolorbit-nginx nginx -t`.
- WebSocket/SSE หลุด: ตรวจ upgrade headers, buffering, timeout และ proxy/CDN policy.
- upload ล้มเหลว: ตรวจ R2 variables, bucket access, request-size limits และ timeout.
- container ไม่กลับมาหลัง reboot: ตรวจ linger ของ service user, restart policy และ rootless Podman user services.

หลีกเลี่ยงการแก้โดยลบ container/volume แบบครอบจักรวาล ให้แก้จาก readiness/log และสำรองข้อมูลก่อนทำการเปลี่ยนแปลงที่ย้อนกลับยาก

## เอกสารที่เกี่ยวข้อง

- [Operations](./OPERATIONS.md)
- [Testing](./TESTING.md)
- [Development rules](../.rules)
- [Repository overview](../README.md)
- [Production Podman Compose](../podman-compose.yml)
