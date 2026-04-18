# Design: Team Teaching + Activity Instructor Hiding

**Date:** 2026-04-11

## Context

ระบบตารางสอนปัจจุบันมี 2 ข้อจำกัดที่ต้องแก้พร้อมกัน (ยังไม่มีข้อมูลจริง — แก้ตั้งแต่โครงสร้างได้):

1. **Synchronized activity (ลูกเสือ/ชุมนุม)** — entry ผูกกับห้องเรียน ครูเห็นผ่าน `activity_slot_instructors` JOIN
   - ครูที่ไม่ว่างคาบนั้น (เช่นสอน ม.ปลายตอนที่ลูกเสือเรียนเฉพาะ ม.ต้น) ไม่สามารถลบออกจากตารางตัวเองได้โดยไม่กระทบห้องเรียน
   - ต้องการ: ซ่อนรายครู ครูยังเป็น slot instructor + sidebar ยังแสดงว่ายังไม่ได้จัด

2. **วิชาปกติ** — รองรับครูสอนแค่ 1 คนต่อ `classroom_courses` (ผ่าน `primary_instructor_id`)
   - ต้องการ: team teaching เช่น STEM มีครูวิทย์ + ครูคณิต สอนด้วยกัน

**เป้าหมาย:** หนึ่ง pattern สถาปัตยกรรมที่แก้ทั้ง 2 ปัญหา โดยไม่กระทบ query/UI เดิมมากเกินไป

## Approach: Junction Table Pattern

ใช้ 2 junction tables:

- `timetable_entry_instructors` — ครูที่ schedule จริงใน entry (ต่อ entry × ครู)
- `classroom_course_instructors` — ครูของวิชา (source สำหรับ team teaching)

source tables เดิม (`activity_slot_instructors`, `activity_slot_classroom_assignments`, `classroom_course_instructors`) ยังเป็น "ควรเป็นครู" — ตอนสร้าง entry จะคัดลอกเข้า junction; junction แก้รายคนได้โดยไม่กระทบ source

## Database Schema

### Migration 076: New Tables

```sql
CREATE TABLE timetable_entry_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_id UUID NOT NULL REFERENCES academic_timetable_entries(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entry_id, instructor_id)
);
CREATE INDEX idx_tei_entry ON timetable_entry_instructors(entry_id);
CREATE INDEX idx_tei_instructor ON timetable_entry_instructors(instructor_id);

CREATE TABLE classroom_course_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    classroom_course_id UUID NOT NULL REFERENCES classroom_courses(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (classroom_course_id, instructor_id)
);
CREATE INDEX idx_cci_course ON classroom_course_instructors(classroom_course_id);
```

### Migration 077: Populate From Existing Data

```sql
-- classroom_courses.primary_instructor_id → classroom_course_instructors
INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
SELECT id, primary_instructor_id, 'primary'
FROM classroom_courses WHERE primary_instructor_id IS NOT NULL;

-- Regular course entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, cc.primary_instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN classroom_courses cc ON te.classroom_course_id = cc.id
WHERE cc.primary_instructor_id IS NOT NULL;

-- Synchronized activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asi.user_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slots asl ON te.activity_slot_id = asl.id
JOIN activity_slot_instructors asi ON asi.slot_id = asl.id
WHERE asl.scheduling_mode = 'synchronized';

-- Independent activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asca.instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slot_classroom_assignments asca
  ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id;
```

### Tables ยังเก็บไว้

- `classroom_courses.primary_instructor_id` — denormalized "ครูหลัก" (sync กับ role='primary'); ใช้ในการแสดงผลที่ต้องการแค่ครูเดียว
- `activity_slot_instructors` — source "ใครควรสอน slot นี้ (synchronized)"
- `activity_slot_classroom_assignments` — source "ใครสอน slot × ห้อง (independent)"

## Backend API

### Entry Creation — Populate Junction

ตอนสร้าง entry (ทั้ง single drag และ batch):

```rust
// หลัง INSERT entry สำเร็จ
match entry.source {
    ClassroomCourse(cc_id) => {
        // Copy from classroom_course_instructors
        sqlx::query(r#"
            INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
            SELECT $1, instructor_id, role FROM classroom_course_instructors
            WHERE classroom_course_id = $2
        "#).bind(entry_id).bind(cc_id).execute(...).await?;
    }
    SynchronizedActivity(slot_id) => {
        sqlx::query(r#"
            INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
            SELECT $1, user_id, 'primary' FROM activity_slot_instructors
            WHERE slot_id = $2
        "#).bind(entry_id).bind(slot_id).execute(...).await?;
    }
    IndependentActivity(slot_id, classroom_id) => {
        sqlx::query(r#"
            INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
            SELECT $1, instructor_id, 'primary' FROM activity_slot_classroom_assignments
            WHERE slot_id = $2 AND classroom_id = $3
        "#).bind(entry_id).bind(slot_id).bind(classroom_id).execute(...).await?;
    }
}
```

