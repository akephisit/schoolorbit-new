# SchoolOrbit — แผนการปรับปรุงระบบ (backend-admin & backend-school)

> วันที่วิเคราะห์: 2026-03-23
> เป้าหมาย: ให้ระบบใช้งานได้ยาวๆ และพัฒนาเพิ่มฟีเจอร์ได้ง่ายในอนาคต

## Overview ระบบ

- **backend-admin** (Rust/Axum, port 8080) — จัดการ school provisioning ผ่าน Neon, Cloudflare, GitHub Actions
- **backend-school** (Rust/Axum, port 8081) — backend หลักสำหรับแต่ละโรงเรียน (multi-tenant, 12 modules, 51 migrations)
- **frontend-admin** (SvelteKit) — admin dashboard, ใช้ `/api/v1/` ✓
- **frontend-school** (SvelteKit) — school app, ใช้ `/api/` (ไม่มี versioning, ตั้งใจ)

ทั้งสองใช้ `INTERNAL_API_SECRET` สื่อสารกัน backend-school เรียก `GET /internal/schools/{subdomain}` ของ backend-admin เพื่อ route request ไปยัง tenant database (ไม่ต่อ admin DB ตรงๆ อีกต่อไป — ✅ M-3 done)

---

## 🔴 Priority 1: Critical — แก้ไขทันที

### ✅ C-1. Race condition ใน lazy migration/permission sync — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/db/migration.rs` |
| **ที่ทำ** | เพิ่ม per-subdomain lock ด้วย `DashMap<String, Arc<tokio::sync::Mutex<()>>>` และ helper `run_once()` ที่ re-check สถานะหลังได้ lock ก่อน execute |
| **ผลลัพธ์** | request tenant เดียวกันตอน cold start จะรัน migration/permission sync ได้แค่ครั้งเดียว ส่วน tenant คนละ subdomain ยังทำงานขนานกันได้ |
| **ความยาก** | Small |

### ✅ C-2. Broken rollback เมื่อ provisioning ล้มเหลว — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs` (บรรทัด 60-66) |
| **ที่ทำ** | รวม provisioning rollback path ให้ `await` การลบ Neon DB จริง, report cleanup failure กลับไปกับ primary error, mark school เป็น `provision_failed` ก่อน cleanup, และลบ school record ออกเมื่อ Neon DB rollback สำเร็จเพื่อให้ subdomain ใช้ซ้ำได้ |
| **ผลลัพธ์** | SSE และ non-SSE create flow ไม่กลบ rollback error ด้วย `let _ =` อีกต่อไป และ deployment failure ถูก mark เป็น `deployment_failed` แบบตรวจ error ได้ |
| **ความยาก** | Small (fix bug) / Medium (rollback สมบูรณ์) |

### ✅ C-3. PII (national_id) ยังไม่ได้เข้ารหัสครบ — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/modules/admission/`, `backend-school/src/modules/staff/`, `backend-school/src/modules/students/` |
| **ที่ทำ** | ใช้ app-side `field_encryption.rs` สำหรับ `national_id`, เพิ่ม keyed HMAC blind hash สำหรับ lookup/unique, และอัปเดต `TODO_ENCRYPTION.md` ให้เลิกอ้าง pgcrypto legacy |
| **ความยาก** | Medium |

### ✅ C-4. INTERNAL_API_SECRET ไม่ timing-safe — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/middleware/internal_auth.rs`, `backend-admin/src/handlers/internal.rs` |
| **ปัญหา** | `auth_header != internal_secret` เปรียบเทียบแบบ naive — เสี่ยง timing attack และ rotate secret ได้ยากเพราะใช้ secret เดียวกับทุก caller |
| **ที่ทำ** | ใช้ `subtle::ConstantTimeEq`, เพิ่ม `X-Internal-Caller`, รองรับ `INTERNAL_API_SECRET_<CALLER>` พร้อม fallback ไป `INTERNAL_API_SECRET`, และให้ backend-admin/backend-school internal clients ส่ง caller header |
| **ความยาก** | Small |

### ✅ C-5. Admin JWT claims มี school-scoped fields ที่ไม่จำเป็น — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/auth/types.rs`, `backend-admin/src/middleware/auth.rs` |
| **ที่ทำ** | แยกเป็น `AdminClaims` ไม่มี `school_id/subdomain`, เพิ่ม `AdminRole::can_access_admin_backend()` และ enforce role ใน middleware + `/api/v1/auth/me` |
| **ความยาก** | Small |

