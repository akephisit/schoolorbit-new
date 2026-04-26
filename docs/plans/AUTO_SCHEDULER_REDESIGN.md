# Auto-Scheduler Redesign — แผนปรับปรุงการจัดตารางสอนอัตโนมัติ

> สถานะ: ✅ Phase A-G ทั้งหมด implemented
> วันที่: 2026-04-26 (เริ่ม) → 2026-04-26 (จบ Phase A-G)
>
> Migration ใหม่: 109-113
> หน้าใหม่: `/staff/academic/scheduling-config`,
>          `/staff/academic/timetable-templates`

---

## 1. ภาพรวม

ระบบจัดตารางสอนอัตโนมัติปัจจุบันทำงานเป็น single-shot — กดปุ่มเดียวจัดทุกอย่าง
ไม่รู้จัก entries ที่มีอยู่แล้ว ไม่รู้จัก priority ของครู ไม่รู้รูปแบบการจัดคาบ

แผนนี้ปรับเป็น **4-phase flow** — แยก fixed slots (manual) ออกจาก variable slots
(scheduler) เพื่อให้ search space เล็กลงและคุณภาพดีขึ้น

```
Phase 1: คนวาง                    Phase 2: คนตั้งค่า
┌──────────────┐                  ┌──────────────────┐
│ Batch fixed  │                  │ Constraints      │
│ • พัก โฮมรูม  │                  │ • Priority ครู   │
│ • sync รวม   │                  │ • ครูไม่ว่าง...    │
│ • TEXT ครู ก. │                  │ • วิชา 1+1+1     │
└──────┬───────┘                  └─────────┬────────┘
       │                                    │
       ▼                                    ▼
┌─────────────────────────────────────────────────┐
│           Phase 3: AUTO-SCHEDULER                │
│  • อ่าน entries เดิม → mark "occupied"           │
│  • รับ priority + unavailable + subject pattern │
│  • จัดวิชา + independent activity ลงช่องที่เหลือ  │
│  • respect ทุก constraint                        │
└────────────────────┬────────────────────────────┘
                     │
                     ▼
            ┌────────────────┐
            │ Phase 4: ตรวจ  │
            │ + ลากปรับเอง   │
            └────────────────┘
```

---

## 2. สถานะระบบปัจจุบัน

### 2.1 Auto-scheduler ที่มี

ไฟล์: `backend-school/src/modules/academic/handlers/scheduling.rs::auto_schedule_timetable`

```rust
// Pseudo-code ของ flow ปัจจุบัน
let available_slots = loader.load_available_slots(semester_id);  // ทุก day×period
let locked_slots    = loader.load_locked_slots(semester_id, classroom_ids);
                                                  // ↑ จาก timetable_locked_slots เท่านั้น
                                                  //   ไม่อ่าน academic_timetable_entries

if force_overwrite {
    DELETE FROM academic_timetable_entries WHERE classroom_course_id IN ...
    // ลบเฉพาะ COURSE — ACTIVITY ไม่โดนลบ แต่ก็ไม่ได้นับเป็น constraint
}

// run scheduler → insert COURSE entries
```

**ปัญหา:**
- ไม่อ่าน entries เดิม → จัดวิชาทับ ACTIVITY ที่ user batch ไว้ก่อนได้
- ไม่รับ priority ครู
- ไม่มี consecutive pattern (1+1+1, 2+1, 3 ติด)
- ไม่ตรวจ "ห้ามวันเดียวกันรหัสซ้ำ"
- ไม่จัด activity independent เลย

### 2.2 โครงสร้างที่มีอยู่ + ที่ต้องเพิ่ม

| Table / Column | สถานะ |
|----------------|-------|
| `instructor_preferences.hard_unavailable_slots` (jsonb) | ✅ มี |
| `instructor_preferences.preferred_slots` (jsonb) | ✅ มี |
| `instructor_preferences.priority` (int) | ❌ ต้องเพิ่ม |
| `subjects.periods_per_week` | ✅ มี (default — classroom_course override ได้) |
| `classroom_courses.consecutive_pattern` (jsonb) | ❌ ต้องเพิ่ม |
| `classroom_courses.same_day_unique` (bool) | ❌ ต้องเพิ่ม |
| `classroom_courses.hard_unavailable_slots` (jsonb) | ❌ ต้องเพิ่ม |
| `school_settings` (key-value) | ❌ ต้องเพิ่ม (global config) |
| `classroom_course_preferred_rooms` (table) | ❌ ต้องเพิ่ม |
| `instructor_room_assignments` | ✅ มี (ใช้เป็น fallback) |
| `timetable_locked_slots` | ✅ มี (ใช้ pin เพิ่มเติมได้) |
| `timetable_templates` + `template_entries` | ❌ ต้องเพิ่ม |

