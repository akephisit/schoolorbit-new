# SchoolOrbit — Project Improvement Analysis

วันที่วิเคราะห์: 21 กรกฎาคม 2026  
ขอบเขต: `backend-school`, `backend-admin`, `frontend-school`, `frontend-admin`, migrations, tests, CI/CD, deployment และเอกสารโครงการ

## Executive summary

SchoolOrbit ไม่จำเป็นต้อง rewrite ใหม่ โครงสร้างพื้นฐานถือว่าดี โดยเฉพาะ service layer, migration discipline, permission model และการเข้ารหัสข้อมูลส่วนบุคคล อย่างไรก็ตาม ก่อนเพิ่มปริมาณผู้ใช้หรือรัน backend หลาย instance ควรพัฒนาตามลำดับดังนี้:

1. ปิดช่องโหว่ WebSocket และ tenant isolation
2. เพิ่ม observability และ CI เพื่อให้วัดประสิทธิภาพได้จริง
3. ลด request fan-out และแก้ resource pooling
4. แยกโมดูลขนาดใหญ่ตาม use case
5. ทำ API contract และ deployment ให้เป็นมาตรฐาน

## จุดแข็งปัจจุบัน

- ใช้ AES-256-GCM สำหรับข้อมูลอ่อนไหว และ HMAC-SHA256 blind index สำหรับค้นหา national ID โดยไม่ใช้ plaintext
- แยก active migrations ออกจาก legacy migrations และมีหลักไม่แก้ migration ที่เคย apply แล้ว
- service layer ของ timetable เป็น reference ที่ดี และ static architecture tests บังคับ thin-handler pattern ได้จริง
- frontend-school มี API client กลางและ response envelope ที่สม่ำเสมอ
- export libraries ขนาดใหญ่หลายส่วนถูกโหลดแบบ dynamic import แล้ว
- Rust compile, Svelte check และ production build ผ่านทั้งระบบ
- มี static tests จำนวนมากสำหรับ permission, route contract และ architecture rules

## ลำดับความสำคัญ

| ระดับ | ประเด็น | ข้อเสนอ |
|---|---|---|
| P0 ✅ เสร็จ 2026-07-21 | WebSocket timetable ไม่ตรวจ JWT และรับ `user_id`, `name`, `school_key` จาก query | ตรวจ JWT ก่อน upgrade, resolve tenant ด้วย resolver กลาง, derive user จาก claims และตรวจ permission/semester |
| P0 ✅ เสร็จ 2026-07-21 | JWT ไม่ผูกกับ tenant | เพิ่ม `tenant_id` หรือ `subdomain`, `iss`, `aud` และ session version ใน claims |
| P0 ✅ เสร็จ 2026-07-21 | Permission cache ใช้เฉพาะ `user_id` เป็น key | เปลี่ยนเป็น `(tenant_id, user_id)` และเพิ่ม cross-tenant negative tests |
| P1 | Realtime, cache และ background jobs เป็น process-local | ก่อนเพิ่ม replica ให้มี distributed backplane และ leader election/distributed lock |
| P1 | ไม่มี PR CI และ deployment ไม่มี readiness/rollback | เพิ่ม CI gates และ deploy immutable artifact/image SHA |
| P1 | ไม่มี request metrics และ tracing | เพิ่ม request ID, latency/error metrics, DB pool และ external API metrics |
| P1 | Timetable frontend มี N+1 requests | เพิ่ม semester-scoped batch endpoint |
| P2 | ไฟล์หลักหลายไฟล์มีขนาด 3,000–5,000 บรรทัด | แยกตาม use case และ resource/controller boundary |
| P2 | Contract ระหว่าง Rust/TypeScript ดูแลด้วยมือ | ใช้ OpenAPI หรือ JSON Schema สร้าง types/client |

## 1. WebSocket security

อัปเดตสถานะ 21 กรกฎาคม 2026: ช่องโหว่ที่บันทึกในหัวข้อนี้แก้แล้ว โดย handshake ตรวจ token และ tenant ก่อน upgrade, identity/room มาจาก server, service ตรวจสิทธิ์อ่าน/จัดการและ semester, server กรอง event ตามสิทธิ์, จำกัด frame 64 KiB และมี heartbeat/idle timeout การถอนสิทธิ์หรือปิดบัญชีจะปิด active socket แบบ fail-closed ก่อนใช้ event ที่ queue อยู่ ส่วน frontend reconnect มี backoff/jitter, หยุดเมื่อ policy close โดยไม่มี intent ใหม่ และยกเลิก timer/listener เมื่อ teardown ข้อความด้านล่างเก็บไว้เป็นหลักฐานของสภาพก่อนแก้ ไม่ใช่พฤติกรรมปัจจุบัน

