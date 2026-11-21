# แบบแยก HTTP crates ของ Backend School

**สถานะ** ขอบเขตได้รับอนุมัติให้ดำเนินงานเป็นชุดใน Draft PR เดียว

## ผลลัพธ์ที่ต้องการ

HTTP handlers ของแต่ละโดเมนมีขอบเขตคอมไพล์แยกจาก service และ persistence ของโดเมน การแก้ handler จึงมีโอกาสใช้ผลคอมไพล์ของ business crate และผู้ใช้บริการเดิมซ้ำได้ พิสูจน์ผลจาก release builds แบบเทียบคู่ กติกาเดียวกันใช้กับเจ้าของ HTTP ที่มีหน้าที่เทียบเท่ากันใน Backend School [Workspace](../../../backend-school/Cargo.toml) · [Development rules](../../../.rules#backend-school-cargo-workspace)

## ขอบเขตเจ้าของโค้ด

| ชั้น | ความรับผิดชอบ |
| --- | --- |
| `school-<domain>` | Service, domain DTO, validation เชิงธุรกิจ, resource policy, repository, transaction และ ports ที่โดเมนเป็นเจ้าของ |
| `school-<domain>-http` | Axum extractors, request หรือ response types ที่ใช้เฉพาะ HTTP, handlers, path declarations, route factories และ state เท่าที่ transport ใช้ |
| `backend-school` | `AppState`, การต่อ narrow state ด้วย `FromRef`, global route prefixes, public/protected partition, shared middleware, adapters ข้ามโดเมน และ offline OpenAPI composition |

Companion ใหม่ใช้ `src/handlers.rs`, `src/routes.rs` และ `src/state.rs` หรือใช้ submodules ภายใต้โมดูลเหล่านี้เมื่อเจ้าของมีหลายชุด routes แต่ละ factory คืน `Router<DomainHttpState>` และ application bind state ก่อนประกอบ shared session boundary HTTP เรียก typed service หรือ port จากเจ้าของเดิม Business crate และ HTTP companion รับเฉพาะ dependencies ที่ใช้ และ crate ภายในรับ state ของตัวเองโดยคง `AppState` ไว้ที่ application [Timetable HTTP peer](../../../backend-school/crates/school-academic-timetable-http/src/lib.rs) · [Application state binding](../../../backend-school/src/http_state.rs) · [Router composition](../../../backend-school/src/app.rs)

ใช้ session, tenant context และ cookie/origin จาก `school-auth-http` ส่วน response envelopes ใช้ `school-http` ตามเดิม Permission constants ย้ายไปยัง consumer ที่ใช้จริงผ่านรายการเลือกและ generator ของ repo DTO ที่อธิบาย domain ยังอยู่กับ service รวมถึง schema derives ที่ contract ต้องใช้ ชื่อ schema, วิธีลงทะเบียน และลำดับใน offline OpenAPI ต้องให้ผลไบต์เดิมหลังย้าย [Permission selections](../../../scripts/permission-code-consumers.json) · [OpenAPI composition](../../../backend-school/src/api_contract.rs)

ย้าย 13 เจ้าของที่ยังมี HTTP composition ใน business crate ได้แก่ Question Bank, Notifications, Administration, Admission, Attendance, Behavior, Budget, Calendar, Fonts, Navigation, Student Care, Students และ Supervision Parents อยู่ใต้ Students และ Administration ใช้ companion เดียวรวม settings, facility, supply catalog และ lookup เจ้าของที่แยกแล้วรักษาการจัดกลุ่มเดิม เช่น Staff HTTP รวม Staff, Achievement, Leave และ FileTransfer ปรับ layout ใน companion ที่มีอยู่เมื่อพบปัญหาขอบเขตจริง [Application route inventory](../../../backend-school/src/app.rs) · [Staff HTTP peer](../../../backend-school/crates/school-staff-http/src/lib.rs)

สำหรับ Notifications ให้ย้าย HTTP composition รวม SSE endpoint และการแปลง stream เป็น HTTP response ส่วนการส่ง inbox, receipts, transport signals, Web Push delivery, workers, presentation lifecycle และการตรวจ source validity ใช้เจ้าของเดิม Producers เรียก service ต่อผ่าน dependency เดิม Cross-domain ports รักษาทิศทางเดิม เมื่อ handler กับ service ต้องแชร์ข้อมูลให้ใช้ API แคบที่เจ้าของข้อมูลรับผิดชอบ [Notifications owner](../../../backend-school/crates/school-notifications/src/lib.rs) · [Existing routes](../../../backend-school/crates/school-notifications/src/http/routes.rs)

## เกณฑ์รับงาน

- รักษาเส้นทางและ method, route groups, session และ tenant isolation, resource scopes, status และ envelopes, side effects, สัญญาหน้าเว็บ ตลอดจนไฟล์ OpenAPI และ generated TypeScript สำหรับ contract เดิมทุกไบต์
- คง migration และ schema, การจัดการข้อมูลและวงจร runtime เดิม `backend-school` ยังเป็น executable เดียวสำหรับแอปปัจจุบัน พร้อม release builder และ deployment topology เดิม
- ย้ายครบทุกเจ้าของในขอบเขต และให้ architecture guards ตรวจทิศทาง dependency, ownership, concrete routes และ state การแก้ guard ที่อ้างพาธเดิมต้องรักษาสิ่งที่ตรวจให้ครบ
- ตรวจ behavior ผ่าน route tests และ fixtures ของ service/tenant ตามขอบเขต และวัด release baseline กับ candidate ในสภาพแวดล้อมเดียวกัน โดยแยก warm handler edit, no-op และ cold build [Verification recipes](../../TESTING.md#backend-school) · [Implementation plan](../plans/2026-11-22-school-domain-http-crates.md)