---

## 3. UI Design

### 3.1 หน้าใหม่ — `/staff/academic/scheduling-config`

```
┌─────────────────────────────────────────────────────────┐
│  ตั้งค่าก่อนจัดอัตโนมัติ                                   │
│  ─────────────────────────────────────────────────       │
│  💡 ลำดับครู = "ใครได้คาบดี ๆ ก่อน" (จัดให้คนสำคัญก่อน)    │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ⋮⋮  1. ครู ก. (รองผู้อำนวยการ)        ▼  3 ข้อจำกัด     │
│  ┌───────────────────────────────────────────────┐    │
│  │ คาบไม่ว่าง                                       │    │
│  │   จันทร์ ▢1 ▢2 ☑3 ☑4 ▢5 ▢6 ▢7  ← grid click    │    │
│  │   อังคาร ☑1 ▢2 ▢3 ▢4 ▢5 ▢6 ▢7                   │    │
│  │   ...                                            │    │
│  │                                                   │    │
│  │ วิชาที่สอน — รูปแบบการจัด                          │    │
│  │   ┌─────────────────────────────────────┐      │    │
│  │   │ ค23101 คณิต ม.3/1 (3 คาบ/สัปดาห์)    │      │    │
│  │   │   ◉ 1+1+1 (แยกวัน)                    │      │    │
│  │   │   ○ 2+1   ○ 1+2   ○ 3 ติด              │      │    │
│  │   │   ☑ ห้ามวันเดียวกันรหัสซ้ำ (default)   │      │    │
│  │   └─────────────────────────────────────┘      │    │
│  │   ...                                            │    │
│  └───────────────────────────────────────────────┘    │
│                                                          │
│  ⋮⋮  2. ครู ข. (หัวหน้ากลุ่มสาระ)       ▶  ไม่มีข้อจำกัด  │
│  ⋮⋮  3. ครู ค.                          ▶  1 ข้อจำกัด    │
│                                                          │
│              [ บันทึกและจัดอัตโนมัติ ]                   │
└─────────────────────────────────────────────────────────┘
```

### 3.2 Drag-and-drop priority

ใช้ `svelte-dnd-action` (ติดตั้งแล้วในโปรเจกต์)
- ลำดับ = `priority` (1 = สำคัญสุด)
- บันทึก order → backend update `instructor_preferences.priority`

### 3.3 Per-row expand → constraint editor

**ตัดสินใจ:** Config ของ pattern/rooms/unavailable เก็บที่ `classroom_courses`
(ระดับ subject × classroom) **ไม่ใช่ subject** — เพราะครู primary ต่างห้องอาจ
อยากตั้งค่าต่างกัน

- คลิก ▼ บน row ครู → ขยายแสดง:
  1. **Grid 7×N** — วัน × คาบ → คลิกเพื่อ toggle "ไม่ว่าง" (teacher-level)
     เก็บใน `instructor_preferences.hard_unavailable_slots`
  2. **ห้องประจำของครู** — multi-select rooms (teacher-level, default fallback)
     เก็บใน `instructor_room_assignments`
  3. **List classroom_courses ที่ครูเป็น primary**
     filter: `classroom_course_instructors.role = 'primary'`
     แต่ละ row = 1 (subject × classroom) → config แยกต่อห้อง
     ```
     • ค23101 คณิต ม.3/1 ▼
         ├─ pattern (radio)
         ├─ same_day_unique (checkbox, default ✓)
         ├─ max_consecutive (number)
         ├─ ห้อง — multi-select + drag rank + checkbox is_required
         └─ Grid 7×N — คาบที่ห้ามจัดวิชานี้ในห้องนี้
              - inherited จากครู ก. → pre-checked readonly + 🔒
              - admin ติ๊กเพิ่ม → editable
     • ค23101 คณิต ม.3/3 ▼   ← row แยก ตั้งค่าได้ต่างจาก ม.3/1
     ```

---

## 4. Consecutive Pattern (รูปแบบการจัดคาบ)

