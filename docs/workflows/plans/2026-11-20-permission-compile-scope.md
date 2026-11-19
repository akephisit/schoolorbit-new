# แผนลดการคอมไพล์ Rust ซ้ำเมื่อแก้ permission

**สถานะ** ร่างสำหรับคุยต่อใน Draft PR หยุดที่แผนก่อนเริ่มทดลองหรือแก้โค้ด

## เป้าหมายและข้อมูลที่มี

ลดจำนวนหน่วยที่ Cargo ต้องคอมไพล์ซ้ำเมื่อเพิ่มหรือแก้ permission ของฟีเจอร์หนึ่งใน `backend-school` โดยให้ `contracts/permissions.json` และ `scripts/generate-permissions.mjs` เป็นเจ้าของ contract และไฟล์ที่ generate ตามเดิม คงค่า permission, การตีความ `WILDCARD`, การตัดสินสิทธิ์ตาม resource scope และพฤติกรรม DB/Workflow รวมถึงผลลัพธ์ TypeScript งานทดลองอยู่ที่ dependency ใน Rust กับการ generate constants เท่านั้น นอกขอบเขตคือ grant หรือข้อมูลใหม่ การแก้ migration, linker, release profile และ deployment [Permission contract](../../../contracts/permissions.json) · [Generator](../../../scripts/generate-permissions.mjs)

