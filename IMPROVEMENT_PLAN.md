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

### C-1. Race condition ใน lazy migration/permission sync
| | |
|---|---|
| **ไฟล์** | `backend-school/src/db/migration.rs` |
| **ปัญหา** | `MigrationTracker` ใช้ `RwLock<HashSet>` — check และ execute ไม่ atomic ถ้า 2 request มาพร้อมกันตอน cold start จะรัน migration ซ้ำ อาจทำให้ data seed ซ้ำหรือ error |
| **แก้ไข** | เปลี่ยนเป็น `DashMap<String, Arc<tokio::sync::Mutex<bool>>>` เพื่อให้แต่ละ school มี lock ของตัวเอง |
| **ความยาก** | Small |

### C-2. Broken rollback เมื่อ provisioning ล้มเหลว
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs` (บรรทัด 60-66) |
| **ปัญหา** | `let _ = async { neon_client.delete_database(...) };` สร้าง future แต่ **ไม่ได้ `.await`** ทำให้ Neon database ถูกสร้างค้างโดยไม่มีการลบ — เสียเงินและ subdomain ติด |
| **แก้ไข** | เพิ่ม `.await` และเพิ่ม rollback สำหรับ school record ที่ค้างใน DB |
| **ความยาก** | Small (fix bug) / Medium (rollback สมบูรณ์) |

### C-3. PII (national_id) ยังไม่ได้เข้ารหัสครบ
| | |
|---|---|
| **ไฟล์** | `backend-school/src/modules/staff/handlers/`, `backend-school/src/modules/students/` |
| **ปัญหา** | `field_encryption.rs` มีพร้อมแล้ว แต่ staff/student handlers ยังเขียน national_id เป็น plaintext — ถ้า DB รั่ว PII ทั้งหมดถูกอ่านได้ ผิด PDPA |
| **แก้ไข** | Apply `field_encryption::encrypt/decrypt` ใน write+read paths ตาม `TODO_ENCRYPTION.md` |
| **ความยาก** | Medium |

### C-4. INTERNAL_API_SECRET ไม่ timing-safe
| | |
|---|---|
| **ไฟล์** | `backend-school/src/middleware/internal_auth.rs`, `backend-admin/src/handlers/internal.rs` |
| **ปัญหา** | `auth_header != internal_secret` เปรียบเทียบแบบ naive — เสี่ยง timing attack และ rotate secret ได้ยากเพราะใช้ secret เดียวกับทุก caller |
| **แก้ไข** | ใช้ `subtle::ConstantTimeEq` และเพิ่ม `X-Internal-Caller` header เพื่อแยก secret ต่อ caller |
| **ความยาก** | Small |

### C-5. Admin JWT claims มี school-scoped fields ที่ไม่จำเป็น
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/auth/types.rs`, `backend-admin/src/middleware/auth.rs` |
| **ปัญหา** | `Claims` struct มี `school_id` และ `subdomain` ทั้งที่ admin ไม่ควรมี และ middleware ไม่ validate `role` field เลย |
| **แก้ไข** | แยก `AdminClaims` struct ออกมา ลบ school fields และเพิ่ม role check ใน middleware |
| **ความยาก** | Small |

---

## 🟠 Priority 2: High — แก้ในระยะสั้น