```
สอน 3 คาบ/สัปดาห์ → 4 รูปแบบหลัก
───────────────────────────────────
[1+1+1]  | จ. ┃ ค │   │   │   │   ┃
แยกทุกวัน | อ. ┃   │   │ ค │   │   ┃
         | พ. ┃   │ ค │   │   │   ┃

[2+1]    | จ. ┃ ค │ ค │   │   │   ┃   ← 2 ติด
         | พฤ.┃   │   │ ค │   │   ┃   ← 1 แยกวัน

[1+2]    | จ. ┃ ค │   │   │   │   ┃
         | พฤ.┃   │   │ ค │ ค │   ┃

[3 ติด]  | จ. ┃ ค │ ค │ ค │   │   ┃   ← 3 ติดวันเดียว
```

**DB schema:**
```sql
ALTER TABLE classroom_courses
    ADD COLUMN consecutive_pattern jsonb DEFAULT NULL;
-- nullable: ถ้า NULL → fallback เป็น [1] * periods_per_week
-- เก็บ array เช่น [1,1,1], [2,1], [3] — ยืดหยุ่นรองรับทุกแบบ
-- 5 คาบ → [2,2,1], [3,1,1], [1,1,1,1,1], ...
```

**Validation:** `sum(pattern) == subjects.periods_per_week` (ของ subject ที่ผูก)

---

## 4.5 Room Assignment (ห้องเรียนของแต่ละวิชา)

### Hierarchy การเลือกห้อง (จากสำคัญสุด → ต่ำสุด)

```
1. classroom_course preferred rooms (multi) ← ระดับ (subject × classroom)
   เช่น ฟิสิกส์ ม.3/1 → [Lab1, Lab2]
        ฟิสิกส์ ม.3/2 → [Lab2, Lab3]   ← ตั้งต่างห้องได้
   ────────────────────────────────────────
2. Instructor preferred rooms (multi) ← ตั้งระดับครู (default fallback)
   เช่น ครูต้น → [ห้อง ม.3/1] (ห้องประจำชั้น)
   ────────────────────────────────────────
3. (no preference) → ใช้ห้องไหนก็ได้ที่ว่าง / หรือไม่กำหนดห้อง
```

### Use cases

| สถานการณ์ | classroom_course rooms | Teacher rooms | Scheduler ทำ |
|-----------|----------------------|---------------|--------------|
| คณิต ม.3/1 ประจำชั้น | (ไม่ระบุ) | [ม.3/1] | ใช้ห้อง ม.3/1 (fallback ครู) |
| ฟิสิกส์ ม.3/1 | [Lab1, Lab2] | [ม.3/1] | Lab1 ก่อน → ถ้าเต็ม ลอง Lab2 |
| พละ ม.3/1 | [สนามฟุตบอล, สนามบาส] | (ไม่ระบุ) | สนามฟุตบอลก่อน |
| HOMEROOM ม.3/1 | (ไม่ระบุ) | (ไม่ระบุ) | ใช้ห้อง classroom (= ม.3/1) |
| คณิตเสริม ม.3/1 | [ห้องคอม] | [ม.3/1] | ห้องคอมก่อน (cc ชนะ) |

### DB schema

```sql
-- 1. ห้องที่ classroom_course นี้ใช้สอนได้ (multi, ranked)
CREATE TABLE classroom_course_preferred_rooms (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    classroom_course_id uuid NOT NULL
        REFERENCES classroom_courses(id) ON DELETE CASCADE,
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    rank int NOT NULL DEFAULT 1,    -- 1 = ใช้เป็นอันดับแรก
    is_required bool DEFAULT false, -- true = ห้ามใช้ห้องอื่น (fail ถ้าทุกห้องเต็ม)
    UNIQUE (classroom_course_id, room_id)
);
CREATE INDEX idx_ccpr_cc ON classroom_course_preferred_rooms(classroom_course_id, rank);

-- 2. ห้องของครู — มีอยู่แล้ว (instructor_room_assignments)
--    ใช้ตรงนี้เลย ไม่ต้องสร้างใหม่
--    fields: instructor_id, room_id, is_preferred, is_required, for_subjects (jsonb)
```

### Algorithm — เลือกห้องสำหรับ assignment

```python
def pick_room(course, slot, day, period):
    # 1. ลองห้องที่ classroom_course กำหนด (sorted by rank)
    cc_rooms = classroom_course_preferred_rooms[course.cc_id]
    for room in cc_rooms.sorted_by_rank():
        if room_free(room, day, period):
            return room
        if room.is_required:
            return None  # ห้ามใช้ห้องอื่น → fail

    # 2. fallback ห้องของครู (filter by for_subjects ถ้าระบุ)
    teacher_rooms = instructor_room_assignments[course.instructor_id]
    for room in teacher_rooms:
        if room.for_subjects and course.subject_id not in room.for_subjects:
            continue  # ห้องนี้ใช้เฉพาะวิชาอื่น
        if room_free(room, day, period):
            return room

    # 3. ไม่มี preference → ใช้ classroom เดิม (homeroom-style) หรือ no-room
    return course.classroom.default_room or None
```