### Timetable Query

```sql
SELECT te.*,
  ARRAY_AGG(DISTINCT u.first_name || ' ' || u.last_name)
    FILTER (WHERE u.id IS NOT NULL) AS instructor_names,
  ...
FROM academic_timetable_entries te
LEFT JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
LEFT JOIN users u ON u.id = tei.instructor_id
...
GROUP BY te.id
```

INSTRUCTOR view filter:
```sql
WHERE EXISTS (
  SELECT 1 FROM timetable_entry_instructors
  WHERE entry_id = te.id AND instructor_id = $X
)
```

Model: `TimetableEntry.instructor_name: Option<String>` → `instructor_names: Vec<String>`

### Conflict Check

Single query ผ่าน junction:
```sql
SELECT 1 FROM academic_timetable_entries te
JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
WHERE tei.instructor_id = ANY($instructor_ids)
  AND te.day_of_week = $day AND te.period_id = ANY($periods)
```

แทน query 3 ตารางแยก (cc.primary_instructor_id / slot_instructors / slot_classroom_assignments)

### New Endpoints

```
POST   /api/academic/timetable/{id}/instructors              — เพิ่มครูเข้า entry
DELETE /api/academic/timetable/{id}/instructors/{uid}        — ลบครูออกจาก entry
POST   /api/academic/timetable/slots/{slot_id}/instructors/{uid}/restore
       — เพิ่มครูกลับเข้าทุก entry ของ slot (batch — สำหรับ "แสดงในตาราง")

GET    /api/academic/planning/courses/{id}/instructors       — list team
POST   /api/academic/planning/courses/{id}/instructors       — เพิ่ม team member
DELETE /api/academic/planning/courses/{id}/instructors/{uid} — ลบ team member
PUT    /api/academic/planning/courses/{id}/instructors/{uid} — update role
```

### Sync primary_instructor_id

เมื่อ add/remove/update role ใน `classroom_course_instructors`:
- ถ้า role='primary' → update `classroom_courses.primary_instructor_id`
- ถ้าลบคน primary → promote secondary คนที่ `created_at` เก่าสุดเป็น primary; ถ้าไม่มี secondary → set `primary_instructor_id = NULL`
- ถ้ามี primary อยู่แล้ว เพิ่มครูใหม่เป็น role='primary' → ต้อง demote คนเดิมเป็น secondary ใน transaction เดียว (constraint: ครู primary ไม่เกิน 1 คนต่อ course)

## Frontend UI

### 1. Course Planning (`planning/+page.svelte`)

- Card แสดงครูทั้งทีม: `ครูหลัก: ก.` + `ครูร่วม: ข. ค.`
- Dialog เพิ่มครู: dropdown เลือกครู + radio role
- ปุ่ม "เปลี่ยนเป็นครูหลัก" (swap role)

### 2. Timetable Grid (`timetable/+page.svelte`)

- Entry card field: `instructor_names: string[]` → join(', ') บนการ์ด
- เว้นที่สำหรับ 2-3 คนให้ดูไม่แน่น

### 3. Delete จาก INSTRUCTOR View

พฤติกรรมใหม่ตามประเภท:

| ประเภท | Action | Dialog |
|--------|--------|--------|
| วิชาปกติ (ครูเดียว) | ลบจาก junction → ถ้า junction ว่าง → ลบ entry | ยืนยันธรรมดา |
| วิชา team | ลบแค่ครูออกจาก junction | ยืนยัน "ลบ<ครู>ออกจากวิชานี้?" |
| Activity synchronized | ลบครูจาก junction ของ **ทุก entry** ของ slot นี้ | "ลบ<ครู>ออกจากกิจกรรมนี้?" |
| Activity independent | ลบ entry เลย (ครู 1 คน = 1 ห้อง) | ยืนยันธรรมดา |

### 4. Sidebar: Restore Hidden Activities (INSTRUCTOR view)

synchronized activity ที่ยังไม่ครบ `periods_per_week` แสดงใน sidebar:

```
┌──────────────────────────────────┐
│ 🟡 ชุมนุม ม.ต้น  (0/1 คาบ)        │
│ [แสดงในตาราง]                     │
└──────────────────────────────────┘
```

ปุ่มเรียก `/slots/{slot_id}/instructors/{my_id}/restore` → เพิ่มครูกลับทุก entry

