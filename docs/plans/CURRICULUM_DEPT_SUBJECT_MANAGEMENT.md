# แผน: ระบบจัดการวิชาระดับกลุ่มสาระ (Department-scoped Subject Management)

> วันที่วางแผน: 2026-03-24

## ที่มาและวัตถุประสงค์

ปัจจุบัน permissions ทุกตัวใน `academic_curriculum` เป็น `.all` scope ทั้งหมด ครูกลุ่มสาระจึงไม่สามารถจัดการ (เพิ่ม/แก้ไข/ลบ) วิชาในกลุ่มสาระของตัวเองได้โดยไม่ต้องให้สิทธิ์ admin ทั้งหมด

**เป้าหมาย:** เพิ่ม permission scope ใหม่ `academic_curriculum.manage.department` ให้ครูกลุ่มสาระจัดการวิชาได้เฉพาะกลุ่มสาระตัวเอง ส่วนครูหลักสูตร (ACAD-CUR) ยังคง `.all` scope ไว้เพื่อจัดการ study plan

---

## Workflow ที่ต้องการ (Direct — ไม่มี approval step)

```
ครูกลุ่มสาระ
  └─ เพิ่ม/แก้ไข/ลบ วิชาในกลุ่มสาระของตัวเอง (subject pool)

ครูหลักสูตร (ACAD-CUR)
  └─ เลือกวิชาจาก pool ทุกกลุ่มสาระ → ใส่ study plan แต่ละแผนการเรียน
```

---

## สถานะระบบปัจจุบัน

### Infrastructure ที่มีอยู่แล้ว (ไม่ต้องสร้างใหม่)

| ส่วน | รายละเอียด |
|------|------------|
| `departments` | มี `SUBJ-TH`, `SUBJ-MA`, `SUBJ-SC` ฯลฯ ครบ 8 กลุ่มสาระ |
| `department_members` | เชื่อมครูกับกลุ่มสาระได้อยู่แล้ว |
| `department_permissions` | รองรับให้ permissions ผ่าน department (migration 034) |
| `subject_groups` | code `TH`, `MA`, `SC` ตรงกับ `SUBJ-TH`, `SUBJ-MA` |
| `subjects` | มี `group_id` FK → `subject_groups`, มี `credit`, `academic_year_id` |
| `study_plans/versions/subjects` | ครบสำหรับ curriculum admin |

### ที่ขาดอยู่

- Permission `academic_curriculum.manage.department` ยังไม่มีใน registry
- Backend handler ยังไม่มี logic กรองตาม department scope
- Departments (SUBJ-*) ยังไม่ link กับ `subject_groups` อย่างชัดเจน (ต้องเพิ่ม FK)

---

## แผนการพัฒนา

### Step 1: Migration ใหม่

**File:** `backend-school/migrations/042_dept_subject_group_link.sql`

```sql
-- 1. เพิ่ม FK จาก departments ไปหา subject_groups
ALTER TABLE departments
ADD COLUMN subject_group_id UUID REFERENCES subject_groups(id);

-- 2. Link SUBJ-* departments กับ subject_groups ที่ตรงกัน
UPDATE departments d SET subject_group_id = sg.id
FROM subject_groups sg
WHERE d.code = 'SUBJ-' || sg.code;

-- 3. เพิ่ม permission ใหม่
INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
  'academic_curriculum.manage.department',
  'จัดการวิชากลุ่มสาระตัวเอง',
  'academic_curriculum', 'manage', 'department',
  'เพิ่ม/แก้ไข/ลบ รายวิชาเฉพาะกลุ่มสาระที่ตัวเองสังกัด'
);

-- 4. Assign permission ให้ทุก SUBJ-* department
INSERT INTO department_permissions (department_id, permission_id)
SELECT d.id, p.id
FROM departments d
CROSS JOIN permissions p
WHERE d.code LIKE 'SUBJ-%'
  AND p.code = 'academic_curriculum.manage.department';
```

---

### Step 2: เพิ่ม Permission Constants (Rust)

**File:** `backend-school/src/permissions/registry.rs`