---

## 🟠 Priority 2: High — แก้ในระยะสั้น

### ✅ H-1. OnceLock DB pool ป้องกัน testing — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/app.rs`, `backend-admin/src/state.rs`, `backend-admin/src/main.rs`, `backend-admin/src/handlers/*.rs` |
| **ปัญหา** | handlers แต่ละไฟล์มี `static DB_POOL: OnceLock<PgPool>` ของตัวเอง ทดสอบไม่ได้ (`OnceLock::set` ครั้งที่ 2 silent fail) ต่างจาก backend-school ที่ใช้ `State<AppState>` ถูกต้อง |
| **ที่ทำ** | เพิ่ม `AppState { pool: PgPool }`, ย้าย router ไป `build_app(state)`, ให้ handlers รับ `State<AppState>` และเพิ่ม integration test ว่า app สร้างจาก state ที่ส่งเข้าไปได้ |
| **ตรวจสอบ** | `cargo test`, `cargo check`, grep ไม่พบ `OnceLock`/`DB_POOL` ใน backend-admin |

### ✅ H-2. `school_service.rs` 783 บรรทัด มี logic ซ้ำ — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs` |
| **ปัญหา** | provisioning logic ซ้ำกัน 2 ชุด (SSE/non-SSE) — bug C-2 เกิดเพราะแก้ใน SSE version แต่ลืม non-SSE |
| **ที่ทำ** | รวม provisioning เป็น `provision_school()` flow เดียว แล้วให้ `create_school()`/`create_school_stream()` เป็น wrapper ผ่าน `ProvisioningRunOptions` |
| **รายละเอียด** | API ปกติใช้ console logging และไม่รอ GitHub Actions; SSE ใช้ `SseLogger`, ส่ง complete event และยังรอ workflow completion เหมือนเดิม |
| **ตรวจสอบ** | `cargo test`, `cargo check`, `git diff --check` |

### ✅ H-3. `schools.config` เป็น untyped `serde_json::Value` — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/models/school.rs`, `backend-admin/src/services/school_service.rs` |
| **ปัญหา** | typo เช่น `"db_id"` vs `"db-id"` ไม่ถูก catch ที่ compile time — return `None` เงียบๆ |
| **ที่ทำ** | สร้าง `SchoolConfig` struct และใช้ `sqlx::types::Json<SchoolConfig>` ใน `School.config` พร้อม bind ผ่าน `Json(config)` ตอนเขียน DB |
| **ผลลัพธ์** | code ที่อ่าน `db_id`/`dns_record_id` เป็น typed field แล้ว ลด typo จาก string key และทำให้ compiler ช่วยจับ schema drift |
| **ตรวจสอบ** | `cargo test`, `cargo check`, `git diff --check` |

### ✅ H-4. Migration gap — ไม่มี migration 003 ใน backend-admin — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/migrations/` |
| **ที่ทำ** | เพิ่ม `003_placeholder.sql` ด้วย `SELECT 1;` พร้อม comment เพื่อคงลำดับ migration |
| **ความยาก** | Small |

### ✅ H-5. Permission check ทำ 2 DB round trips ต่อ request — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/middleware/permission.rs`, `backend-school/src/db/permission_cache.rs` |
| **ที่ทำ** | เพิ่ม in-memory permission cache (`DashMap<Uuid, Vec<String>>`) ใน `AppState` พร้อม TTL 30 นาที และ explicit invalidation จาก mutation handlers |
| **หลักการ** | cache hit: JWT verify + cache lookup = **0 DB trips** / cache miss: permissions-only query (ไม่มี JOIN user) = **1 DB trip** / `check_permission` เปลี่ยน return type เป็น `Uuid` แทน `User` เพื่อไม่ต้อง fetch user เลย |
| **Invalidation** | `assign_user_role`, `remove_user_role`, `update_staff` → `invalidate(user_id)` / `update_role`, `update_department_permissions` → `clear_all()` / TTL 30 นาทีเป็น safety net |
| **ผลลัพธ์** | จาก 2 DB trips → 0 trips (cache hit) หรือ 1 trip (miss) ต่อทุก request ที่ตรวจสิทธิ์ |

