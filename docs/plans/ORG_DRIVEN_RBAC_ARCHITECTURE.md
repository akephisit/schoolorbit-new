# การวิเคราะห์ Architecture: Org-driven RBAC

> วันที่วางแผน: 2026-03-24

---

## สรุปคำแนะนำ (TL;DR)

**ไม่ต้องทิ้งระบบ permission เดิม** — แต่ควร evolve ให้ "โครงสร้างองค์กรเป็นตัวกำหนด permission" แทนที่จะ assign permission ให้คนโดยตรง

Pattern นี้เรียกว่า **Org-driven RBAC** และรองรับทุกสิ่งที่ต้องการ รวมถึง document flow ในอนาคต

---

## ปัญหาของระบบปัจจุบัน

| ปัญหา | รายละเอียด |
|-------|-----------|
| Permission เท่ากันทุกคนในกลุ่ม | `department_permissions` ให้ permission เดียวกับทุกคน ไม่ว่าจะเป็น head หรือ member |
| ไม่มี delegation | หัวหน้าไม่สามารถมอบหมายสิทธิ์บางอย่างให้สมาชิกได้ |
| Assign permission ด้วยมือ | Admin ต้องกำหนดสิทธิ์ให้แต่ละคนเอง ไม่ automatic |
| ไม่รองรับ future flows | Document flow, budget approval ต้องการ org context ซึ่งระบบ permission ล้วนๆ ทำไม่ได้ |

---

## Infrastructure ที่มีอยู่แล้ว (ใช้ต่อได้เลย)

```
departments (กลุ่มสาระ / กลุ่มบริหาร)
  ├─ id, code, name, category (academic/administrative), org_type
  └─ parent_department_id (รองรับ hierarchy แล้ว)

department_members (สมาชิกในกลุ่ม)
  ├─ user_id, department_id
  └─ position: head | deputy_head | member | coordinator  ← มีอยู่แล้ว แต่ยังไม่ถูกใช้

department_permissions (bridge dept → permissions)
  └─ department_id, permission_id  ← ยังไม่รู้จัก position
```

**โครงสร้างที่ขาดไปมีแค่ 2 อย่าง:**
1. `department_permissions` ไม่รู้จัก `position` (head ได้เหมือน member)
2. ไม่มี delegation mechanism

---

## เปรียบเทียบทางเลือก

| | Org-driven RBAC | Pure Org (ไม่มี permission) | Claim-based (JWT) |
|--|--|--|--|
| Flexibility | สูง | ต่ำ | สูง |
| ความซับซ้อน admin | กลาง | ต่ำ | กลาง |
| Refactor ที่ต้องทำ | เล็กน้อย | ใหญ่มาก | ปานกลาง |
| รองรับ edge case | ใช่ | ไม่ | ใช่ |
| Document flow | ใช่ | ใช่ | ใช่ |
| Delegation | ใช่ | ยาก | ใช่ |

**Org-driven RBAC ดีที่สุด** เพราะ build on ของเดิม, flexible, migration ไม่กระทบ feature ที่มีอยู่

---

## Architecture ที่แนะนำ

### แนวคิดหลัก

```
ตำแหน่งในองค์กร  →  Permission Template  →  Effective Permissions
(position)           (position-aware)

User
 └─ department_members (dept + position)
     └─ department_permissions (กรองตาม position)
         └─ permissions ที่ได้จริง

หัวหน้าสามารถ:
 └─ permission_delegations → ให้สมาชิกมีสิทธิ์ชั่วคราว
```

### ตัวอย่างการทำงาน

| ตำแหน่ง | กลุ่ม | ได้รับ permission อัตโนมัติ |
|---------|------|--------------------------|
| head | SUBJ-MA | `academic_curriculum.manage.department` + `dept_work.approve.department` |
| member | SUBJ-MA | `academic_curriculum.manage.department` เท่านั้น |
| head | กลุ่มบริหารวิชาการ | `academic_curriculum.manage.all` + `academic_structure.manage.all` |
| member | กลุ่มบริหารวิชาการ | `academic_curriculum.read.all` |

---

## การเปลี่ยนแปลงที่ต้องทำ

### Step 1: เพิ่ม `position` ใน `department_permissions`

