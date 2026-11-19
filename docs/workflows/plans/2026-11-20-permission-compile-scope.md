# แผนลดการคอมไพล์ Rust ซ้ำเมื่อแก้ permission

**สถานะ** ผู้ใช้ให้เริ่มงานแล้ว เริ่มทดลองและเตรียม candidate ใน Draft PR การรวมงานขึ้นกับผลวัดและการตรวจพฤติกรรมสิทธิ์

## เป้าหมายและข้อมูลที่มี

ลดจำนวนหน่วยที่ Cargo ต้องคอมไพล์ซ้ำเมื่อเพิ่มหรือแก้ permission ของฟีเจอร์หนึ่ง หากผลวัดคุ้ม ให้ย้ายผู้ใช้ permission constants ทั้ง application และ crates ใน `backend-school` ไปใช้รูปแบบเดียวกันในรอบส่งมอบเดียว `contracts/permissions.json` และ `scripts/generate-permissions.mjs` ยังเป็นเจ้าของ contract และไฟล์ที่ generate คงค่า permission, การตีความ `WILDCARD`, การตัดสินสิทธิ์ตาม resource scope, พฤติกรรม DB/Workflow และผลลัพธ์ TypeScript ขอบเขตการเปลี่ยนคือการสร้าง constants และ dependency ใน Rust ส่วน grant, ข้อมูล, migration, linker, release profile และ deployment ใช้ของเดิม [Permission contract](../../../contracts/permissions.json) · [Generator](../../../scripts/generate-permissions.mjs)