---

## 🟡 Priority 3: Medium — sprint ถัดไป

### ✅ M-1. Subdomain extraction จาก `Origin` header ขาดความยืดหยุ่น — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/utils/subdomain.rs` |
| **ปัญหา** | ไม่รองรับ non-browser clients (Postman, mobile, scripts), local dev และ custom domains ในอนาคต |
| **ที่ทำ** | ฝั่ง backend ยังรับ `X-School-Subdomain` เป็น first-class input: ถ้ามี header จะ validate และใช้ก่อน fallback ไป Origin/Referer; ฝั่ง frontend ส่ง header เฉพาะเมื่อกำหนด `PUBLIC_SCHOOL_SUBDOMAIN` ชัดเจน ส่วน production browser tenant ใช้ Origin/Referer เป็นหลัก, เพิ่ม smoke/preflight coverage และอัปเดต CORS docs ให้ allow header นี้ |
| **ความยาก** | Small |

### ✅ M-2. 123 migrations ไม่มี rebaseline strategy — clean cutover แล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/migrations/`, `backend-school/migrations_legacy/`, `backend-school/src/db/migration.rs` |
| **ปัญหา** | provisioning ใหม่รัน migration 123 ไฟล์ถึง version 127 ยิ่งนาน migration ยิ่งช้า และมี migration checkpoint เฉพาะทางที่ต้องคุมให้ถูกก่อน rebaseline |
| **ที่ทำ** | ย้าย migration เก่าไป `backend-school/migrations_legacy/`, ให้ active runtime migration เหลือ `backend-school/migrations/001_baseline.sql` ไฟล์เดียว, ลบ fast path/checkpoint เฉพาะทางใน `run_tenant_migrations()`, ให้ `seed_sandbox` ใช้ runner กลาง และเพิ่ม guard ว่า active migration ต้องเป็น clean baseline เท่านั้น |
| **ข้อจำกัด** | ห้าม deploy ชุด clean นี้ทับ tenant DB เดิมที่มี `_sqlx_migrations` 1-127 อยู่ ต้อง provision ฐานใหม่, apply `001_baseline.sql`, copy ข้อมูลที่ต้องใช้, validate แล้วค่อยสลับ database URL |
| **ความยาก** | Medium |

### ✅ M-3. แยก backend-school ออกจาก admin DB (HTTP separation) — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/db/admin_client.rs` (ใหม่), `backend-school/src/main.rs`, `backend-school/src/db/school_mapping.rs` |
| **ที่ทำ** | ลบ `admin_pool: PgPool` ออกจาก `AppState` และแทนด้วย `admin_client: Arc<AdminClient>` — HTTP client ที่เรียก `GET /internal/schools/{subdomain}` บน backend-admin แทนการ query DB ตรง เพิ่ม endpoint `PUT /internal/schools/{subdomain}/migration-status` สำหรับ write-back |
| **ผลลัพธ์** | backend-school ไม่มี admin DB credentials อีกต่อไป — isolation สมบูรณ์ |

### ✅ M-4. User role assignment API contract ยังไม่ชัด — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/modules/staff/services/user_role_service.rs`, `backend-school/src/modules/staff/handlers/user_roles.rs`, `frontend-school/src/lib/api/roles.ts`, `frontend-school/src/lib/components/UserRoleManager.svelte` |
| **ปัญหา** | `GET /api/users/{id}/roles` ฝั่ง backend ส่ง `Role[]` จาก `roles JOIN user_roles` แต่ frontend type ชื่อ `UserRole[]` และ component ใช้ field แบบ assignment (`role_id`, `is_primary`) ทำให้ contract ไม่ตรงกับ payload จริง |
| **ที่ทำ** | เพิ่ม `UserRoleAssignmentResponse` ให้ backend ส่ง assignment fields (`role_id`, `is_primary`, period, notes) พร้อม nested `role` object และปรับ frontend เป็น `UserRoleAssignment[]` ให้ component ใช้ `userRole.role` โดยตรง |
| **ตรวจสอบ** | `cargo check`, `cargo test --bin backend-school`, `npm run check`, `npm run test:static` |

