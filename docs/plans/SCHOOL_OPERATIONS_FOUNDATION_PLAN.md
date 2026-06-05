# แผนวางฐานระบบสำหรับการใช้งานจริงในโรงเรียน

> เป้าหมาย: ทำให้ SchoolOrbit พัฒนาต่อได้ยั่งยืน และรองรับงานโรงเรียนจริงตามโครงสร้างการบริหาร เช่น กลุ่มสาระ, กลุ่มบริหาร, กิจกรรมเปิด/ปิดรับสมัคร, การส่งงาน, การอนุมัติ, การติดตามงาน และหลักฐานการทำงานตลอดปีการศึกษา

## หลักคิด

ระบบโรงเรียนไม่ควรเริ่มจาก feature แยกขาดกัน เช่น "กิจกรรม", "ส่งงาน", "เอกสาร", "อนุมัติ" คนละระบบ เพราะสุดท้ายทุกอย่างต้องตอบคำถามชุดเดียวกัน:

- ใครเป็นคนทำ
- อยู่กลุ่ม/ฝ่ายไหน
- มีสิทธิ์ระดับใด
- งานนี้เปิดเมื่อไร ปิดเมื่อไร
- ต้องส่งอะไร
- ใครตรวจ/อนุมัติ
- แจ้งเตือนใครบ้าง
- มีหลักฐานย้อนหลังหรือไม่

ดังนั้นฐานระบบควรเป็น **Org-driven Operations Platform**: ใช้โครงสร้างองค์กรของโรงเรียนเป็นแกน แล้วให้ permission, workflow, activity, submission, audit และ notification ทำงานร่วมกัน

## สถานะปัจจุบันโดยย่อ

ระบบมีฐานที่ดีแล้วหลายส่วน:

- `departments`, `department_members`, `department_permissions` รองรับโครงสร้างฝ่าย/กลุ่มสาระ
- `subject_groups` แยกจาก `departments` และเชื่อมผ่าน `departments.subject_group_id`
- permission resolver รองรับ role, department position, delegation และ cache
- มีกิจกรรม/ตารางสอน/หลักสูตร/รับสมัคร/ไฟล์/notification บางส่วนแล้ว
- มี `audit_logs` และ `utils/audit.rs` แต่ยังไม่ได้ใช้เป็นมาตรฐานใน mutation สำคัญ
- schema โตเร็วมาก มี migration จำนวนมาก จึงต้องมี policy ก่อนรื้อหรือเพิ่มตารางใหญ่

จุดที่ควรปรับก่อนเพิ่ม feature ใหญ่:

- permission foundation ถูกย้ายมาใช้ `ActorContext`/registry/static guard แล้ว แต่ยังต้องตรวจ route/menu/frontend guard ตาม feature ใหม่ทุกครั้ง
- handler หลายจุดยังมีรูปแบบ response และ request context ไม่สม่ำเสมอ ถึงแม้ permission flow หลักจะใช้ `load_actor_context` แล้ว
- API หลาย endpoint ยังสร้าง `Json(json!({...}))` แบบ ad-hoc และยังไม่ได้มี typed response contract ครบทุก domain
- mutation สำคัญยังไม่มี audit log สม่ำเสมอ
- workflow เปิด/ปิด/ส่ง/ตรวจ/อนุมัติ ยังไม่มีกลางระบบ
- notification ยังไม่ผูกกับ event/workflow กลาง

## Foundation Backlog ล่าสุด

> สถานะ ณ 2026-06-05: Permission foundation รอบ backend ถูกเก็บให้ใช้ `load_actor_context(...)` และ `actor.require_*` แล้ว, static architecture tests อยู่ฝั่ง backend-school แล้ว, และ `backend-school` ผ่าน `cargo clippy --all-targets -- -D warnings` โดยไม่ suppress lint

ลำดับนี้คือรายการ “ต้องแก้เป็นฐานก่อนเพิ่ม feature ใหญ่” ไม่ใช่ feature เสริม:

1. **Backend Handler / Request Context Foundation**
   - เก็บ module ที่ยังมี helper local เช่น `get_pool`, auth, permission, response mapping ที่ไม่สม่ำเสมอ
   - ใช้ `utils/request_context.rs` เป็น pattern กลางสำหรับ tenant, pool, actor และ request metadata
   - handler ใหม่ควรมี flow เดียว: tenant/actor context → permission → service → typed response