- ตัวอย่าง CI ล่าสุดที่ตรวจ 8 งานใช้เวลา link จริง 0.51–0.66 วินาที งาน Annual Budget ใช้เวลา Cargo 408.72 วินาที มี sccache hit 7 และ miss 41 สำหรับไลบรารี Rust โดย PR นั้นเพิ่ม permission ของ Budget 4 รายการ [Annual Budget PR](https://github.com/akephisit/schoolorbit-new/pull/263) · [Annual Budget CI](https://github.com/akephisit/schoolorbit-new/actions/runs/44029051865/job/133856458532)
- ที่ commit `1f6890a8d70a773530144e72518c3f6dd67d25c5` การไล่ normal dependencies จาก manifest พบ 41 packages ที่ `school-permissions` อาจส่งผลถึง เหลือ 36 หากตัดเส้นตรงจาก `school-authorization` เพียงจุดเดียว นี่คือจำนวนจากกราฟ ส่วนจำนวนที่ Cargo คอมไพล์ซ้ำต้องวัดจาก build จริง CI อีกงานที่ cache อุ่นใช้เวลา Cargo 65.87 วินาที มี hit 46 และ miss 2 งาน CI ทั้งสองมาจากคนละ build จึงต้องใช้การวัดแบบจับคู่เพื่อประเมินผล [CI ที่ cache อุ่น](https://github.com/akephisit/schoolorbit-new/actions/runs/44300267303/job/134788087046)

## การทดลอง `WILDCARD` และรูปแบบที่จะใช้ทั้ง workspace

1. เก็บ reverse graph ที่ baseline ด้วย `cargo tree --manifest-path backend-school/Cargo.toml --locked -p backend-school -e normal,build -i school-permissions` แล้วใช้ `--workspace` เพื่อดูทั้ง workspace และตรวจ dev edges แยก ใช้ target และ features ตาม release builder แล้วตรวจเทียบกับหน่วยที่ Cargo ใช้จริงใน release ของ `backend-school` และ `migrate_tenant_schema` [Release builder](../../../backend-school/scripts/build_release.sh)
2. ใน checkout ทดลอง ให้ generator สร้างเฉพาะ `WILDCARD` ใน `school-authorization/src/permission_codes_generated.rs` จาก contract ที่ผ่าน validation แล้ว ให้ `permissions.rs` อ่านจากโมดูลภายใน และย้าย `school-permissions` ไปอยู่ใน `[dev-dependencies]` ชั่วคราวสำหรับ tests ที่ใช้ constants อื่น รันการวัดเพื่อดูผลของ dependency จุดเดียวเป็นข้อมูลสำหรับวัดตัวเลือกครบทั้ง workspace ขั้นนี้เป็นงานทดลองใน draft [Production permissions](../../../backend-school/crates/school-authorization/src/permissions.rs) · [Scope fixtures](../../../backend-school/crates/school-authorization/src/resource_access_policy.rs) · [Acceptance test](../../../backend-school/crates/school-authorization/tests/resource_acceptance.rs)
3. สำหรับตัวเลือกที่จะส่งมอบ ให้สำรวจ imports และ Cargo dependencies ใน application และทุก crate ที่เกี่ยวข้อง ใช้ generator เดิมสร้าง `src/permission_codes_generated.rs` ตาม code ที่ production ใช้จริง รวม code ข้ามโดเมน Unit fixtures จะ re-export constants ที่ใช้ร่วมกับ production ส่วน fixtures ของ Cargo integration test แต่ละ target ใช้ไฟล์ `permission_codes_generated.rs` ของ target นั้นจาก contract เดียวกัน เพราะ Cargo คอมไพล์แต่ละ target เป็นคนละ crate Constants ที่ใช้เฉพาะใน `integration-test-support` อยู่ในโมดูลที่ Cargo เปิดพร้อม feature นั้น วิธีนี้เก็บโมดูล production ไว้เป็น private และจำกัด dependency ของ fixtures ตาม target และ feature โดยค่าของ permission และผลการตรวจสิทธิ์เท่าเดิม [Permission rules](../../../.rules#permission-contract)
4. ให้ `school-migrations::sync_permissions`, Workflow ที่ตรวจเงื่อนไขจัดการรอบงาน และ fixtures ที่ยืนยันรายการ registry ใช้ `ALL_PERMISSIONS` สำหรับอ่าน metadata ทั้งชุด ส่วน constants ที่เจ้าของเหล่านี้ใช้ยังมาจากโมดูลที่ generate ของตัวเอง เก็บ direct dependency บน `school-permissions` เฉพาะเป้าหมายที่อ่าน metadata จริง ตรวจเส้นทางอ้อมที่ยังพา registry เข้ามา และปรับ `.rules` ให้ตรงกับ convention เดียวกันในรอบส่งมอบ [DB permission sync](../../../backend-school/crates/school-migrations/src/lib.rs) · [Workflow policy](../../../backend-school/crates/school-workflow/src/policy.rs)

ตัวอย่างรายการที่ generator เลือกจาก contract ให้แต่ละ package

| Package | ตัวอย่าง constants ที่ใช้จริง |
| --- | --- |
| `school-budget` | `BUDGET_READ_SCHOOL`, `BUDGET_ADJUSTMENT_CREATE_SCHOOL`, `BUDGET_PAYMENT_EXECUTE_SCHOOL` |
| `school-attendance` | `ATTENDANCE_MANAGE_SCHOOL` และ `BEHAVIOR_MANAGE_SCHOOL` ที่ `create_special` ตรวจเมื่อมี scoring |
| `school-behavior` | `BEHAVIOR_READ_ASSIGNED`, `BEHAVIOR_MANAGE_SCHOOL` |

`school-attendance` รับ `BEHAVIOR_MANAGE_SCHOOL` จาก generator โดยตรง จึงใช้ constant ข้ามโดเมนได้โดยไม่ต้องเพิ่ม dependency บน crate ของ Behavior หาก call site ต้องเรียก policy ที่ Behavior เป็นเจ้าของ ให้ใช้ provider หรือ adapter เดิมที่รับผิดชอบเรื่องนั้นและคงผลตัดสินสิทธิ์เดิม ต้นทุนของรูปแบบนี้คือการดูแลรายการ codes ต่อ consumer ควบคู่กับการตรวจ drift [Budget policy](../../../backend-school/crates/school-budget/src/policy.rs) · [Attendance handlers](../../../backend-school/crates/school-attendance/src/http/handlers.rs) · [Behavior policy](../../../backend-school/crates/school-behavior/src/policy.rs)

## วิธีวัดผลและเกณฑ์ตรวจ

- วัดผลการทดลอง `WILDCARD` เทียบกับ baseline แล้วเทียบ candidate ที่ย้ายครบทุก consumer กับ baseline อีกครั้ง ใช้เครื่อง, build context, toolchain, target, release flags, CPU quota, Cargo jobs และสภาพ cache เริ่มต้นที่เทียบกันได้ อิงคำสั่งและลำดับใน `build_release.sh` เก็บ Cargo JSON แยกสำหรับ `backend-school` และ `migrate_tenant_schema` พร้อมเวลา Cargo ของแต่ละ binary และเวลารวม [คู่มือ benchmark](../../TESTING.md#backend-school) · [Release builder](../../../backend-school/scripts/build_release.sh)
- ใช้ fixture การเปลี่ยน permission แบบเดียวกันในแต่ละคู่ ครอบคลุมการเปลี่ยน code ของ consumer เดียว การเปลี่ยน code ที่ใช้ข้ามโดเมน และ build หลัง generate โดยที่ input เดิม แต่ละรอบให้คืน checkout ทดลองสู่สภาพก่อนใช้ fixture แล้ว prime graph ก่อนใช้ fixture และ generate output เริ่มจับเวลาเมื่อ Cargo เริ่ม build ทำซ้ำอย่างน้อย 3 รอบต่อกรณีและสลับลำดับการวัด เก็บค่ากลางและช่วงของเวลา Cargo รวมถึงรายชื่อและจำนวนหน่วยที่ `fresh: false` เมื่อเปิด sccache ให้เก็บ hit/miss ต่อรอบด้วย
- สำหรับ contract เดียวกัน การทดลอง `WILDCARD` ต้องให้ Rust registry กลางตรงกับ baseline ทุกไบต์ เมื่อย้ายครบแล้วให้เก็บรายการ `ALL_PERMISSIONS` ไว้ด้วยลำดับ ชนิด และค่าเดิม และปิดการเข้าถึง `school_permissions::registry::codes` จากภายนอก crate คง `contracts/permissions.lock.json` และ TypeScript registry ทุกไบต์ตลอดทั้งสองช่วง [Generator tests](../../../scripts/tests/generate-permissions.test.mjs)
- ให้ generator ใช้เฉพาะชื่อและค่าของ codes ที่เลือกในการสร้าง output ของ consumer และใช้ write-if-bytes-change เดิม เช็กทั้งไบต์และเวลาเขียนของไฟล์ที่ไม่เกี่ยวเมื่อแก้ permission และยืนยันว่า code ข้ามโดเมนอัปเดตครบตามที่ใช้ ให้ `--check` ตรวจจับ output ที่ขาด ผิด หรือค้างจากรายการที่ถูกลบ เมื่อย้ายครบแล้วให้ static architecture ตรวจ imports และให้ local backend verification ตรวจ Cargo metadata ว่าเหลือ direct registry dependencies เฉพาะเป้าหมายที่อ่าน metadata [Architecture tests](../../../backend-school/tests/static_architecture.rs) · [Local verification](../../../scripts/pipeline.mjs)
- ยืนยันว่า wildcard, การอนุญาตและปฏิเสธ, exact/tree และการรวมหลาย scope ให้ผลเดิม ใช้ PostgreSQL fixture ของ repo รัน `db::migration_tests` ที่ตรวจการรักษา grant เดิม การ rollback เมื่อ insert ล้มเหลว และการ retry จากนั้นตรวจการเลือก management permission ของ Workflow [DB sync tests](../../../backend-school/src/db/migration_tests.rs) · [Permission testing](../../TESTING.md#permission-contract)

คำสั่งจากราก repo สำหรับตรวจงานเมื่อได้ข้อสรุป

```bash
npm --prefix frontend-school run generate:permissions
npm --prefix frontend-school run check:permissions
npm --prefix frontend-school run test:permissions
cargo check --manifest-path backend-school/Cargo.toml --workspace --all-targets --locked
cargo test --manifest-path backend-school/Cargo.toml --locked --test static_architecture
cargo test --manifest-path backend-school/Cargo.toml --locked -p school-authorization --lib
./scripts/test_backend_school.sh --package school-authorization --test resource_acceptance -- --test-threads=1
./scripts/test_backend_school.sh db::migration_tests -- --test-threads=1
./scripts/test_backend_school.sh permission_coverage -- --test-threads=1
./scripts/test_backend_school.sh --package school-workflow --lib -- --test-threads=1
```

ใช้ `node --test frontend-school/tests/static/documentation-policy.test.mjs` และ `git diff --check` เพื่อตรวจเอกสารและ diff [Verification matrix](../../../.rules#11-verification-matrix)

## เกณฑ์ตัดสินใจ

พิจารณาว่าเวลา Cargo ลดลงเหนือความแกว่งของตัวอย่างและคุ้มกับการดูแล generator หรือไม่ ตัดสินจากผลกราฟและผล Cargo ของ candidate ที่ย้ายครบ รวมถึงการตรวจว่า policy ให้ผลเดิม หากคุ้มให้รวมงานครั้งเดียวหลังย้ายครบทุก consumer โดยแบ่ง commit ใน draft ตามลำดับ dependency ได้ หากผลวัดของรูปแบบครบทั้ง workspace ยังไม่คุ้ม ให้ใช้รูปแบบ import จาก registry กลางตามเดิม