### ✅ M-5. `println!` แทน structured logging ใน backend-admin — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs`, `backend-admin/src/clients/*.rs` |
| **ปัญหา** | 111+ `println!` calls — ไม่สามารถ filter level หรือ aggregate ใน Grafana/Datadog ได้ และอาจ log sensitive data |
| **ที่ทำ** | เพิ่ม `tracing`/`tracing-subscriber`, init logger จาก `RUST_LOG`, และแทนที่ `println!`/`eprintln!` ใน backend-admin main/service/clients ด้วย `info!`/`warn!`/`error!`/`debug!` |
| **ผลลัพธ์** | log filter ได้ตาม level/module และลดการพิมพ์ response ดิบจาก external APIs |
| **ตรวจสอบ** | `cargo test`, `cargo check`, `git diff --check`, `rg "println!\|eprintln!" backend-admin/src/main.rs backend-admin/src/services/school_service.rs backend-admin/src/clients/*.rs` |

### ✅ M-6. กำหนด behavior สำหรับ route ลบ role/organization unit ที่ frontend อ้างถึง — เสร็จแล้ว

| | |
|---|---|
| **ไฟล์** | migration `027_role_organization_system_flags.sql`, staff handlers/services, permission resolver/policies, generated OpenAPI/TypeScript, role/organization management UI |
| **ที่ทำ** | เพิ่ม `DELETE /api/roles/{id}` และ `DELETE /api/organization/units/{id}` เป็น reversible deactivation (`is_active = false`) โดยเก็บ assignments, memberships, grants, delegations และ history ไว้ทั้งหมด; เพิ่ม `include_inactive=true` สำหรับหน้าจัดการและเปิดกลับผ่าน PUT |
| **ความปลอดภัย** | DELETE ต้องมี `roles.delete.all`; PUT ที่ส่ง `is_active: false` ต้องมีทั้ง update+delete; `is_system` เป็น read-only migration/provisioning flag ป้องกัน `ADMIN` และ `SCHOOL`; inactive role/unit/scoped delegation ไม่ให้ effective permission และห้ามสร้าง assignment ใหม่ไปยัง record ที่ inactive |
| **Hierarchy / side effects** | ห้ามปิด parent ที่ยังมี active child และห้ามเปิด/สร้าง/ย้าย active child ใต้ inactive parent; status transition จริงเขียน audit ใน transaction เดียวกันและจึง invalidate tenant permission cache + แจ้ง realtime ส่วน idempotent no-op ไม่สร้าง side effect |
| **Frontend** | หน้าจัดการแสดง inactive/system, ใช้คำว่า “ปิดใช้งาน/เปิดใช้งาน”, จำกัด action ตาม permission และอธิบายว่าความสัมพันธ์เดิมยังอยู่; assignment picker ยังเรียก active-only default |
| **ตรวจสอบ** | database lifecycle/authorization/assignment tests, OpenAPI 409 + operation inventory, static architecture 99 tests, generated-contract tests, focused UI tests, Svelte autofixer และ `svelte-check` |
| **ความยาก** | Medium |

### ✅ M-7. Shared API contract ระหว่าง backend-school และ frontend — read phase เสร็จแล้ว

| | |
|---|---|
| **ไฟล์** | `backend-school/src/api_contract.rs`, `contracts/openapi/school-api.json`, `frontend-school/src/lib/api/generated/school-api.ts`, `backend-school/tests/static_architecture.rs` |
| **ที่ทำ** | ให้ Rust serde DTO + `utoipa` เป็น source of truth, generate OpenAPI/TypeScript แบบ offline และย้าย frontend wire DTO ที่ซ้ำให้ใช้งาน generated schema พร้อม router-derived drift guard |
| **ผลลัพธ์** | ปัจจุบัน contract มี 68 unique operations: 32 auth/authorization และ 36 read-oriented JSON operations ทำให้ backend/frontend drift ถูกตรวจใน CI และ type check |
| **งานต่อ Phase 4** | เพิ่ม mutation operations ทีละกลุ่มหลังตรวจ behavior, permission, status และ response DTO โดยกลุ่ม soft-deactivation เป็นชุดแรกที่เสร็จแล้ว; SSE, WebSocket, health/readiness และ file/binary endpoints ยังอยู่นอก OpenAPI contract โดยตั้งใจ |
| **ความยาก** | Medium |

---

## 🟢 Priority 4: Low — Developer experience