2. **API Contract Foundation**
   - ทุก JSON API ใช้ envelope เดียว `{ success, data, error?, message? }`
   - endpoint ที่ frontend ใช้จริงควรมี typed Rust response struct และ typed frontend API client
   - ลด ad-hoc `Json(json!({...}))` ใน endpoint สำคัญ เพื่อให้ CSR frontend ไม่ต้องรองรับ response แปลกหลายแบบ

3. **Audit / Logging Foundation**
   - mutation สำคัญต้องเขียน `audit_logs` อย่างสม่ำเสมอ
   - ใช้ `tracing` แทน `println!` / `eprintln!` ใน backend-school code path ปกติ
   - audit/log ต้องไม่บันทึก plaintext `national_id` หรือข้อมูลอ่อนไหว

4. **School Organization Foundation**
   - API/UI สำหรับ `departments`, `department_members`, position, subject group link และ delegation ต้องชัด
   - โครงสร้างองค์กรต้องเป็นฐานให้ permission, workflow, approval และงานกลุ่มสาระ/กลุ่มบริหาร

5. **Resilience / Ops Foundation**
   - เพิ่ม timeout/retry/backoff/circuit breaker ใน external clients ที่สำคัญ
   - health/observability ต้องสะท้อน dependency จริง ไม่ใช่แค่ service ยังเปิดอยู่
   - migration squash ให้ทำเป็น policy หลังยืนยัน tenant baseline แล้ว ไม่ใช่รื้อทันที

6. **Data Model / Migration Foundation**
   - ทำ schema map/ERD ของ domain หลักก่อนเพิ่มตารางใหญ่
   - ตั้ง JSONB policy และ typed DTO สำหรับ JSONB ที่ยังจำเป็น
   - เพิ่ม constraints/index ตาม query จริงและ migration policy ที่ตรวจสอบได้

## เป้าหมายสถาปัตยกรรม

```text
School / Tenant
  └─ Organization
      ├─ Departments / Subject Groups
      ├─ Members / Positions
      └─ Permission Templates / Delegations

Actor Context
  ├─ user_id
  ├─ subdomain
  ├─ effective permissions
  ├─ departments / positions
  └─ request metadata

Operations
  ├─ Activities / Events / Tasks
  ├─ Open-Close Windows
  ├─ Submissions / Attachments
  ├─ Reviews / Approvals
  ├─ Notifications / Reminders
  └─ Audit Logs
```

## Phase 1: Access Foundation

เป้าหมาย: ทุก feature ใช้สิทธิ์และ actor model เดียวกัน

สิ่งที่ต้องทำ:

- รวม permission check ให้ใช้ resolver กลางเท่านั้น
- เลิกใช้ legacy `UserPermissions::has_permission()` ใน module ใหม่ และค่อย migrate module เก่า
- เพิ่ม helper กลาง:
  - `require_permission`
  - `require_any_permission`
  - `require_all_permissions`
  - `load_actor_context`
- ห้ามเขียน permission string ตรงใน handler ให้ใช้ `permissions::registry::codes`
- เพิ่ม static test ตรวจว่า permission ที่ใช้ใน backend/frontend มีอยู่ใน registry
- ทำให้ permission cache invalidation ครอบคลุม role, department member, department permission และ delegation ทุก mutation

สถานะล่าสุด:

- มี `ActorContext`, `load_actor_context`, `require_permission`, `require_any_permission`, `require_all_permissions` เป็น helper กลางแล้ว
- มี static contract ตรวจว่า backend/frontend permission references ต้องอยู่ใน registry หรือเป็น module ที่ประกาศใน registry
- activity permissions ที่ frontend/backend ใช้จริงถูกเพิ่มเข้า permission registry แล้ว
- ลบ frontend static menu helper เก่าที่ไม่ถูกใช้งานและใช้ permission name ไม่ตรง registry แล้ว

ผลลัพธ์ที่ต้องได้:

- เพิ่ม feature ใหม่แล้วไม่ต้องคิด auth/permission ใหม่ทุกครั้ง
- ลด bug สิทธิ์หลุดหรือสิทธิ์ไม่ทำงานเพราะ string ผิด
- รองรับสิทธิ์ตามตำแหน่ง เช่น หัวหน้ากลุ่มสาระ, รองหัวหน้า, สมาชิก