Route `/ws/timetable` ถูกประกาศโดยไม่มี auth middleware และ handler เชื่อข้อมูลจาก query โดยตรง:

- [`backend-school/src/main.rs`](../backend-school/src/main.rs) ประกาศ route พร้อม comment ว่าไม่ใช้ standard auth
- [`backend-school/src/modules/academic/websockets.rs`](../backend-school/src/modules/academic/websockets.rs) รับ `semester_id`, `user_id`, `name`, `school_key`
- [`frontend-school/src/lib/stores/timetable-socket.ts`](../frontend-school/src/lib/stores/timetable-socket.ts) ส่งข้อมูลดังกล่าวผ่าน query string

ผลกระทบที่เป็นไปได้:

- client ปลอม user ID และชื่อผู้ใช้ได้
- client เลือก tenant และ semester room ได้เอง
- client เข้าร่วม room และรับ broadcast โดยไม่ผ่าน permission check
- สามารถสร้าง presence หรือ event ปลอมได้

แนวทางแก้:

1. ตรวจ JWT/cookie ระหว่าง WebSocket handshake ก่อน `on_upgrade`
2. ใช้ tenant resolver เดียวกับ HTTP แทนการ parse Host/query แยกเอง
3. derive `user_id` และชื่อจาก JWT/ฐานข้อมูล ไม่รับจาก client
4. ตรวจ permission สำหรับการอ่าน timetable และสิทธิ์ที่สูงกว่าเมื่อส่ง mutation-related event
5. ตรวจว่า semester อยู่ใน tenant ที่ resolve ได้
6. เพิ่ม Origin validation, connection limit และ message-size/rate limit
7. เพิ่ม ping/pong heartbeat, idle timeout และ reconnect แบบ exponential backoff + jitter

## 2. Tenant isolation

อัปเดตสถานะ 21 กรกฎาคม 2026: security boundary นี้แก้แล้ว JWT มี tenant, issuer, audience และ token version แบบ strict; request context ต้องตรวจ tenant ใน token ให้ตรงกับ tenant ที่ resolve ก่อนอ่าน tenant data; permission cache ใช้ `(tenant, user_id)` พร้อม revision ต่อ tenant/user เพื่อไม่ให้ in-flight fetch เขียนสิทธิ์เก่ากลับหลัง invalidation และมี negative tests ป้องกันการข้าม tenant ข้อความเดิมด้านล่างเป็นผลวิเคราะห์ก่อนแก้

JWT ปัจจุบันมีเพียง user ID, username, user type และเวลาออก/หมดอายุ:

- [`backend-school/src/modules/auth/models.rs`](../backend-school/src/modules/auth/models.rs)
- [`backend-school/src/utils/jwt.rs`](../backend-school/src/utils/jwt.rs)

ขณะที่ request จะเลือก tenant จาก header/origin แล้วนำ user ID จาก JWT ไปค้นในฐานข้อมูล tenant:

- [`backend-school/src/utils/request_context.rs`](../backend-school/src/utils/request_context.rs)

Permission cache ใช้ `Uuid` อย่างเดียวเป็น key:

- [`backend-school/src/db/permission_cache.rs`](../backend-school/src/db/permission_cache.rs)

ระบบจึงพึ่งสมมติฐานว่า UUID จะไม่ซ้ำข้ามฐานข้อมูล ซึ่งไม่ควรใช้เป็น security boundary โดยเฉพาะกรณี clone, restore, seed หรือ import database

โครงสร้างที่แนะนำ:

```text
Resolved tenant
    -> verify JWT tenant claim
    -> ActorContext(tenant_id, user_id)
    -> PermissionCache[(tenant_id, user_id)]
    -> tenant database service
```

JWT ควรมี tenant identity, `iss`, `aud`, session/token version และตรวจให้ตรงกับ tenant ที่ resolve จาก request ทุกครั้ง

## 3. Horizontal scaling และ realtime

อัปเดตสถานะ 21 กรกฎาคม 2026: process-global notification, permission และ work events ถูกผูก tenant แล้ว จึงไม่ส่ง event ข้าม tenant ภายใน process เดียว อย่างไรก็ตาม backplane ข้าม replica, distributed cache invalidation และ leader election ยังเป็นงาน P1 ตามเดิม และยังไม่ควรอ้างว่ารองรับ horizontal scaling สมบูรณ์

ค่าต่อไปนี้อยู่ใน memory ของแต่ละ process:

- `WebSocketManager`
- notification broadcast channel
- permission-event channel
- work-event channel
- permission cache