| # | ปัญหา | แก้ไข | ความยาก |
|---|---|---|---|
| L-1 | ไม่มี circuit breaker สำหรับ Neon/Cloudflare | เพิ่ม timeout + exponential backoff ใน clients | Medium |
| ✅ L-2 | `/health` return healthy เสมอ ไม่ตรวจ DB จริง | แยก `/health` เป็น liveness ที่ไม่แตะ DB และเพิ่ม `/ready` สำหรับ `SELECT 1` readiness เพื่อลดการปลุก Neon จาก container healthcheck | Small |
| L-3 | ไม่มี shared type contracts ระหว่าง services | สร้าง `schoolorbit-contracts` workspace crate | Medium |
| L-4 | RBAC มีใน code แต่ไม่ enforce | เพิ่ม `require_role()` middleware factory ใน backend-admin | Small |
| ✅ L-5 | backend-school ยังใช้ Rust module-root แบบ `mod.rs` | migrate เป็น Rust 2018-style module roots (`foo.rs` + `foo/` children) และคง `.rules` ว่าไม่สร้าง `mod.rs` ใหม่ | Small |
| ✅ L-6 | backend-only architecture guards ชุดแรกยังอยู่ใน `frontend-school` static tests | ย้ายกฎ no `mod.rs` และ service-layer handler guard ไป `backend-school/tests/static_architecture.rs` แล้ว | Small |
| ✅ L-7 | backend permission/internal architecture guards ที่เหลือยังปนใน `frontend-school` static tests | ย้าย guard ที่เป็น backend-only เพิ่มเติม เช่น permission helper, cache invalidation, internal auth และ tenant pool resolver ไป `backend-school/tests/static_architecture.rs` แล้ว | Small |

---

## แผนการทำงาน

### Sprint 1 (เริ่มเลย)
**C-1, C-2, C-3, C-4, C-5** + **H-1, H-3, H-4** + **M-5, L-2, L-4**

ทั้งหมด small/medium complexity แก้ security + correctness + รากฐาน testing

### Sprint 2
**M-1** (subdomain header + frontend-school client.ts) — เสร็จแล้ว

*(H-5 เสร็จแล้วใน session 2026-03-24)*

### Sprint 3
**H-2** (monolith split), **M-2** (migration squash), **L-1, L-3**

*(M-3 เสร็จแล้วใน session 2026-03-24)*

---

## ไฟล์สำคัญ

| ไฟล์ | เกี่ยวข้องกับ |
|---|---|
| `backend-admin/src/services/school_service.rs` | C-2, H-2, H-3 |
| `backend-admin/src/main.rs` | H-1 |
| `backend-admin/src/auth/types.rs` | C-5, L-4 |
| `backend-admin/src/middleware/auth.rs` | C-5, L-4 |
| `backend-admin/src/handlers/*.rs` | H-1 |
| `backend-admin/migrations/` | H-4 |
| `backend-school/src/db/migration.rs` | C-1 |
| `backend-school/src/middleware/internal_auth.rs` | C-4 |
| `backend-school/src/middleware/permission.rs` | ✅ H-5 (done) |
| `backend-school/src/db/permission_cache.rs` | ✅ H-5 (done) |
| `backend-school/src/modules/staff/handlers/` | C-3 |
| `backend-school/src/modules/students/` | C-3 |
| `backend-school/src/utils/subdomain.rs` | M-1 |
| `backend-school/src/main.rs` | L-2 |
| `backend-school/src/db/admin_client.rs` | ✅ M-3 (done) |
| `frontend-school/src/lib/api/client.ts` | M-1 (เพิ่ม header) |

---

## Verification

หลังแก้ไขแต่ละจุด:
1. `cargo build` ใน backend-admin และ backend-school ต้องผ่านโดยไม่มี warning
2. `cargo test` (เมื่อเพิ่ม tests หลัง H-1)
3. สร้างโรงเรียนใหม่ผ่าน SSE endpoint และ non-SSE endpoint ทั้งคู่ — ตรวจว่า rollback ทำงานถูกต้องเมื่อ Neon step ล้มเหลว
4. ทดสอบ concurrent provision requests (2 requests พร้อมกัน) เพื่อยืนยัน C-1 fix
5. ตรวจ Neon console ว่าไม่มี orphaned DB หลัง provision failure (C-2)