**File:** `backend-school/migrations/042_position_aware_dept_permissions.sql`

```sql
ALTER TABLE department_permissions
ADD COLUMN position TEXT DEFAULT NULL;
-- NULL = ใช้กับทุก position ใน department นั้น
-- 'head', 'deputy_head', 'member', 'coordinator' = เฉพาะ position นั้น
```

### Step 2: เพิ่มตาราง `permission_delegations`

**File:** `backend-school/migrations/043_permission_delegations.sql`

```sql
CREATE TABLE permission_delegations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_user_id UUID NOT NULL REFERENCES users(id),   -- หัวหน้าที่มอบหมาย
    to_user_id UUID NOT NULL REFERENCES users(id),     -- สมาชิกที่ได้รับ
    permission_id UUID NOT NULL REFERENCES permissions(id),
    department_id UUID REFERENCES departments(id),      -- context (กลุ่มไหน)
    reason TEXT,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,                            -- NULL = ไม่หมดอายุ
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Step 3: อัปเดต Permission Resolver (Rust)

**File:** `backend-school/src/permissions/mod.rs`

Query permission ปัจจุบัน:
```
Role permissions + Department permissions (ไม่สนใจ position)
```

เปลี่ยนเป็น:
```
Role permissions
+ Department permissions WHERE position = NULL OR position = user's position in that dept
+ Delegated permissions WHERE to_user_id = me AND NOT revoked AND NOT expired
```

### ไฟล์ที่ต้องแก้ไข

| ไฟล์ | การเปลี่ยนแปลง |
|------|---------------|
| `migrations/042_position_aware_dept_permissions.sql` | เพิ่ม `position` column ใน `department_permissions` |
| `migrations/043_permission_delegations.sql` | ตาราง `permission_delegations` |
| `src/permissions/mod.rs` | อัปเดต query รวม permissions ให้รู้จัก position + delegation |
| `src/permissions/registry.rs` | เพิ่ม permission สำหรับ delegation (`dept_work.delegate`) |
| `src/modules/staff/handlers/departments.rs` | API สำหรับหัวหน้ามอบหมาย/เพิกถอนสิทธิ์ |
| `frontend-school` | หน้าจัดการ department members + delegation UI |

---

## รองรับ Future Features อย่างไร

### Document Flow (ส่งเอกสาร)

ใช้ org structure เดิมเป็น routing ไม่ต้องสร้าง org model ใหม่:

```sql
-- migration อนาคต
CREATE TABLE document_flow_templates (
    id UUID PRIMARY KEY,
    name TEXT,        -- เช่น "ใบลา", "ขออนุมัติงบ"
    steps JSONB       -- [{department_id, required_position, action: "approve/review/sign"}]
);
```

ตัวอย่าง routing:
- **ใบลา** → หัวหน้ากลุ่มสาระ → หัวหน้ากลุ่มบริหารบุคคล → ผู้อำนวยการ
- **ขออนุมัติงบ** → หัวหน้ากลุ่มสาระ → กลุ่มบริหารงบประมาณ → ผู้อำนวยการ

---

## ลำดับการพัฒนาที่แนะนำ

```
Phase 1 (งานด่วน):
  ├─ migration 042: position column ใน department_permissions
  ├─ อัปเดต permission resolver
  └─ ใช้กับ curriculum subject management (ดู CURRICULUM_DEPT_SUBJECT_MANAGEMENT.md)

Phase 2:
  ├─ migration 043: permission_delegations table
  ├─ API: หัวหน้ามอบหมาย/ถอนสิทธิ์
  └─ UI: หน้าจัดการสมาชิก + delegation

Phase 3 (อนาคต):
  ├─ Document flow template + routing engine
  └─ Task/work assignment system
```

---

## ข้อสรุป

- **ไม่ต้อง refactor ใหญ่** — เพิ่มแค่ 2 อย่าง (position-awareness + delegation table)
- **Permission ยังเป็น mechanism** แต่ org structure เป็นตัวกำหนดว่าใครได้อะไร
- **รองรับ document flow** ได้ทันทีโดยใช้ org structure เดิม
- **Migration path ชัดเจน** ทำเป็น phase ได้ ไม่กระทบ feature ที่มีอยู่