ดูได้จาก [`backend-school/src/main.rs`](../backend-school/src/main.rs)

Cron jobs ถูกเริ่มจาก `main()` ทุกครั้งที่เปิด instance ดังนั้นเมื่อมีหลาย replica จะเกิด:

- WebSocket clients ต่าง instance ไม่เห็น event กัน
- permission invalidation ไม่กระจาย
- presence และ sequence reconciliation ไม่สมบูรณ์
- file cleanup และ calendar reminder ทำงานซ้ำ

แนวทางระยะสั้นคือประกาศข้อจำกัดว่า backend-school รองรับหนึ่ง replica เท่านั้น จนกว่าจะมี:

- Redis, NATS หรือ PostgreSQL LISTEN/NOTIFY เป็น realtime/invalidation backplane
- PostgreSQL advisory lock, leader election หรือ external worker สำหรับ scheduled jobs
- idempotency และ bounded concurrency สำหรับงานทุก tenant

## 4. Backend performance และ reliability

### 4.1 Tenant pool manager

[`backend-school/src/db/pool_manager.rs`](../backend-school/src/db/pool_manager.rs) มีจุดที่ควรแก้:

- cache hit ไม่อัปเดต `last_used` ทำให้ pool ที่ยังถูกใช้งานหมดอายุจากเวลา creation
- concurrent cold requests สามารถสร้างหลาย pools ของ tenant เดียวกันก่อนเขียน cache
- log cache hit ทุก request ที่ระดับ `info` ทำให้ log มี noise
- cache key ใช้ database URL แทน stable tenant identity

ข้อเสนอ:

- ใช้ per-tenant single-flight lock
- อัปเดต `last_used` ทุก cache hit
- ใช้ stable tenant key และแยก URL fingerprint/version
- ลด cache-hit log เป็น `debug`
- expose pool count, pool acquire time และ creation failure เป็น metrics

### 4.2 External HTTP clients

HTTP clients ระหว่าง services และ Cloudflare/Neon ส่วนใหญ่ใช้ `Client::new()` โดยไม่มี timeout กลาง:

- [`backend-school/src/db/admin_client.rs`](../backend-school/src/db/admin_client.rs)
- [`backend-admin/src/clients/backend_school_client.rs`](../backend-admin/src/clients/backend_school_client.rs)

ควรมี shared HTTP client configuration ที่กำหนด:

- connect/request timeout
- bounded exponential retry + jitter เฉพาะ idempotent requests
- circuit breaker
- correlation/request ID
- typed external-service errors
- idempotency key สำหรับ provisioning/deployment

### 4.3 Router และ readiness

`backend-school/src/main.rs` รวม route registration, state initialization, background jobs และ server startup ไว้ด้วยกัน ต่างจาก backend-admin ที่มี `build_app(state)`

ควรแยกเป็น:

- `app.rs` สำหรับ router composition
- module-local routers
- `config.rs` สำหรับ typed startup configuration
- `jobs/` สำหรับ scheduled job registration
- route policy แยก public, protected, internal และ upload routes

`/health` ของ backend-school ตอบ healthy เสมอและยังไม่มี `/ready` ควรให้:

- `/health` เป็น liveness ที่เบา
- `/ready` ตรวจ critical configuration และ control-plane/admin connectivity ตามความเหมาะสม
- ไม่จำเป็นต้องปลุกฐานข้อมูล tenant ทุกโรงเรียนเพื่อตรวจ readiness

### 4.4 Observability ก่อน optimize SQL

ยังไม่พบ request tracing, request ID, metrics หรือ distributed tracing ที่เพียงพอ จึงยังไม่ควรเดาว่าต้องเพิ่ม index ใดจากจำนวน query เพียงอย่างเดียว

ควรเก็บ:

- HTTP route p50/p95/p99 และ error rate
- tenant resolution latency
- DB pool acquire time และจำนวน tenant pools
- external API latency/error/retry
- WebSocket connections, reconnects, message lag และ dropped messages
- background-job duration/failure ต่อ tenant
- frontend Web Vitals
- `pg_stat_statements` และ slow-query samples

จากนั้นใช้ `EXPLAIN (ANALYZE, BUFFERS)` กับ query ที่ช้าจริง และใช้ cursor/keyset pagination สำหรับตารางที่เติบโตมากเมื่อ UX รองรับ

### 4.5 File cleanup

[`backend-school/src/services/cleaner.rs`](../backend-school/src/services/cleaner.rs) ตรวจ orphan profile image ด้วย `LIKE '%' || storage_path || '%'` ซึ่งไม่สามารถใช้ index ได้ดีและผูกความสัมพันธ์ผ่าน URL string