```rust
// เพิ่มใน pub mod codes
pub const ACADEMIC_CURRICULUM_MANAGE_DEPT: &str = "academic_curriculum.manage.department";
pub const ACADEMIC_CURRICULUM_READ_DEPT: &str = "academic_curriculum.read.department";
```

เพิ่มใน `ALL_PERMISSIONS` array ทั้งสองตัวพร้อม metadata ภาษาไทย

---

### Step 3: Backend Handler Logic (Rust)

**File:** `backend-school/src/modules/academic/handlers/subjects.rs`

#### Helper Function ใหม่

```rust
async fn get_user_subject_group_id(user_id: Uuid, pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"
        SELECT sg.id FROM subject_groups sg
        JOIN departments d ON d.subject_group_id = sg.id
        JOIN department_members dm ON dm.department_id = d.id
        WHERE dm.user_id = $1 AND dm.ended_at IS NULL
        LIMIT 1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}
```

#### แก้ไข Handlers

| Handler | `.all` scope | `.manage.department` scope |
|---------|-------------|--------------------------|
| `list_subject_groups` | ดูได้ทุกกลุ่ม | ดูได้ทุกกลุ่ม (read-only ไม่ sensitive) |
| `list_subjects` | ดูได้ทุกวิชา | filter `WHERE group_id = teacher_group_id` |
| `create_subject` | สร้างได้ทุกกลุ่ม | ตรวจ `body.group_id == teacher_group_id` |
| `update_subject` | แก้ไขได้ทุกตัว | ตรวจ `subject.group_id == teacher_group_id` |
| `delete_subject` | ลบได้ทุกตัว | ตรวจ `subject.group_id == teacher_group_id` |

---

### Step 4: Frontend — Subjects Page

**File:** `frontend-school/src/routes/(app)/staff/academic/subjects/+page.svelte`

```typescript
const canManageAll = $can('academic_curriculum.create.all');
const canManageDept = $can('academic_curriculum.manage.department');
const isDeptScope = canManageDept && !canManageAll;
```

เมื่อ `isDeptScope = true`:
- API จะ return เฉพาะวิชากลุ่มสาระของครูคนนั้น (filter ที่ backend แล้ว)
- Dropdown กลุ่มสาระใน filter bar → `disabled`
- Form เพิ่ม/แก้ไขวิชา: field กลุ่มสาระ → `disabled` (pre-fill อัตโนมัติ)
- ซ่อน Bulk Copy feature (admin only)

---

### Step 5: Menu Access

ตรวจสอบ `required_permission` ของ menu item `/staff/academic/subjects` ใน database:
- ถ้าเป็น `academic_curriculum.read.all` → เปลี่ยนเป็น `academic_curriculum` (module wildcard)
- ทำให้ทั้ง `.read.all` และ `.manage.department` ผ่านได้

---

## ไฟล์ที่ต้องแก้ไข

| ไฟล์ | การเปลี่ยนแปลง |
|------|---------------|
| `backend-school/migrations/042_dept_subject_group_link.sql` | **สร้างใหม่** |
| `backend-school/src/permissions/registry.rs` | เพิ่ม 2 permission constants |
| `backend-school/src/modules/academic/handlers/subjects.rs` | เพิ่ม helper + dual-scope logic |
| `frontend-school/src/routes/(app)/staff/academic/subjects/+page.svelte` | dept-scope UI |

---

## การทดสอบ

1. สร้างครูทดสอบ → ผูกเข้า `department_members` กับ SUBJ-MA
2. Login → ตรวจว่าเห็นเมนู "รายวิชา"
3. หน้า subjects → แสดงเฉพาะวิชาคณิตศาสตร์ / dropdown กลุ่มสาระ disabled
4. เพิ่มวิชาใหม่กลุ่ม MA → สำเร็จ
5. ส่ง `group_id` กลุ่ม TH → ได้รับ 403 Forbidden
6. Login เป็นครูหลักสูตร (ACAD-CUR) → เห็นทุกกลุ่ม, จัดการได้ทุกกลุ่ม
7. Study plan management → ยังต้อง `.all` permission เท่าเดิม