### 5. Delete จาก CLASSROOM View (ไม่เปลี่ยน)

- วิชา/ทีม: ลบ entry → CASCADE (junction หายด้วย)
- Activity synchronized: dialog "เฉพาะห้อง/ทุกห้อง" (เดิม)
- Activity independent: ลบ entry (เดิม)

### 6. Conflict Highlight (ตอนลาก)

- Regular course/team: query junction หาคาบที่ครูคนใดคนหนึ่งไม่ว่าง → highlight
- Team teaching: ใช้ union ของครูทุกคน

## Delete Behavior Summary

| View | ประเภท | ลบ junction? | ลบ entry? | กระทบห้องอื่น? |
|------|--------|-------------|-----------|----------------|
| CLASSROOM | ทุกประเภท (1 entry) | CASCADE | ✅ | ❌ |
| CLASSROOM | Sync (batch) | CASCADE | ✅ ทุกห้อง | ✅ (user ยืนยัน) |
| INSTRUCTOR | วิชา ครูเดียว | ✅ | auto ถ้า junction ว่าง | ❌ |
| INSTRUCTOR | วิชา team | ✅ แค่ครูคนเลือก | ❌ | ❌ |
| INSTRUCTOR | Sync | ✅ ทุก entry ของ slot | ❌ | ❌ |
| INSTRUCTOR | Independent | ✅ + ลบ entry | ✅ | ❌ |

## Critical Files

| File | Changes |
|------|---------|
| `backend-school/migrations/076_team_teaching_junction.sql` | **NEW** junction tables |
| `backend-school/migrations/077_populate_junction.sql` | **NEW** data migration |
| `backend-school/src/modules/academic/models/timetable.rs` | `instructor_names: Vec<String>` |
| `backend-school/src/modules/academic/models/course_planning.rs` | เพิ่ม `CourseInstructor` model |
| `backend-school/src/modules/academic/handlers/timetable.rs` | Populate junction + JOIN query + new endpoints + conflict unified |
| `backend-school/src/modules/academic/handlers/course_planning.rs` | CRUD course instructors + sync primary |
| `backend-school/src/modules/academic/mod.rs` | Routes ใหม่ |
| `frontend-school/src/lib/api/timetable.ts` | Types + endpoints |
| `frontend-school/src/lib/api/academic.ts` | Course instructor API |
| `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte` | Team teaching UI |
| `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte` | หลายครู + delete per-instructor + restore |

## Verification

1. **Migrations**:
   - Run 076 + 077 → junction มีข้อมูลตรงกับ entries เดิม
   - Check ไม่มี orphan (entry ไม่มีครู / ครูไม่มี entry)

2. **Team teaching**:
   - Course planning: สร้างวิชา STEM → เพิ่ม ครูวิทย์ (primary) + ครูคณิต (secondary)
   - ลาก STEM ลง timetable → entry มีครู 2 คนใน junction
   - CLASSROOM view แสดง "ครูวิทย์, ครูคณิต" บน card
   - INSTRUCTOR view ครูวิทย์ และ ครูคณิต ทั้งคู่เห็น STEM

3. **Synchronized hide**:
   - Batch add ลูกเสือ → ครู ก/ข/ค ใน junction ของทุก entry
   - INSTRUCTOR view ครู ก → กดลบ → หายจาก grid
   - ตารางห้อง ม.1/1-ม.3/3 ไม่กระทบ (ครู ข/ค ยังอยู่)
   - ครู ก sidebar ยังเห็นลูกเสือ (ยังเป็น slot instructor) — 0/1 คาบ
   - กด "แสดงในตาราง" → ครู ก กลับเข้า junction ทุก entry

4. **Conflict**:
   - ลาก STEM ลงคาบที่ครูคณิตสอนวิชาอื่น → เตือน
   - Batch ลูกเสือ → ครูใน slot มีคาบอื่น → เตือน

5. **Delete CLASSROOM**:
   - ลบ entry วิชาเดียว → junction หาย (CASCADE)
   - ลบ entry batch sync → ทุกห้อง + ทุก junction หาย
   - ลบ slot → entries + junction หาย

6. **Primary sync**:
   - ลบ primary instructor → secondary คนแรก promote เป็น primary (หรือ null)
   - เปลี่ยน secondary → primary → คนเดิม demote

## Out of Scope

- Bulk team teaching UI (กำหนดทีมหลายวิชาพร้อมกัน)
- Role permission ตาม primary/secondary (เช่น primary แก้ไขเกรดได้ secondary อ่านอย่างเดียว)
- Team teaching สำหรับ activity groups (กลุ่มภายใน slot)
- Historical tracking ว่าใครสอนจริงในแต่ละคาบ (log)