### H-1. OnceLock DB pool ป้องกัน testing
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/main.rs`, `backend-admin/src/handlers/*.rs` |
| **ปัญหา** | handlers แต่ละไฟล์มี `static DB_POOL: OnceLock<PgPool>` ของตัวเอง ทดสอบไม่ได้ (`OnceLock::set` ครั้งที่ 2 silent fail) ต่างจาก backend-school ที่ใช้ `State<AppState>` ถูกต้อง |
| **แก้ไข** | สร้าง `AppState { pool: PgPool }` และส่งผ่าน `.with_state()` — mechanical refactor ไม่เปลี่ยน behavior |
| **ความยาก** | Medium |

### H-2. `school_service.rs` 783 บรรทัด มี logic ซ้ำ
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs` |
| **ปัญหา** | provisioning logic ซ้ำกัน 2 ชุด (SSE/non-SSE) — bug C-2 เกิดเพราะแก้ใน SSE version แต่ลืม non-SSE |
| **แก้ไข** | แยก `ProvisioningOrchestrator` ที่รับ logger callback ทั้ง SSE และ non-SSE handler เรียกตัวเดียวกัน |
| **ความยาก** | Large |

### H-3. `schools.config` เป็น untyped `serde_json::Value`
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/models/school.rs`, `backend-admin/src/services/school_service.rs` |
| **ปัญหา** | typo เช่น `"db_id"` vs `"db-id"` ไม่ถูก catch ที่ compile time — return `None` เงียบๆ |
| **แก้ไข** | สร้าง `SchoolConfig` struct และใช้ `sqlx::types::Json<SchoolConfig>` |
| **ความยาก** | Small |

### H-4. Migration gap — ไม่มี migration 003 ใน backend-admin
| | |
|---|---|
| **ไฟล์** | `backend-admin/migrations/` |
| **ปัญหา** | ลำดับข้าม 002 → 004 ถ้าใครสร้าง `003_xxx.sql` ในอนาคต SQLx จะ reject ทุก tenant |
| **แก้ไข** | สร้าง `003_placeholder.sql` ด้วย `SELECT 1;` พร้อม comment |
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

### M-1. Subdomain extraction จาก `Origin` header ขาดความยืดหยุ่น
| | |
|---|---|
| **ไฟล์** | `backend-school/src/utils/subdomain.rs` |
| **ปัญหา** | ไม่รองรับ non-browser clients (Postman, mobile, scripts), local dev และ custom domains ในอนาคต |
| **แก้ไข** | เพิ่ม `X-School-Subdomain` header เป็น first-class input (ก่อน Origin/Referer) และต้องเพิ่มใน `frontend-school/src/lib/api/client.ts` ด้วย |
| **ความยาก** | Small |

### M-2. 51 migrations ไม่มี squash strategy
| | |
|---|---|
| **ไฟล์** | `backend-school/migrations/` |
| **ปัญหา** | provisioning ใหม่รัน migration ทั้ง 51 ไฟล์ ยิ่งนาน migration ยิ่งช้า |
| **แก้ไข** | สร้าง squash migration หลังจาก tenant ทุกตัว migrate ครบ 051 และ document migration policy |
| **ความยาก** | Medium |

### ✅ M-3. แยก backend-school ออกจาก admin DB (HTTP separation) — เสร็จแล้ว
| | |
|---|---|
| **ไฟล์** | `backend-school/src/db/admin_client.rs` (ใหม่), `backend-school/src/main.rs`, `backend-school/src/db/school_mapping.rs` |
| **ที่ทำ** | ลบ `admin_pool: PgPool` ออกจาก `AppState` และแทนด้วย `admin_client: Arc<AdminClient>` — HTTP client ที่เรียก `GET /internal/schools/{subdomain}` บน backend-admin แทนการ query DB ตรง เพิ่ม endpoint `PUT /internal/schools/{subdomain}/migration-status` สำหรับ write-back |
| **ผลลัพธ์** | backend-school ไม่มี admin DB credentials อีกต่อไป — isolation สมบูรณ์ |

### M-5. `println!` แทน structured logging ใน backend-admin
| | |
|---|---|
| **ไฟล์** | `backend-admin/src/services/school_service.rs`, `backend-admin/src/clients/*.rs` |
| **ปัญหา** | 111+ `println!` calls — ไม่สามารถ filter level หรือ aggregate ใน Grafana/Datadog ได้ และอาจ log sensitive data |
| **แก้ไข** | เพิ่ม `tracing` ใน Cargo.toml และแทนที่ทุก `println!`/`eprintln!` |
| **ความยาก** | Small |

---

## 🟢 Priority 4: Low — Developer experience

| # | ปัญหา | แก้ไข | ความยาก |
|---|---|---|---|
| L-1 | ไม่มี circuit breaker สำหรับ Neon/Cloudflare | เพิ่ม timeout + exponential backoff ใน clients | Medium |
| L-2 | `/health` return healthy เสมอ ไม่ตรวจ DB จริง | เพิ่ม `SELECT 1` ping + 503 on failure | Small |
| L-3 | ไม่มี shared type contracts ระหว่าง services | สร้าง `schoolorbit-contracts` workspace crate | Medium |
| L-4 | RBAC มีใน code แต่ไม่ enforce | เพิ่ม `require_role()` middleware factory ใน backend-admin | Small |

---

## แผนการทำงาน

### Sprint 1 (เริ่มเลย)
**C-1, C-2, C-3, C-4, C-5** + **H-1, H-3, H-4** + **M-5, L-2, L-4**

ทั้งหมด small/medium complexity แก้ security + correctness + รากฐาน testing

### Sprint 2
**M-1** (subdomain header + frontend-school client.ts)

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