## Phase 2: School Organization Foundation

เป้าหมาย: โครงสร้างโรงเรียนเป็นข้อมูลหลัก ไม่ใช่แค่ dropdown

สิ่งที่ต้องทำ:

- ทำหน้า/บริการจัดการ `departments` ให้ชัดว่าเป็น:
  - กลุ่มบริหาร
  - งาน/ฝ่าย
  - กลุ่มสาระ
- ทำหน้า/บริการจัดการสมาชิกหน่วยงาน:
  - เพิ่ม/ย้าย/สิ้นสุดสมาชิก
  - กำหนด position: `head`, `deputy_head`, `coordinator`, `member`
  - บันทึกช่วงวันที่เริ่ม/สิ้นสุด
- ทำ permission template ต่อ department + position
- ทำ delegation ให้ใช้งานจริง:
  - หัวหน้ามอบหมายสิทธิ์บางอย่างให้สมาชิก
  - กำหนดวันหมดอายุ
  - เพิกถอน
  - audit ทุกครั้ง
- แยกความหมายให้ชัด:
  - `subject_groups` = หมวดวิชา/กลุ่มสาระทางวิชาการ
  - `departments` = หน่วยงาน/กลุ่มคน/สิทธิ์

ผลลัพธ์ที่ต้องได้:

- โรงเรียนกำหนดโครงสร้างตามจริงได้
- ฟีเจอร์อนาคต เช่น หนังสือราชการ, งานกิจกรรม, งบประมาณ, ระบบลา สามารถ route ตามฝ่าย/ตำแหน่งได้

## Phase 3: Workflow / Task Foundation

เป้าหมาย: รองรับงานโรงเรียนที่มีเปิด-ปิด ส่งงาน ตรวจงาน อนุมัติ และติดตามสถานะ

ควรสร้าง module กลาง เช่น `operations` หรือ `workflows` สำหรับข้อมูลต่อไปนี้:

- `work_items`
  - งานหรือกิจกรรมที่ถูกประกาศ
  - มี owner เป็น user หรือ department
  - มี audience เป็น user, classroom, grade level, department หรือทั้งโรงเรียน
  - มี `opens_at`, `closes_at`, `due_at`
  - มีสถานะ `draft`, `published`, `open`, `closed`, `archived`
- `work_submissions`
  - ผู้ส่ง
  - เวลาส่ง
  - สถานะ `submitted`, `returned`, `approved`, `rejected`, `late`
  - แนบไฟล์ผ่าน file module
- `workflow_instances`
  - งานที่ต้องผ่านหลายขั้น เช่น หัวหน้ากลุ่มสาระ → กลุ่มบริหาร → ผู้อำนวยการ
- `workflow_steps`
  - ผู้รับผิดชอบแต่ละขั้น
  - required position หรือ permission
  - action: `review`, `approve`, `sign`, `acknowledge`
- `work_comments`
  - comment ภายในงาน
  - ใช้แทนการเก็บข้อความกระจัดกระจายในแต่ละ feature

หลักสำคัญ:

- งานเปิด/ปิดต้อง enforce ที่ backend
- frontend แค่แสดงสถานะ ไม่ใช่ตัวตัดสินสิทธิ์
- การเปลี่ยนสถานะทุกครั้งต้องมี audit log
- notification ควรถูกสร้างจาก workflow event ไม่ใช่ยิงกระจัดกระจายใน handler

ผลลัพธ์ที่ต้องได้:

- ใช้ฐานเดียวกันได้กับกิจกรรม, ส่งงาน, เอกสาร, แบบฟอร์ม, งานฝ่าย, งานกลุ่มสาระ
- ไม่ต้องสร้างระบบส่งงานใหม่ทุก feature

## Phase 4: Audit, Logging, and Compliance Foundation

เป้าหมาย: ใช้งานจริงแล้วตรวจสอบย้อนหลังได้ โดยไม่ log ข้อมูลอ่อนไหว

สิ่งที่ต้องทำ:

- ใช้ structured logging (`tracing`) ให้ทั่ว backend-school
- ลด `println!` / `eprintln!` ใน code path ปกติ
- ใช้ `audit_logs` กับ mutation สำคัญ:
  - login/logout ที่สำคัญ
  - role/permission/department change
  - staff/student create/update/delete
  - admission status changes
  - file upload/delete
  - school settings
  - workflow approve/reject/return