### UI integration

หน้า config เพิ่มในแต่ละ row ครู (ในส่วน expand):
- "ห้องประจำของครู" — multi-select (teacher-level fallback)
- ในแต่ละ row classroom_course → "ห้องเฉพาะ (subject × classroom)"
  - multi-select + drag เพื่อเรียงลำดับ rank
  - toggle "บังคับ (is_required)" — ถ้าเต็มทุกห้อง → fail แทน fallback

---

## 4.6 Classroom Course Unavailable Slots (คาบที่ห้ามจัดวิชานี้ในห้องนี้)

นอกจากครูจะมีคาบไม่ว่าง — แต่ละ **(subject × classroom)** ก็มีคาบที่ "ไม่ควรจัด"
ได้เช่นกัน เช่น คณิต ม.3/1 ไม่ควรเป็นคาบสุดท้าย / พละ ม.3/2 ห้ามคาบเช้า

แยกระดับ classroom_course เพราะแต่ละห้องอาจมี constraint ต่างกัน — ห้องที่อยู่
ติดสนามอาจต้องเลี่ยงเสียงดัง, ห้องที่อยู่ใกล้ห้องน้ำคนพิการอาจมี constraint อื่น

### Auto-derive from teachers

```
effective_unavailable(cc) =
    cc.hard_unavailable_slots
  ∪ ⋃ for each teacher in cc.team:
        teacher.hard_unavailable_slots
```

ดังนั้น admin ไม่ต้องกรอกซ้ำ — UI ดึงจากครูทีมมาแสดงเป็น pre-checked + readonly

### UI behavior

```
ค23101 คณิต ม.3/1 — คาบที่ไม่จัดสอน (classroom_course-level)
                  คาบ1  คาบ2  คาบ3  คาบ4  คาบ5  คาบ6  คาบ7  คาบ8
จันทร์            ☐     ☐     ☑🔒   ☑🔒   ☐     ☐     ☐     ☐
                              (ครู ก. ไม่ว่าง)
อังคาร           ☑     ☐     ☐     ☐     ☐     ☐     ☐     ☐
                  ↑ admin ติ๊กเพิ่ม (cc-level)
พุธ              ☐     ☐     ☐     ☐     ☐     ☐     ☑🔒   ☑🔒
                                                  (ครู ข. ไม่ว่าง)
```

- 🔒 = inherited จากครูใน team — แก้ที่ row ครู
- ที่เหลือ = classroom_course-level (admin override / เพิ่มเอง)

### DB schema

```sql
ALTER TABLE classroom_courses
    ADD COLUMN hard_unavailable_slots jsonb DEFAULT '[]',
    ADD COLUMN same_day_unique bool DEFAULT true;
-- hard_unavailable_slots format: [{"day": "MON", "period_id": "uuid"}, ...]
-- max_consecutive อยู่ที่ school_settings (global) — ดู §8
```

---

## 4.7 Templates สำหรับ Phase 1 Fixed Slots

### ปัญหา

ทุกครั้งที่กดจัดอัตโนมัติ → ผลลัพธ์อาจไม่ถูกใจ → user อยากเคลียร์แล้วลองใหม่
แต่การเคลียร์ลบหมด = หายทั้ง Phase 1 (พัก/โฮมรูม/sync) ที่ตั้งใจวางไว้
→ user ต้องกลับไป batch ใหม่ทีละอัน

### ทางออก: บันทึกเป็น Template

```
┌─────────────────────────────────────────────────┐
│  Templates (ตารางพื้นฐานก่อนจัด)                  │
├─────────────────────────────────────────────────┤
│                                                  │
│  📋 ตาราง ม.ต้น 2/2569                            │
│      • พักเช้า 9:30-9:45 ทุกห้อง ม.1-3            │
│      • พักกลางวัน 12:00-13:00 ทุกห้อง            │
│      • โฮมรูม คาบ 1 จันทร์ ทุกห้อง                 │
│      • ชุมนุม คาบ 7-8 พุธ (sync)                  │
│      [ ใช้ template นี้ ]   [ แก้ไข ]   [ ลบ ]    │
│                                                  │
│  📋 ตาราง ม.ปลาย 2/2569                           │
│      ...                                         │
│                                                  │
│  [ + สร้าง Template ใหม่จากตารางปัจจุบัน ]         │
└─────────────────────────────────────────────────┘
```

### Workflow