- ตัวอย่าง CI ล่าสุดที่ตรวจ 8 งานใช้เวลา link จริง 0.51–0.66 วินาที งาน Annual Budget ใช้เวลา Cargo 408.72 วินาที มี sccache hit 7 และ miss 41 สำหรับไลบรารี Rust โดย PR นั้นเพิ่ม permission ของ Budget 4 รายการ [Annual Budget PR](https://github.com/akephisit/schoolorbit-new/pull/263) · [Annual Budget CI](https://github.com/akephisit/schoolorbit-new/actions/runs/44029051865/job/133856458532)
- การไล่จาก manifest ที่ commit `1f6890a8d70a773530144e72518c3f6dd67d25c5` พบ 41 packages ที่อาจได้รับผลทางตรงหรือทางอ้อมจาก `school-permissions` ส่วน CI อีกงานที่วัดขณะ cache อุ่นใช้เวลา Cargo 65.87 วินาที มี hit 46 และ miss 2 ตัวเลขจาก manifest แสดงเส้นทางที่เป็นไปได้ ส่วนผลจาก CI สองงานมาจากคนละ build จึงต้องทดลองแบบจับคู่เพื่อวัดสาเหตุและขนาดของผล [CI ที่ cache อุ่น](https://github.com/akephisit/schoolorbit-new/actions/runs/44300267303/job/134788087046)

## ทดลองตัด dependency จาก `school-authorization` ก่อน

1. เก็บ reverse graph ที่ baseline ด้วย `cargo tree --manifest-path backend-school/Cargo.toml --locked -p backend-school -e normal,build -i school-permissions` และรันด้วย `--workspace` แทน `-p backend-school` เพื่อดูภาพรวม ใช้ target และ features ตาม release builder แล้วแยก dev edges ออก ตรวจผลกับหน่วยที่ Cargo ใช้จริงใน release ของ `backend-school` และ `migrate_tenant_schema` [Release builder](../../../backend-school/scripts/build_release.sh)
2. ให้ generator ใช้ wildcard ที่ผ่าน validation แล้วสร้างเฉพาะ `WILDCARD` ใน `school-authorization/src/permission_codes_generated.rs` และให้ `permissions.rs` ใช้ค่านั้นจากโมดูลภายใน ผลลัพธ์ขึ้นกับ constant ที่เลือกเท่านั้น ใช้ write-if-bytes-change เดิมและเพิ่มไฟล์เข้า `--check` สแกน imports อีกครั้งก่อนย้าย `school-permissions` ไปไว้ใน `[dev-dependencies]` เพราะ unit และ integration tests ใช้ constants อื่นสำหรับ fixtures การทดสอบอาจยัง rebuild ผ่าน dev graph [Production permissions](../../../backend-school/crates/school-authorization/src/permissions.rs) · [Scope fixtures](../../../backend-school/crates/school-authorization/src/resource_access_policy.rs) · [Acceptance test](../../../backend-school/crates/school-authorization/tests/resource_acceptance.rs)
3. คง dependency ที่อ่าน `ALL_PERMISSIONS` ไว้สำหรับ `school-migrations::sync_permissions` และ Workflow ที่ตรวจสิทธิ์จัดการรอบงาน รวมถึง direct edges เดิมของ application และ domain crates ที่ยัง import constants กลาง ไล่กราฟซ้ำเพื่อหาเส้นทางที่หายไปและเส้นทางอ้อมที่ยังพา registry มาถึง package เดิม ถ้ารูปแบบนี้ผ่านการทดลอง ให้นำข้อกำหนดเรื่องตำแหน่ง import ใน `.rules` มาคุยพร้อมการเปลี่ยนโค้ด [DB permission sync](../../../backend-school/crates/school-migrations/src/lib.rs) · [Workflow policy](../../../backend-school/crates/school-workflow/src/policy.rs) · [Permission rules](../../../.rules#permission-contract)

## วิธีวัดผลและเกณฑ์ตรวจ

- เปรียบเทียบ baseline กับ pilot บนเครื่องและ build context เดียวกัน ใช้ toolchain, target, release flags, CPU quota, Cargo jobs และสภาพ cache เริ่มต้นที่เทียบกันได้ ใช้ `build_release.sh` เป็นตัวอ้างอิงสำหรับคำสั่งและลำดับ build รวม `migrate_tenant_schema` เก็บเวลา Cargo รวมและแยกผลของแต่ละ binary [คู่มือ benchmark](../../TESTING.md#backend-school) · [Release builder](../../../backend-school/scripts/build_release.sh)
- ใช้ permission fixture ชุดเดียวกันเพิ่ม code สำหรับการทดลองใน checkout ชั่วคราวทั้งสองฝั่ง สร้าง output ก่อนช่วงจับเวลาและวัดเทียบกับกรณี source เดิม ใช้ target ที่ prime แต่ละ graph แล้วและทำซ้ำอย่างน้อย 3 รอบต่อกรณี สลับลำดับการวัดและคืน fixture ก่อน prime รอบถัดไป เก็บค่ากลางและช่วงของเวลา Cargo กับรายชื่อและจำนวนหน่วยที่ `fresh: false` จาก Cargo JSON แยกจากจำนวน compile จริงเมื่อเปิด sccache ซึ่งต้องดู hit/miss ต่อรอบประกอบ
- สำหรับ contract เดียวกัน ให้ Rust registry กลาง, `contracts/permissions.lock.json` และ TypeScript registry ตรงกับผลจาก baseline ทุกไบต์ ทั้งกรณีเดิมและกรณีเพิ่ม permission ใน fixture ยืนยันว่า output ของ `school-authorization` ไม่มี digest ของ contract รวมและไฟล์คงทั้งไบต์กับเวลาเขียนเดิมเมื่อ permission อื่นเปลี่ยน ทดสอบ `--check` ให้จับไฟล์เฉพาะ crate ที่หายหรือผิด แล้วให้ผ่านเมื่อ generate คืน [Generator tests](../../../scripts/tests/generate-permissions.test.mjs)
- ยืนยันว่า wildcard, การอนุญาตและปฏิเสธ, exact/tree และการรวมหลาย scope ให้ผลเดิม ใช้ PostgreSQL fixture ของ repo รัน `db::migration_tests` ที่ตรวจการรักษา grant เดิม การ rollback เมื่อ insert ล้มเหลว และการ retry จากนั้นตรวจการเลือก management permission ของ Workflow [DB sync tests](../../../backend-school/src/db/migration_tests.rs) · [Permission testing](../../TESTING.md#permission-contract)

คำสั่งจากราก repo สำหรับช่วงทดลองเมื่อได้ข้อสรุป

```bash
npm --prefix frontend-school run generate:permissions
npm --prefix frontend-school run check:permissions
npm --prefix frontend-school run test:permissions
cargo test --manifest-path backend-school/Cargo.toml --locked --test static_architecture
cargo test --manifest-path backend-school/Cargo.toml --locked -p school-authorization --lib
./scripts/test_backend_school.sh --package school-authorization --test resource_acceptance -- --test-threads=1
./scripts/test_backend_school.sh db::migration_tests -- --test-threads=1
./scripts/test_backend_school.sh permission_coverage -- --test-threads=1
./scripts/test_backend_school.sh --package school-workflow --lib -- --test-threads=1
```

สำหรับ Draft PR ครั้งนี้ ใช้ `node --test frontend-school/tests/static/documentation-policy.test.mjs` และ `git diff --check` เพื่อตรวจแผน [Verification matrix](../../../.rules#11-verification-matrix)

## ทางเลือกหลังได้ผลทดลอง

ถ้า direct edges ที่ import เฉพาะ constants ยังทำให้หลาย package rebuild ให้พิจารณา generate รายการ constants ตามการใช้งานจริงใน package เจ้าของโค้ด ตัวอย่างตำแหน่งไฟล์ `src/permission_codes_generated.rs` ใน crates เดิมมีดังนี้

| Package | ตัวอย่าง constants ที่ใช้จริง |
| --- | --- |
| `school-budget` | `BUDGET_READ_SCHOOL`, `BUDGET_ADJUSTMENT_CREATE_SCHOOL`, `BUDGET_PAYMENT_EXECUTE_SCHOOL` |
| `school-attendance` | `ATTENDANCE_MANAGE_SCHOOL` และ `BEHAVIOR_MANAGE_SCHOOL` ที่ handler `create_special` ตรวจเมื่อมี scoring |
| `school-behavior` | `BEHAVIOR_READ_ASSIGNED`, `BEHAVIOR_MANAGE_SCHOOL` |

รายการแยกตาม consumer จำกัดการเปลี่ยนไฟล์ได้ละเอียดและครอบคลุมกรณีใช้ code ข้ามโดเมน แต่ต้องดูแล mapping และการตรวจ drift เพิ่ม การจัดตามโมดูล permission ลดจำนวนรายการที่ต้องระบุ แต่อาจทำให้ไฟล์เปลี่ยนเมื่อเพิ่ม code ที่ consumer ยังไม่ได้ใช้ ทั้งสองแบบได้ผลต่อ Cargo เมื่อไม่มี dependency ทางอื่นที่พา registry กลับเข้ามา [Budget policy](../../../backend-school/crates/school-budget/src/policy.rs) · [Attendance handlers](../../../backend-school/crates/school-attendance/src/http/handlers.rs) · [Behavior policy](../../../backend-school/crates/school-behavior/src/policy.rs)

คุยต่อว่าจะเริ่มทดลอง `school-authorization` หรือไม่ และจะถือว่าประหยัดคุ้มเมื่อเวลา Cargo ลดลงเท่าใดเมื่อเทียบกับความแกว่งของตัวอย่าง หลังทดลองให้ใช้จำนวนหน่วยที่ลดลง เวลาที่วัดซ้ำได้ และค่าใช้จ่ายดูแล generator ตัดสินว่าจะจบที่ pilot หรือขยายไปยัง Budget, Attendance และ Behavior พักการตัดสินใจเรื่องขยายทั้ง workspace ไว้จนกว่าจะมีผลวัดและได้คุยกันอีกครั้ง
