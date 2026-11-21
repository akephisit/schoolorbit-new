# แผนย้าย HTTP composition ของ Backend School

ดำเนินงานใน Draft PR เดียวจาก baseline `6d801711025f5c40ed301ea74b770995ea83700d` ย้ายเจ้าของเป็นชุดและคง behavior กับ wire contract ตามแบบที่อนุมัติ [แบบสถาปัตยกรรม](../specs/2026-11-22-school-domain-http-crates-design.md)

## ลำดับงาน

1. บันทึก default และ `api-docs` graph, OpenAPI export และต้นทาง fixture ของแต่ละชุดก่อนย้าย ตรวจ `app.rs`, `http_state.rs`, `api_contract.rs`, imports ใน Rust tests, สัญญาหน้าเว็บ และ guards ที่ตามพาธจริง ใช้ baseline เดียวกันในการวัด Cargo รอบสุดท้าย [Application router](../../../backend-school/src/app.rs) · [Architecture guards](../../../backend-school/tests/static_architecture.rs)
2. ย้าย `school-question-bank-http` ก่อน ตรวจ protected nesting ของ Question Bank, concrete state, รายการ endpoints และ typed responses รวมถึงสิทธิ์และการส่งไฟล์ เก็บตัวอย่างขั้นตอนย้าย route factory, test imports และการตรวจ OpenAPI ไว้สำหรับชุดถัดไป [Question Bank source](../../../backend-school/crates/school-question-bank/src/lib.rs)
3. ย้าย `school-notifications-http` แล้วตรวจ inbox, Push devices และ SSE endpoint รวมสิทธิ์ตามผู้ใช้และ tenant และการปิด stream เมื่อ session ใช้ต่อไม่ได้ ตรวจ worker และ runtime wiring ที่ประกอบอยู่ใน application และยืนยันว่า producers กับ test support ใช้ `school-notifications` ได้จาก graph ใหม่ [Notifications source](../../../backend-school/crates/school-notifications/src/lib.rs) · [Notification fixtures](../../TESTING.md#inbox-first-notification-regressions)
4. ย้ายเจ้าของที่เหลือในลำดับต่อไปนี้ ตรวจร่วมเมื่อมี route, provider, generated permission หรือ OpenAPI fixture ที่ใช้ด้วยกัน

   | ชุด | เจ้าของและจุดตรวจที่ต้องรักษา |
   | --- | --- |
   | 1 | `school-administration-http`, `school-admission-http`, `school-attendance-http`, `school-behavior-http` ตรวจ public/protected กลุ่มย่อย, multipart/ไฟล์, scope การเข้าเรียนและคะแนน |
   | 2 | `school-budget-http`, `school-calendar-http`, `school-fonts-http`, `school-navigation-http` ตรวจ schema registration ของ Budget, public/management routes ของ Calendar, access และเมนูตามสิทธิ์ |
   | 3 | `school-student-care-http`, `school-students-http` รวม Parents, `school-supervision-http` ตรวจ scopes ของนักเรียนและผู้ปกครอง, PII, และ route guards ของการประเมิน |

5. ตรวจรายชื่อ 13 companions และ peer boundaries ทั้ง workspace ทบทวน source และ dependency graph, release/runtime topology, คำสั่ง verification และผลวัด เทียบกับ baseline ก่อนพร้อมรวมงาน บันทึกผลที่ตรวจได้ใน PR แล้วนำ workflow spec และ plan ออกจาก active tree ตามวงจรเอกสาร [Verification matrix](../../../.rules#11-verification-matrix) · [Workflow artifact lifecycle](../../../.rules#0-rule-ownership-and-documentation-policy)

## วิธีทำแต่ละชุด

- สร้าง workspace member และลง dependency ภายในด้วย versions/lints ที่ workspace เป็นเจ้าของ ตรวจ normal, development และ `api-docs` edges ย้าย handlers, route factories และ state โดยรักษา path declarations, types และลำดับ composition เรียก service/port เจ้าของเดิมผ่าน interface แคบ และลบ domain HTTP module ที่ถูกแทนที่
- ปรับ application imports, `FromRef`, routing, offline schema registration และ fixtures ที่อ้างเจ้าของเดิม ย้าย permission selections และ run generator เมื่อตำแหน่งผู้ใช้เปลี่ยน ตรวจว่า permission contract, DB registry และ TypeScript คงเดิมทุกไบต์ ตรวจว่าล็อกเปลี่ยนเฉพาะ graph ที่จำเป็น
- ทำให้ static tests ตาม implementation ใหม่และตรวจข้อรับประกันเดิม เช่น middleware ที่ห่อ route, scope, tenant policy, narrow state และ transport dependency direction เพิ่มเฉพาะ regression ที่การย้ายเสี่ยงหลุด และบันทึกผลกับไฟล์ที่ย้ายใน checkpoint ของ Draft PR [Backend School checks](../../TESTING.md#backend-school) · [Permission checks](../../TESTING.md#permission-contract)

## ตรวจรับและวัดผล

- ในแต่ละชุด ใช้ `cargo fmt --all -- --check`, `cargo check --workspace --all-targets --locked` และคำสั่งเดียวกันกับ `--features api-docs` จาก `backend-school` รัน focused crate, route, static architecture และ disposable PostgreSQL fixtures ตามเจ้าของ รวม allowed/denied tenant/resource, session และ CSRF ตามเส้นทางที่ย้าย ก่อนรวมงานให้รัน workspace production Clippy ทั้ง default และ `api-docs` ตาม verification owner [Testing commands](../../TESTING.md#local-verification-and-github-delivery)
- รัน `npm --prefix frontend-school run check:permissions` เมื่อเปลี่ยน selections และตรวจ offline OpenAPI จาก executable ของ baseline และ candidate ใช้ `node scripts/generate-api-contracts.mjs --check` และเทียบ tracked OpenAPI กับ generated TypeScript ทุกไบต์ ตรวจ feature forwarding และ schema registration ทั้งผล export และ suite ของ API [API contract checks](../../TESTING.md#api-contract)
- เปรียบเทียบ release builds บนเครื่องเดียวกัน ใช้ Rust, target path, features, flags, quota, Cargo jobs และสภาพ cache ตั้งต้นที่เทียบกันได้ สำหรับ warm handler edit ให้ prime แต่ละ graph นอกช่วงจับเวลา แล้วใช้การแก้ handler ที่เทียบกันได้ในตำแหน่งเดิมกับ companion ใหม่ วัดอย่างน้อยสามรอบต่อกรณี ใช้ no-op จาก target อุ่นเป็น control และวัด cold build ด้วย target ใหม่ทั้งคู่พร้อมระบุสภาพ dependency/compiler cache ที่ใช้ เก็บค่ากลางและช่วงของเวลา Cargo จำนวนหน่วยที่ rebuild และเวลา link แยกจากการสร้าง image และ deploy หลักฐานที่รายงานต้องบอกว่า phase และ binary ใดถูกวัด [Backend build benchmark recipes](../../TESTING.md#backend-school) · [Release build commands](../../../backend-school/scripts/build_release.sh)