```
1. ครั้งแรก: batch พัก/โฮมรูม/sync ตามปกติ
2. กด "บันทึกเป็น template" → ตั้งชื่อ → save
3. กดจัดอัตโนมัติ → ผลไม่ถูกใจ
4. กด "เคลียร์ทั้งหมด" → ลบทุก entry
5. กด "ใช้ template" → batch กลับมาเหมือนเดิม
6. กดจัดอัตโนมัติใหม่ → ลองใหม่ (อาจปรับ priority/constraint ก่อน)
```

### DB schema

```sql
CREATE TABLE timetable_templates (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL,
    description text,
    created_by uuid REFERENCES users(id),
    created_at timestamptz DEFAULT now()
);

CREATE TABLE timetable_template_entries (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id uuid NOT NULL REFERENCES timetable_templates(id) ON DELETE CASCADE,
    day_of_week text NOT NULL,
    period_id uuid NOT NULL REFERENCES academic_periods(id),
    entry_type text NOT NULL,                  -- BREAK / HOMEROOM / ACTIVITY / ACADEMIC
    title text,                                -- TEXT batch — title
    activity_slot_id uuid REFERENCES activity_slots(id),  -- SLOT batch — slot ref
    -- Scope ของห้อง: ใส่หลายแบบ
    grade_level_ids jsonb DEFAULT '[]',        -- ["ม.1", "ม.2"] ← apply ทุกห้องใน grade
    classroom_ids jsonb DEFAULT '[]',          -- specific classrooms (ถ้ามี)
    instructor_ids jsonb DEFAULT '[]',         -- ครูที่ tag ใน entries
    room_id uuid REFERENCES rooms(id)
);
```

### Apply template = batch hydration

```python
def apply_template(template_id, target_semester_id):
    template = load_template(template_id)
    for entry in template.entries:
        # Resolve target classrooms (จาก grade_level_ids → ห้องจริงในเทอม)
        target_classrooms = resolve_classrooms(
            entry.grade_level_ids,
            entry.classroom_ids,
            target_semester_id
        )
        # ใช้ batch endpoint เดิม — หลีกเลี่ยง logic ซ้ำ
        createBatchTimetableEntries({
            classroom_ids: target_classrooms,
            instructor_ids: entry.instructor_ids,
            days_of_week: [entry.day_of_week],
            period_ids: [entry.period_id],
            entry_type: entry.entry_type,
            title: entry.title,
            room_id: entry.room_id,
            activity_slot_id: entry.activity_slot_id,
            academic_semester_id: target_semester_id,
        })
```

### Endpoint ใหม่

```
POST   /api/academic/timetable-templates              — สร้าง template
GET    /api/academic/timetable-templates              — list
GET    /api/academic/timetable-templates/{id}         — detail
PUT    /api/academic/timetable-templates/{id}         — แก้ไข
DELETE /api/academic/timetable-templates/{id}
POST   /api/academic/timetable-templates/{id}/apply   — body: {semester_id}
POST   /api/academic/timetable-templates/from-current — สร้างจากตารางปัจจุบัน
                                                        body: {semester_id, name, filter}
DELETE /api/academic/timetable/clear                  — เคลียร์ entries ทั้งภาคเรียน
                                                        body: {semester_id, entry_types?}
                                                        default: ลบทุกประเภท
                                                        ระบุได้ว่าเก็บประเภทไหนไว้
```

### Use case combinations

| สถานการณ์ | ใช้ |
|-----------|-----|
| ปีใหม่ — เริ่มจาก 0 | + สร้าง template เป็นครั้งแรก |
| Reschedule ทั้งเทอม | clear → apply template → auto-schedule |
| Clear เฉพาะวิชา (เก็บกิจกรรม) | clear?entry_types=COURSE → auto-schedule |
| Copy จากเทอมก่อน | สร้าง template จากเทอม 1 → apply ใน เทอม 2 |

---

## 5. Priority ของครู — ตีความใหม่

`priority` **ไม่ใช่** "ใครชนะใครแพ้" (เพราะครูสองคนสอนคาบเดียวกันคือ hard
constraint อยู่แล้ว — ห้ามชน)

แต่คือ **ลำดับการ assign** — ใครถูกจัดก่อน:

```
ครู ก. (priority 1) → ตัวเลือกเยอะ → ได้คาบที่ตัวเองอยากได้แน่ ๆ
ครู ข. (priority 2) → ตัวเลือกลดลง (เพราะ ก. จองไปแล้ว)
ครู ค. (priority 3) → ต้องรับคาบที่เหลือ
```