ควรเปลี่ยนเป็น foreign key เช่น `users.profile_image_file_id` หรือ relation table เพื่อให้ตรวจ orphan ด้วย indexed join ได้โดยตรง

## 5. Security และ PDPA

ส่วนที่ทำได้ดีคือ app-side AES-256-GCM และ keyed HMAC blind index ถูกใช้งานจริง:

- [`backend-school/src/utils/field_encryption.rs`](../backend-school/src/utils/field_encryption.rs)

จุดที่ควรปรับ:

1. [`backend-school/src/modules/menu/services/public_menu_service.rs`](../backend-school/src/modules/menu/services/public_menu_service.rs) โหลด User เกือบทั้งแถว รวม password hash/national ID แล้ว decrypt ทั้งที่ใช้เพียง `user_type`
2. [`backend-school/src/error.rs`](../backend-school/src/error.rs) ส่ง foreign-key database message บางส่วนกลับ client
3. มี audit schema และ utility แต่ [`backend-school/src/utils/audit.rs`](../backend-school/src/utils/audit.rs) ระบุว่ายังไม่ได้ integrate
4. ไม่พบ rate limiting ที่บังคับใช้กับ login, public admission และ internal endpoints
5. ควรมี explicit CSRF/Origin policy สำหรับ cookie-based mutation
6. [`podman-compose.yml`](../podman-compose.yml) มี fallback `DEPLOY_KEY` สำหรับ production stack

Audit log ควรสร้างใน service/transaction boundary และห้ามบันทึก national ID, password, token, encryption key หรือ request body แบบดิบ หาก audit event ต้องเชื่อถือได้มากควรใช้ transactional outbox

## 6. Backend modularity

ไฟล์ที่ใหญ่ที่สุดบางส่วน:

- `exam_schedule_service.rs` ประมาณ 4,874 บรรทัด
- `supervision/services.rs` ประมาณ 4,172 บรรทัด
- `timetable_service.rs` ประมาณ 2,620 บรรทัด
- `calendar/services.rs` ประมาณ 2,263 บรรทัด

ควรแยกตาม cohesive use case ไม่ใช่แบ่งตามจำนวนบรรทัด เช่น:

```text
exam_schedule/
  rounds_and_days.rs
  placement_and_conflicts.rs
  room_assignment.rs
  invigilation.rs
  publish.rs
  export.rs

supervision/
  cycles.rs
  templates.rs
  requests.rs
  evaluations.rs
  review_and_approval.rs
```

Handler ควรคงรูปแบบ permission check -> service call -> HTTP/WS response และให้ service รับ `&PgPool`/domain inputs โดยไม่ผูกกับ Axum

## 7. Frontend-school

### 7.1 Data loading และ auth bootstrap

Authenticated app ตั้ง `ssr = false` ใน [`frontend-school/src/routes/(app)/+layout.ts`](<../frontend-school/src/routes/(app)/+layout.ts>) ซึ่งเป็นการตัดสินใจที่สมเหตุผลกับ sibling API และ cross-origin cookie architecture จึงไม่ควรย้ายทุกอย่างไป `+page.server.ts` โดยอัตโนมัติ

อย่างไรก็ตาม layout ตรวจ `/auth/me` ใน `onMount` แล้วแสดง full-screen spinner ก่อน render children:

- [`frontend-school/src/routes/(app)/+layout.svelte`](<../frontend-school/src/routes/(app)/+layout.svelte>)

หน้าแต่ละหน้าจึงเริ่มโหลดข้อมูลภายหลังและเกิด waterfall ข้อเสนอคือ:

- ใช้ universal client `load` หรือ route-scoped resource controller
- cache/dedupe auth bootstrap
- ใช้ load fetch/invalidation semantics เมื่อเหมาะสม
- รองรับ cancellation เมื่อออกจาก route หรือเปลี่ยน filter

### 7.2 API client

[`frontend-school/src/lib/api/client.ts`](../frontend-school/src/lib/api/client.ts) มี response normalization ที่ดี แต่ยังขาด:

- `AbortSignal`
- request timeout
- typed API error ที่มี status/code
- safe GET retry
- request deduplication
- injectable fetch สำหรับ testing

ยังไม่จำเป็นต้องเพิ่ม state/query library ขนาดใหญ่ทันที ควรเริ่มด้วย native resource modules ก่อน แล้วใช้ TanStack Query เมื่อพบ duplication/caching complexity จริง

### 7.3 Timetable request fan-out

[`frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`](<../frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte>) โหลด instructor/classroom assignments แยกทีละ activity slot บางส่วนเป็น sequential loop