- ห้าม log plaintext `national_id` หรือข้อมูลอ่อนไหว
- เพิ่ม request metadata:
  - actor user id
  - request path/method
  - IP/user agent เท่าที่จำเป็น
  - entity type/id
- ทำ audit API สำหรับ admin ที่มีสิทธิ์เท่านั้น

ผลลัพธ์ที่ต้องได้:

- โรงเรียนตรวจสอบได้ว่าใครแก้อะไร เมื่อไร
- ลดความเสี่ยง PDPA และลดปัญหาเวลามี dispute

## Phase 5: Notification and Calendar Foundation

เป้าหมาย: งานโรงเรียนไม่ตกหล่น เพราะทุกงานมี deadline และผู้รับผิดชอบ

สิ่งที่ต้องทำ:

- สร้าง event/outbox pattern สำหรับ notification
- แยก notification event ออกจาก handler ธรรมดา
- รองรับ reminders:
  - ก่อนเปิด
  - ก่อนปิด
  - ก่อน deadline
  - เมื่อมีงานค้าง
- ผูก work item กับ calendar:
  - วันเปิด/ปิด
  - วันครบกำหนด
  - วันจัดกิจกรรม
- มี notification preference ในอนาคต:
  - in-app
  - push
  - email/LINE ในระยะถัดไป

ผลลัพธ์ที่ต้องได้:

- นักเรียน/ครู/ผู้ปกครองเห็นงานและ deadline จากที่เดียว
- ลดการแจ้งเตือนซ้ำซ้อนในแต่ละ module

## Phase 6: Data Model and Migration Foundation

เป้าหมาย: ลดความซับซ้อนโดยไม่รื้อระบบเสี่ยง

สิ่งที่ต้องทำ:

- ทำ schema map/ERD ของ domain หลัก:
  - users/staff/students/parents
  - departments/permissions
  - academic curriculum
  - timetable
  - activities
  - admission
  - files
  - operations/workflows
- ตั้ง policy ของ JSONB:
  - ใช้ JSONB เฉพาะข้อมูล dynamic จริง
  - ถ้าฟิลด์ถูก query/filter/report บ่อย ให้ย้ายเป็น column/table
  - ทุก JSONB สำคัญต้องมี schema comment และ typed API DTO
- เพิ่ม constraints/checks ให้ค่าที่เป็น enum สำคัญ
- เพิ่ม index ตาม query จริง ไม่ใช่เพิ่มแบบคาดเดา
- วาง migration squash strategy หลัง tenant ทุกตัว migrate ครบ baseline
- สร้าง read models/views เฉพาะจุดที่ query ซับซ้อน เช่น dashboard/report

ผลลัพธ์ที่ต้องได้:

- schema เข้าใจง่ายขึ้นโดยไม่ทำลายข้อมูลเดิม
- migration ใหม่ปลอดภัยและตรวจสอบได้
- query สำคัญเร็วขึ้นและดูแลได้

## Phase 7: API and Frontend Foundation

เป้าหมาย: frontend เป็น CSR ตามที่กำหนด และ contract ชัดพอให้พัฒนาต่อเร็ว

สิ่งที่ต้องทำ:

- ทุก API ใช้ envelope เดียว `{ success, data, error?, message? }`
- ทุก known endpoint ใช้ typed `apiClient<T>`
- แยก API module ตาม domain:
  - `auth`
  - `staff`
  - `students`
  - `academic`
  - `activities`
  - `operations`
  - `files`
  - `notifications`
- เพิ่ม static contract tests เมื่อเพิ่ม API หลัก
- frontend route ใน `(app)` ใช้ CSR ต่อไป
- UI สำหรับงานโรงเรียนควรมี pattern เดียว:
  - status badge
  - open/close/due time
  - assigned audience
  - submission state
  - reviewer/action history

ผลลัพธ์ที่ต้องได้:

- เพิ่มหน้าใหม่ได้โดยไม่ต้องเดา response shape
- UX งานโรงเรียนสม่ำเสมอ

## Phase 8: Testing, Sandbox, and Operations

เป้าหมาย: ทุก foundation ที่เพิ่มต้องตรวจได้จริง ไม่ใช่แค่ compile ผ่าน

สิ่งที่ต้องทำ:

- เพิ่ม sandbox seed ให้มี:
  - ผู้อำนวยการ
  - หัวหน้ากลุ่มบริหาร
  - หัวหน้ากลุ่มสาระ
  - ครูสมาชิก
  - นักเรียน
  - ผู้ปกครอง
  - กิจกรรมเปิด/ปิดตัวอย่าง
  - งานส่งตัวอย่าง
- เพิ่ม smoke tests:
  - login
  - permission by role/department/position
  - activity open/close enforcement
  - submission before/after deadline
  - audit log created
- เพิ่ม Playwright E2E เฉพาะ workflow หลัก:
  - ครูสร้างงาน
  - นักเรียนส่งงาน
  - ครูตรวจ
  - ระบบแจ้งเตือน/สถานะเปลี่ยน
- เพิ่ม health/observability:
  - DB health
  - notification worker health
  - storage health

ผลลัพธ์ที่ต้องได้:

- ก่อน deploy มีหลักฐานว่าระบบโรงเรียน flow หลักยังทำงาน
- ลด regression เมื่อเพิ่ม feature ใหม่

## ลำดับที่ควรทำจริง

| ลำดับ | งาน | เหตุผล |
|---|---|---|
| 1 | Backend Handler / Request Context Foundation | ลด code ซ้ำใน handler และทำให้ audit/API/workflow ใช้ข้อมูล actor/tenant ชุดเดียวกัน |
| 2 | API Contract Foundation | frontend CSR และ service ใหม่จะพัฒนาง่ายขึ้นเมื่อ response shape และ typed client คาดเดาได้ |
| 3 | Audit / Logging Foundation | ใช้งานจริงต้องตรวจย้อนหลังได้ และต้องไม่ log ข้อมูลอ่อนไหว |
| 4 | School Organization Foundation | department/position/subject group/delegation เป็นฐานของงานฝ่าย กลุ่มสาระ และ approval |
| 5 | Resilience / Ops Foundation | ลดความเสี่ยงจาก external services, deployment, health และ migration operations |
| 6 | Data Model / Migration Foundation | ลดความซับซ้อนระยะยาวโดยไม่รื้อ schema เสี่ยง |
| 7 | Workflow / Notification Foundation | เริ่มหลังฐาน actor/API/audit/org ชัด เพื่อไม่สร้างระบบส่งงาน/แจ้งเตือนซ้ำในแต่ละ feature |

## สิ่งที่ไม่ควรทำก่อน

- ไม่ควรรื้อ database ใหญ่ทันทีโดยไม่มี schema map และ migration strategy
- ไม่ควรสร้างระบบส่งงานเฉพาะกิจกรรมอย่างเดียว เพราะจะซ้ำกับเอกสาร/งานฝ่าย/แบบฟอร์มในอนาคต
- ไม่ควรเพิ่ม permission string ใหม่โดยไม่เข้ registry
- ไม่ควรให้ frontend ตัดสินสิทธิ์หรือ deadline เอง
- ไม่ควรใช้ audit log เป็น optional สำหรับ mutation สำคัญ

## Definition of Done ของฐานระบบ

งาน foundation ถือว่าเสร็จเมื่อ:

- มี API/service pattern กลาง ไม่ต้อง copy auth/subdomain/permission ใน handler ใหม่
- permission ทุกตัวที่ใช้จริงอยู่ใน registry และมี static test ตรวจ
- mutation สำคัญมี audit log
- workflow เปิด/ปิด/ส่ง/ตรวจ ถูก enforce ที่ backend
- notification เกิดจาก event/outbox ไม่กระจายใน handler
- มี sandbox seed สำหรับ role/department/activity/submission flow
- มี smoke/E2E ครอบคลุม flow โรงเรียนหลัก

## ข้อสรุป

ฐานที่ควรทำก่อนคือ **Permission + Actor Context + Audit + Workflow กลาง** ไม่ใช่การรื้อ schema ทันที

เมื่อฐานนี้นิ่งแล้ว ระบบจะรองรับงานโรงเรียนจริงได้ยืดหยุ่นกว่าเดิม เช่น กิจกรรมเปิด/ปิดรับสมัคร, การส่งงาน, การตรวจงาน, การอนุมัติเอกสาร, การมอบหมายงานตามฝ่าย/กลุ่มสาระ และการแจ้งเตือนตลอดปีการศึกษา โดยไม่ต้องสร้าง logic ซ้ำทุก feature