ครูสำคัญ → ขึ้นบนสุด → ได้สิทธิ์เลือกก่อน

---

## 6. Constraints ทั้งหมด

### 6.1 Hard constraints (ห้ามชน — fail ถ้าทำไม่ได้)

| # | Constraint | แหล่งข้อมูล |
|---|-----------|-------------|
| H1 | ครู 1 คน 1 คาบ = 1 ที่ | derived |
| H2 | ห้องเรียน 1 ห้อง 1 คาบ = 1 ที่ | derived |
| H3 | นักเรียน 1 ห้อง 1 คาบ = 1 วิชา | derived |
| H4 | ครู availability (ไม่ว่างคาบไหน) | `instructor_preferences.hard_unavailable_slots` |
| H5 | จำนวนคาบ/สัปดาห์ ของแต่ละวิชา | `subjects.periods_per_week` |
| H6 | Existing entries (Phase 1) | `academic_timetable_entries` ← **ต้องอ่าน** |
| H7 | Locked slots | `timetable_locked_slots` |
| H8 | คาบติดกันต้องห้องเดิม | derived (default on) |
| H9 | ห้องที่ระบุ `is_required` ต้องว่าง | `subject_preferred_rooms.is_required` |
| H10 | คาบที่ห้ามจัด (รวมจากครู) | `classroom_courses.hard_unavailable_slots` ∪ teacher's |

### 6.2 Soft constraints (อยากได้ — เป็น penalty ไม่ใช่ fail)

| # | Constraint | แหล่งข้อมูล |
|---|-----------|-------------|
| S1 | คาบติดกันไหม (consecutive pattern) | `classroom_courses.consecutive_pattern` |
| S2 | ห้ามวันเดียวกันรหัสซ้ำ | `classroom_courses.same_day_unique` |
| S3 | ครูสอนติดไม่เกิน N คาบ (global) | `school_settings.default_max_consecutive` |
| S4 | Priority order | `instructor_preferences.priority` |

---

## 7. Algorithm — Pseudo-code

```python
def auto_schedule(semester_id, classroom_ids, force_overwrite):
    # === STEP 1: Pre-load ===
    existing = load_existing_entries(semester_id, classroom_ids)
    occupied_slots = build_occupied_map(existing)
    # occupied_slots[classroom][day][period] = "ACTIVITY"/"BREAK"/...

    locked = load_locked_slots(semester_id, classroom_ids)
    available = load_available_slots(semester_id) - occupied_slots - locked

    # === STEP 2: Sort ครู by priority ===
    instructors = load_instructors_with_prefs()
    instructors.sort(key=lambda i: (i.priority, i.id))

    # === STEP 3: Sort วิชา ===
    # วิชาที่จัดยากก่อน — น้อย slot, ครูเดียว, periods_per_week มาก
    courses = load_courses_to_schedule(classroom_ids)
    courses.sort(key=difficulty_score, reverse=True)

    # === STEP 4: Greedy + backtracking ===
    assignments = []
    for course in courses:  # course = classroom_course (subject × classroom)
        instructor = course.primary_instructor
        pattern = course.consecutive_pattern \
                  or [1] * course.subject.periods_per_week

        for chunk_size in pattern:
            # หา slot ที่:
            # - ครู available (ไม่อยู่ใน hard_unavailable + ไม่ assign อยู่)
            # - ไม่ใช่ occupied (Phase 1)
            # - ถ้า chunk_size > 1 → คาบติดกัน
            # - same_day_unique → วันนั้นห้ามมีรหัสนี้แล้ว
            # - max_consecutive → ครูไม่สอนติดเกิน N คาบ
            slot = find_best_slot(course, instructor, chunk_size, soft_constraints)
            if slot is None:
                fail(course, reason="ไม่พบช่องที่ว่างพอ")
                continue

            # เลือกห้องตาม hierarchy (subject > instructor > classroom default)
            room = pick_room(course, slot, slot.day, slot.period)
            if room is None and course.has_required_rooms:
                fail(course, reason="ห้องที่ระบุ (required) เต็มทุกห้อง")
                continue

            assignments.append((course, slot, chunk_size, room))
            mark_used(slot, instructor, course, chunk_size, room)

    # === STEP 5: Independent activities (รอบสอง) ===
    indep_activities = load_independent_activities(semester_id, classroom_ids)
    for act in indep_activities:
        # ครู = activity_slot_classroom_assignments[act.slot][act.classroom]
        # จัดเหมือนวิชาเลย
        ...

    # === STEP 6: Insert ===
    if force_overwrite:
        DELETE FROM academic_timetable_entries
            WHERE entry_type = 'COURSE' AND classroom_id = ANY($1)
    INSERT all assignments

    return result_with_failures(assignments, fails)
```