ควรเพิ่ม endpoint ลักษณะ:

```text
GET /api/academic/activity-slots/context?semester_id=...
```

ให้คืน slots, instructors และ classroom assignments ในครั้งเดียว หรือตาม filter ของ classroom/instructor เพื่อลดจำนวน request จาก O(slots) เป็น O(1)

### 7.4 Component size และ Svelte reactivity

ไฟล์ขนาดใหญ่:

- timetable page ประมาณ 5,630 บรรทัด
- `SupervisionWorkspace.svelte` ประมาณ 3,605 บรรทัด
- subjects page ประมาณ 2,935 บรรทัด

ควรแยก timetable เป็น:

- route container
- timetable resource/controller
- grid/sidebar components
- drag-and-drop adapter
- conflict/bulk dialogs
- export adapter
- realtime adapter

Svelte 5 analysis พบ effect หลายจุดที่ mutate state และมี lint แนะนำ writable derived/SvelteMap/SvelteSet ควรให้ `$derived` ใช้สำหรับ computed state และใช้ event handlers/resource controller สำหรับ async orchestration แทน effect chains

ใช้ `$state.raw` เฉพาะ immutable API snapshots ขนาดใหญ่หลังวัดผล ไม่ควรเปลี่ยนแบบ blanket rewrite

### 7.5 Bundle และ build performance

Production build ผ่าน แต่ emitted client assets รวมประมาณ 15 MB ซึ่งไม่เท่ากับ initial download เนื่องจากมี code splitting อย่างไรก็ตามมี asset ใหญ่:

- รายชื่อโรงเรียนไทยประมาณ 2.35 MB raw / 236 KB gzip
- question-bank route ประมาณ 1.49 MB raw / 425 KB gzip
- PDF/Excel/Word chunks หลายก้อนประมาณ 0.5–1.3 MB raw

แนวทางปรับปรุง:

- เพิ่ม bundle visualizer และ route-level bundle budgets
- ลด `chunkSizeWarningLimit` จาก 1,500 KB ใน [`frontend-school/vite.config.ts`](../frontend-school/vite.config.ts)
- ย้ายรายชื่อโรงเรียนไป search API หรือ compact indexed file
- แยก rich editor/viewer จาก initial question-bank route
- pin `xlsx` เป็นเวอร์ชันแทน `xlsx-latest`
- ตรวจ `client-only-word-exporter` plugin เพราะ build timing ระบุว่าใช้เวลาส่วนใหญ่

### 7.6 Service worker

[`frontend-school/src/service-worker.ts`](../frontend-school/src/service-worker.ts) intercept same-origin requests ด้วย network-only strategy แต่พยายาม fetch `/offline.html` หลัง network failure ทั้งที่ไม่ได้ precache

ควรเลือกหนึ่งแนวทาง:

- push-only service worker โดยไม่ intercept fetch หรือ
- precache app shell/offline page/static assets และมี offline strategy ที่ใช้งานได้จริง

## 8. Frontend-admin และ backend-admin

frontend-admin ยังมี maturity ต่ำกว่า frontend-school:

- ไม่มี automated tests
- dependency versions ตามหลัง frontend-school
- [`frontend-admin/src/lib/api/client.ts`](../frontend-admin/src/lib/api/client.ts) cast response โดยไม่มี runtime validation
- มี `Record<string, any>` และ untyped SSE data
- lint พบ navigation ที่ไม่ผ่าน `resolve()` และ `{#each}` ที่ไม่มี key

Backend-admin ตรวจว่า role เป็น Admin/SuperAdmin แล้ว แต่ทั้งสอง role ทำ protected actions ได้เหมือนกัน:

- [`backend-admin/src/auth/types.rs`](../backend-admin/src/auth/types.rs)
- [`backend-admin/src/app.rs`](../backend-admin/src/app.rs)

ควรเปลี่ยนเป็น capability-based authorization เช่น:

- `school.read`
- `school.provision`
- `school.update`
- `school.delete`
- `deployment.trigger`
- `migration.manage`

## 9. Testing strategy

ปัจจุบัน static/unit tests มีจำนวนมาก แต่ยังขาด integration tests ตาม risk boundary:

- backend-school มี integration test file หลักเพียง static architecture
- auth DB tests สองตัวเป็น setup scaffold และยังไม่ได้เรียก handler/ตรวจ response จริง
- frontend-school มี static tests 24 ไฟล์ แต่ Playwright มีเพียง login flow
- frontend-admin ไม่มี automated tests

ชุดทดสอบที่ควรเพิ่มตามลำดับ:

1. cross-tenant JWT/cache/WebSocket negative tests
2. Axum router tests สำหรับ cookie, permission และ response contract
3. PostgreSQL integration tests ด้วย ephemeral database/schema
4. component interaction tests สำหรับ timetable, question bank และ admin provisioning
5. Playwright critical flows: login, staff/student, timetable mutation, admission, admin provisioning

Source-text static tests ควรคงไว้เป็น architecture guard แต่ไม่ควรใช้แทน behavior tests

## 10. CI/CD และ deployment

ปัจจุบันไม่มี PR CI; smoke/E2E เป็น manual และ deployment ทำงานจาก push โดยตรง

จุดเสี่ยง:

- [`deploy-all-schools.yml`](../.github/workflows/deploy-all-schools.yml) คืน `exit 0` เมื่อ API รายชื่อโรงเรียนล้มเหลว ทำให้ workflow ดูสำเร็จโดยไม่ได้ deploy
- frontend ถูก build ใหม่ทุก tenant ทั้งที่ source artifact ส่วนใหญ่เหมือนกัน
- menu registration เป็น side effect ของ build และ failure ไม่ทำให้ build fail
- backend deploy ใช้ `latest`, หยุด container เก่าก่อน และไม่มี readiness/rollback
- backend-school ไม่มี `/ready`
- Dockerfiles ใช้ unpinned Rust base image และติดตั้ง `cargo-chef` แบบไม่ระบุเวอร์ชัน
- production compose ใช้ `latest` และมี secret fallback

CI ที่แนะนำ:

```text
PR
 |- Rust fmt + clippy + unit/static tests
 |- Svelte check + ESLint + Prettier + static tests
 |- Production builds + bundle budgets
 |- PostgreSQL integration tests
 `- Playwright critical flows

Deploy
 |- use artifact/image SHA produced by CI
 |- start new instance
 |- readiness + smoke
 |- switch traffic
 `- automatic rollback on failure
```

ควร build frontend artifact ครั้งเดียว แยก menu synchronization ออกจาก Vite build และ deploy immutable artifact ต่อ tenant หรือใช้ shared wildcard deployment เมื่อ architecture รองรับ

## 11. Shared contracts และ developer flexibility

อัปเดตสถานะ 21 กรกฎาคม 2026: permission registry มี
`contracts/permissions.json` เป็น source of truth แล้ว และ generate Rust/TypeScript
registry พร้อม lock, parity tests และ focused CI โดยอัตโนมัติ งานนี้ลด drift ของ permission
ระหว่าง backend/frontend เท่านั้น ส่วน API request/response contracts และ client generation
ทั่วไปยังเป็นงานอนาคตตามข้อเสนอด้านล่าง

ควรมี canonical API contract เพื่อลด Rust/TypeScript drift:

- OpenAPI หรือ JSON Schema เป็น source of truth
- generate frontend types/client
- generate/request contract tests ใน CI
- shared Rust crate เฉพาะ contract/config/HTTP client ที่เสถียร

ไม่ควรสร้าง shared crate ที่รวม business logic ของ backend-admin และ backend-school จนสอง services ผูกกันแน่นเกินไป

พิจารณา root workspace สำหรับ tooling ที่ใช้ร่วมกัน:

- Cargo workspace สำหรับ common contracts/config/testing utilities
- npm workspace สำหรับ lint/format/test configuration
- version alignment ระหว่าง frontend-school และ frontend-admin

## 12. Documentation drift

เอกสารหลายส่วนไม่ตรงกับโค้ดปัจจุบัน:

- [`docs/PROJECT_PLAN.md`](PROJECT_PLAN.md) ยังระบุ student/timetable เป็น 0%
- [`IMPROVEMENT_PLAN.md`](../IMPROVEMENT_PLAN.md) ระบุ backend-school readiness เสร็จแล้ว แต่ไม่มี `/ready`
- [`TODO_ENCRYPTION.md`](../TODO_ENCRYPTION.md) อ้าง migration 117 ขณะที่ active migrations เป็น baseline + 002–026
- [`README.md`](../README.md) ระบุ rate limiting/virus scan ready แต่ไม่พบ implementation ที่บังคับใช้ชัดเจน

ควรมี living architecture document และ ADR สำหรับ:

- database-per-tenant
- intentional CSR authenticated app
- tenant-bound authentication
- single-replica vs multi-replica support
- realtime backplane
- API contract generation
- deployment และ menu-registration lifecycle

เอกสารสถานะควรมี `last verified date` และ commit SHA และย้ายรายงานเก่าไปหมวด archive แทนการปล่อยให้ดูเหมือน current state

## Verification results — original analysis baseline

