# แบบแยกตัวปรับ HTTP request context ของ Auth

**สถานะ** อนุมัติให้ทดลองขอบเขตนี้ใน Draft PR เดียว

ปัจจุบัน `school-auth-http` รวม Auth endpoints กับ adapter ที่ application และ HTTP companions ต้องใช้ทุกครั้งที่อ่าน session หรือ tenant context การแก้ handler จึงอาจทำให้ Cargo คอมไพล์ HTTP companions ที่ใช้แค่ adapter ซ้ำ งานทดลองนี้ให้ consumer ขึ้นกับ crate ที่เป็นเจ้าของ adapter โดยตรง แล้วใช้ผลวัด release build ตรวจว่าหน่วยที่คอมไพล์ลดลงจริง [Workspace](../../../backend-school/Cargo.toml) · [Auth HTTP crate](../../../backend-school/crates/school-auth-http/src/lib.rs)

| เจ้าของ | ความรับผิดชอบ |
| --- | --- |
| `school-auth` | Identity, session และ remembered-device policy, persistence, security state, cache และ `AuthRuntime` ตามเดิม |
| `school-auth-context` | ย้าย `client_address`, `context`, `cookies`, `origin` และ `tenant` พร้อม tests ของแต่ละโมดูล เป็น adapter HTTP กลางที่ใช้ `AuthRuntime`, Authorization และ Tenancy เดิม |
| `school-auth-http` | Session, profile และ consent endpoints, narrow `AuthHttpState` และ `ProfileUpdatePort` |
| `backend-school` | `AppState`, `FromRef`, route assembly, middleware, application adapters และ offline OpenAPI composition |

ให้ middleware ของ application, Auth endpoints และ HTTP companions ที่ใช้ shared adapter import `school-auth-context` โดยตรง กลุ่มที่ใช้เฉพาะ adapter ถอด edge ไปยัง Auth endpoints ออก ให้ business crates ใช้ service และ ports ของเจ้าของตามเดิม ชุดเดียวกันนี้ครอบคลุมทุก caller ที่ตรวจพบทั้ง production และ tests รวมถึง realtime และ public tenant routes [Application router](../../../backend-school/src/app.rs) · [Session middleware](../../../backend-school/src/middleware/session.rs)

การย้ายโมดูลต้องคง precedence ของ origin และ tenant hint, การเลือก client address, cookie และ CSRF wire semantics, session/permission checks, การใช้ cache และ realtime invalidation ตามเดิม Auth profile ยังใช้ application adapter ที่ประกอบคำสั่ง Auth/Files เดิม และ fixture ของ tenant directory ยังตรวจ 404, 503 กับการปิดบัง upstream diagnostics [App profile adapter](../../../backend-school/src/adapters/auth_profile.rs) · [School session tests](../../TESTING.md#school-session-authentication)

เกณฑ์รับงานคือ graph แยกตามเจ้าของครบทุก caller, tests ของ Auth/Tenancy/HTTP ผ่าน และ OpenAPI, generated TypeScript, permission contract และ migrations ตรงกับ baseline สำหรับการเทียบผลคอมไพล์ให้ดู paired warm handler edit และ no-op จาก release profile เดิม การวัดนี้มีขอบเขตที่ Cargo ส่วนแอปยังใช้ binary และ topology เดิม [Implementation plan](../plans/2026-11-23-school-auth-request-context.md)