---

## 8. การตัดสินใจเรื่องข้อเพิ่มเติม

| # | ประเด็น | สถานะ | หมายเหตุ |
|---|---------|-------|---------|
| 1 | ครูสอนติดเกิน X คาบ | ✅ ทำ — **global** | `school_settings.default_max_consecutive` (ไม่ทำ per-teacher) |
| 2 | คาบเรียนยาก → ช่วงเช้า | ❌ skip | ใช้ `hard_unavailable_slots` แทน (ใส่บ่ายไม่ว่างถ้าอยากให้อยู่เช้า) |
| 3 | ห้องพิเศษ | ✅ ครอบคลุมแล้ว | `classroom_course_preferred_rooms` (Phase D) |
| 4 | First/last period restrictions | ❌ skip | ใช้ `hard_unavailable_slots` แทน (ใส่คาบ 1 ไม่ว่าง / คาบสุดท้าย) |
| 5 | ครูคนเดียวสอนหลายห้องวิชาเดียว | ✅ ครอบคลุมแล้ว | Hard constraint H1 (ครู 1 คน 1 คาบ = 1 ที่) |
| 6 | กิจกรรมที่ pin ไว้แล้ว | ✅ ทำ | H6 — scheduler อ่าน existing entries |
| 7 | แสดงผลถ้าจัดไม่สำเร็จ | ✅ ทำ — Phase E | ระบุเหตุผลชัดเจน ไม่ใช่แค่ fail |

### ที่ต้องเพิ่ม

```sql
-- Global school settings (single row หรือ key-value table)
CREATE TABLE school_settings (
    key text PRIMARY KEY,
    value jsonb NOT NULL,
    updated_at timestamptz DEFAULT now()
);
INSERT INTO school_settings (key, value) VALUES
    ('default_max_consecutive', '4');
```

UI: เพิ่มในหน้า scheduling-config ด้านบน (global section ก่อน list ครู)

```
┌──────────────────────────────────────────────────┐
│  ตั้งค่ารวม                                        │
│  • ครูสอนติดสูงสุด: [4] คาบ ← number input        │
│  ─────────────────────────────────────           │
│  ลำดับครู                                         │
│  ⋮⋮ 1. ครู ก. ▶                                  │
│  ⋮⋮ 2. ครู ข. ▶                                  │
└──────────────────────────────────────────────────┘
```

---

## 9. แผนการทำงาน (Phased Implementation)

### Phase A — Foundation (priority + unavailable + global settings)
- [ ] Migration: เพิ่ม `instructor_preferences.priority` (int, default 100)
- [ ] Migration: `school_settings` table + insert `default_max_consecutive = 4`
- [ ] Backend: API GET/PUT `/scheduling/instructor-prefs/order` — bulk update priority
- [ ] Backend: API GET/PUT `/scheduling/settings` — global settings
- [ ] Backend: scheduler รับ priority order → sort ครูก่อน assign
- [ ] Backend: scheduler ตรวจ default_max_consecutive (S3)
- [ ] Backend: อ่าน existing entries → mark occupied (H6)
- [ ] Frontend: หน้าใหม่ `/staff/academic/scheduling-config`
- [ ] Frontend: ส่วน global — input max_consecutive
- [ ] Frontend: DnD list ครู + grid วัน×คาบ toggle unavailable
- [ ] Frontend: ปุ่ม "บันทึกและจัดอัตโนมัติ" → call existing endpoint

### Phase B — Classroom-course patterns + unavailable
- [ ] Migration: `classroom_courses` ADD `consecutive_pattern` jsonb (nullable),
      `same_day_unique` bool DEFAULT true, `hard_unavailable_slots` jsonb DEFAULT '[]'
- [ ] Backend: scheduler ใช้ classroom_courses.consecutive_pattern กระจายคาบ
      (fallback [1]*periods_per_week ถ้า NULL)
- [ ] Backend: ตรวจ same_day_unique (S2)
- [ ] Backend: scheduler รวม cc + teacher unavailable เป็น effective set (H10)
- [ ] Backend: API GET classroom_courses รวม teacher_unavailable เพื่อ pre-fill UI
- [ ] Frontend: per-row expand → list **classroom_courses ที่ครูเป็น primary**
- [ ] Frontend: แต่ละ cc row — radio pattern + checkbox same_day_unique + grid 7×N
      (pre-checked readonly สำหรับคาบครู, editable สำหรับ cc-level)