ตารางนี้เป็นผลตรวจ ณ เวลาวิเคราะห์ครั้งแรกก่อนทำ P0 และเก็บไว้เป็นประวัติ ผลตรวจหลังดำเนินการอยู่ในหัวข้อถัดไป

| คำสั่ง/ชุดตรวจ | ผล |
|---|---|
| backend-school `cargo check --bin backend-school` | ผ่าน |
| backend-school unit tests ที่ไม่พึ่ง DB | 387 ผ่าน, 2 DB tests ถูกกรองออก |
| backend-school static architecture tests | 76 ผ่าน |
| backend-admin tests | 18 ผ่าน |
| frontend-school `npm run check` | 0 errors, 0 warnings |
| frontend-admin `npm run check` | 0 errors, 0 warnings |
| frontend-school static tests | 242 ผ่าน |
| production build ทั้งสอง frontend | ผ่าน |
| Rust format ทั้งสอง backend | ผ่าน |
| strict Clippy | backend-school 4 errors, backend-admin 2 errors |
| ESLint | frontend-school 11 errors, frontend-admin 31 errors |
| Prettier | frontend-school 16 ไฟล์, frontend-admin 15 ไฟล์ไม่ผ่าน |

DB-backed tests, sandbox smoke และ Playwright E2E ไม่ได้รันเพราะต้องใช้ credentials/environment ภายนอก โดย auth DB tests ปัจจุบันยังเป็น setup scaffold และไม่ได้เรียก handler จริง

### P0 tenant authentication and realtime boundary — completed 2026-07-21

- JWTs are tenant-, issuer-, audience-, and version-bound; the request tenant must match before tenant data is read.
- Permission cache entries and process-global notification/permission/work events are tenant-scoped; revision-tagged fills prevent stale permissions from being restored after invalidation.
- Timetable WebSocket identity and rooms are server-derived; connection/edit permissions, active-session revocation, frame limits, heartbeat, and bounded reconnect are enforced.
- No database migration was added or modified.
- Final post-review verification below targets commit `10950783`.
- Verification commands run on 2026-07-21 are listed here with one of two exact prefixes: `PASS —` followed by the successful command, or `NOT RUN —` followed by the command and the concrete environment limitation.