### Phase C — Independent activities
- [ ] Backend: scheduler รวม activity independent ในรอบ schedule
- [ ] Backend: respect `activity_slot_classroom_assignments` (ครูประจำห้อง)
- [ ] Frontend: แสดง activity ในรายการวิชาที่จัด

### Phase D — Room assignment
- [ ] Migration: `classroom_course_preferred_rooms` table
      (cc ↔ rooms multi + rank + required)
- [ ] Backend: scheduler ใช้ pick_room() hierarchy (cc > instructor > classroom)
- [ ] Backend: respect `is_required` → fail ถ้าห้องเต็ม (H9)
- [ ] Frontend: scheduling-config — ห้องประจำครู (teacher-level)
      + ใน cc row → ห้องเฉพาะ (cc-level multi-select + drag rank + is_required)

### Phase E — Failure reasons
- [ ] Backend: scheduler return reason ละเอียด — เช่น
      "ค23101 ม.3/1 จัดไม่ได้ — คาบที่เหลือทุกคาบ ครู ก. ไม่ว่าง / ห้อง Lab1 เต็ม"
- [ ] Frontend: แสดง list ของวิชาที่จัดไม่ได้ พร้อมเหตุผล + ปุ่ม "แก้ที่ config"

### Phase F — Templates (Phase 1 enhancement)
- [x] Migration: `timetable_templates` + `timetable_template_entries` (112)
- [x] Backend: CRUD endpoints — list/create/update/delete templates (PUT for edit added)
- [x] Backend: POST `/templates/{id}/apply` — hydrate เข้า semester ผ่าน batch logic เดิม
- [x] Backend: POST `/templates/from-current` — สร้าง template จากตารางปัจจุบัน
- [x] Backend: DELETE `/timetable/clear?entry_types=` — เคลียร์ตามชนิด
- [x] Frontend: หน้า `/staff/academic/timetable-templates`
- [x] Frontend: ปุ่ม link "Templates" + redirect "จัดอัตโนมัติ" → /scheduling-config ใน /timetable
      (ใน /timetable-templates page มี save/clear/apply ครบ)

### Phase G — UX polish
- [ ] Realtime preview ก่อนกดจัด — **deferred** (complex, scheduler dry-run mode)
- [x] Conflict report — แสดงสาเหตุที่จัดไม่ได้ (Phase E)
- [x] Undo last auto-schedule (migration 113 + endpoint)

---

## 10. คำถามก่อนเริ่ม implement

1. **Scope ของ Phase แรก** — เริ่ม Phase A อย่างเดียวก่อนไหม? หรือรวม B เลย?
2. **Pattern ของวิชา** — ครูเลือกเอง หรือ admin ตั้ง default ที่ subject แล้วครู
   override เฉพาะตัวเอง?
3. **ข้อเพิ่มเติม 1-7** — เห็นด้วยกับอันไหน อันไหนไม่ต้อง?
4. **กิจกรรม independent** — เพิ่ม Phase A เลย หรือคงไว้ Phase C?
5. **Failure handling** — ถ้า scheduler จัดไม่ได้ทุกคาบ → ทำต่อแบบ partial หรือ
   rollback ทั้งหมด?

---

## ภาคผนวก — ไฟล์ที่ต้องแก้

```
backend-school/
├── migrations/
│   ├── XXX_instructor_priority_school_settings.sql      ← ใหม่ (Phase A)
│   ├── YYY_classroom_course_scheduling.sql              ← ใหม่ (Phase B)
│   ├── ZZZ_classroom_course_preferred_rooms.sql         ← ใหม่ (Phase D)
│   └── WWW_timetable_templates.sql                       ← ใหม่ (Phase F)
├── src/modules/academic/
│   ├── handlers/
│   │   ├── scheduling.rs                            ← แก้ algorithm
│   │   └── scheduling_config.rs                     ← มีบางส่วนแล้ว ขยาย
│   ├── services/
│   │   ├── scheduler.rs                             ← refactor algorithm
│   │   └── scheduler_data.rs                        ← เพิ่ม load_existing_entries
│   └── mod.rs                                        ← register routes ใหม่

frontend-school/
├── src/lib/api/
│   └── scheduling.ts                                ← ใหม่ (DnD + constraints)
├── src/routes/(app)/staff/academic/
│   ├── scheduling-config/
│   │   ├── +page.svelte                             ← ใหม่
│   │   └── +page.ts                                 ← ใหม่ (_meta สำหรับ menu)
│   └── timetable/
│       └── +page.svelte                             ← เพิ่ม link ไป config
```