- PASS — `cd backend-school && cargo fmt --all -- --check` (exit 0).
- PASS — `cd backend-school && cargo check --bin backend-school` (exit 0).
- PASS — `cd backend-school && cargo test --bin backend-school -- --skip modules::auth::tests::auth_tests::test_login_success --skip modules::auth::tests::auth_tests::test_login_invalid_credentials` (exit 0; 431 passed, 0 failed, 2 filtered out).
- PASS — `cd backend-school && cargo test --test static_architecture` (exit 0; 88 current static guards passed).
- PASS — `cd backend-school && cargo clippy --all-targets --all-features -- -D warnings` (exit 0; strict Clippy passed).
- PASS — `cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check` (exit 0; 0 errors, 0 warnings).
- PASS — `cd frontend-school && npm run test:static` (exit 0; 254 passed, 0 failed).
- PASS — `cd frontend-school && npx eslint src/lib/stores/timetable-socket.ts src/lib/utils/timetable-reconnect.ts src/lib/utils/timetable-socket-runtime.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte'` (exit 0).
- PASS — `cd frontend-school && npx prettier --check src/lib/stores/timetable-socket.ts src/lib/utils/timetable-reconnect.ts src/lib/utils/timetable-socket-runtime.ts 'src/routes/(app)/staff/academic/timetable/+page.svelte' tests/static/api-global-contract.test.mjs tests/static/timetable-realtime-security.test.mjs tests/static/timetable-socket-runtime.test.mjs` (exit 0).
- PASS — `cd frontend-school && PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run build` (exit 0).
- PASS — `sed -n '/pub struct WsParams/,/State Manager/p' backend-school/src/modules/academic/websockets.rs | rg -n 'user_id|name|school_key'` found no matches (`rg` exit 1 as expected).
- PASS — `sed -n '/export type TimetableSocketParams = {/,/^};/p' frontend-school/src/lib/utils/timetable-socket-runtime.ts | rg -n 'school_key|name:\s*string'` produced no output (`rg` exit 1 as expected); local-only `current_user_id` is intentionally allowed because it is not query identity.
- PASS — `sed -n '/const qs = new URLSearchParams({/,/}).toString();/p' frontend-school/src/lib/stores/timetable-socket.ts | rg -n 'school_key|name|user_id'` produced no output (`rg` exit 1 as expected).
- PASS — `sed -n '/const qs = new URLSearchParams({/,/}).toString();/p' frontend-school/src/lib/stores/timetable-socket.ts | rg -n 'semester_id:\s*String\(params\.semester_id\)'` returned `2:\t\t\tsemester_id: String(params.semester_id)` (`rg` exit 0), confirming the scoped URL-construction range was non-empty.
- PASS — `rg -n 'Sender<\(Uuid, Notification\)>|permission_cache\.(invalidate\(|clear_all\()' backend-school/src` found no legacy API (`rg` exit 1 as expected).
- PASS — `rg -n 'JwtService::verify_token' backend-school/src/modules` found no feature-module JWT parser (`rg` exit 1 as expected).
- PASS — `rg -n 'auth_token|JWT_SECRET|ENCRYPTION_KEY|BLIND_INDEX_KEY|national_id' backend-school/src/modules/academic/websockets.rs frontend-school/src/lib/stores/timetable-socket.ts` found no token secret or PII handling (`rg` exit 1 as expected).
- PASS — `git diff --check 9095cc1e..10950783` (exit 0 before recording this final result).
- PASS — `git diff --name-only -- backend-school/migrations` returned no migration paths.
- PASS — `git diff --name-only 9095cc1e..10950783 -- backend-school/migrations` returned no migration paths across the final implementation.
- PASS — `nginx:alpine nginx -t` with the deployment upstream host mapping and a temporary self-signed certificate reported `syntax is ok` and `test is successful`.
- PASS — `git status --short` and `git diff --stat` confirmed a clean worktree before this report update.
- NOT RUN — `TEST_DATABASE_URL=... cargo test modules::auth::tests::auth_tests::test_login_success --bin backend-school`; `TEST_DATABASE_URL` was absent, so the DB-backed auth test remained one of the two intentionally filtered tests.
- NOT RUN — `TEST_DATABASE_URL=... cargo test modules::auth::tests::auth_tests::test_login_invalid_credentials --bin backend-school`; `TEST_DATABASE_URL` was absent, so the DB-backed auth test remained one of the two intentionally filtered tests.
- NOT RUN — `MIGRATION_AUDIT_DATABASE_URL=... ./scripts/check_migration_rebaseline_ready.sh`; no approved audit database URL was present.
- NOT RUN — `./scripts/smoke_test.sh`; `SMOKE_USERNAME` and `SMOKE_PASSWORD` were absent and no external sandbox scope was supplied.
- NOT RUN — `cd frontend-school && npm run test:e2e`; neither `E2E_BASE_URL`/`E2E_USERNAME`/`E2E_PASSWORD` nor equivalent `SMOKE_*` credentials were present.
- NOT RUN — live timetable WebSocket 401/403/404, two-tab presence, reader/manager, and offline reconnect checklist in `docs/TESTING.md`; `E2E_SEMESTER_ID` and sandbox credentials were absent.

หมายเหตุ: frontend identity search เดิมในแผนใช้ช่วง `sed` ที่เริ่มจากข้อความ `type TimetableSocketParams` ใน import หลังแยก runtime จึงครอบ server-event DTO ทั้งไฟล์และพบ `name` ของ presence/instructor ซึ่งไม่ใช่ query identity ผลนั้นไม่ได้ถูกนับเป็น PASS; ใช้ scoped query-contract search ข้างต้นร่วมกับ static behavior tests ที่ผ่านแทน โดยไม่แก้ production code เพื่อหลบการตรวจ

## Recommended roadmap

### Sprint 1 — Security boundary

- [x] WebSocket authentication และ authorization
- [x] tenant-bound JWT และ permission cache key
- [x] cross-tenant negative tests
- ตัด PII over-fetch และ safe database errors
- บังคับ production secrets

### Sprint 2 — Reliability และ measurable performance

- request ID, tracing, metrics และ `/ready`
- pool single-flight + sliding TTL
- HTTP timeout/retry/circuit breaker
- timetable batch endpoints
- scheduler locking และ realtime backplane

### Sprint 3 — Developer flexibility

- แยก timetable, supervision และ exam-schedule modules
- [x] generate permission registry ร่วมกันระหว่าง Rust/TypeScript พร้อม drift CI
- OpenAPI-generated contracts
- frontend resource controllers และ request cancellation
- bundle budgets
- PR CI และ immutable deployment
- ปรับเอกสารและ ADR ให้ตรงกับระบบจริง

## ข้อจำกัดของการวิเคราะห์

รายงานนี้มาจาก static inspection และ local verification ของ repository ยังไม่มี production traffic metrics, database execution plans หรือ production traces จึงควรเก็บ observability data ก่อนตัดสินใจเพิ่ม index, เปลี่ยน cache policy หรือทำ performance rewrite ขนาดใหญ่
