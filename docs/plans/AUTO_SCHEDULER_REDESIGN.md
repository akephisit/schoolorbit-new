# Auto-Scheduler Redesign — แผนปรับปรุงการจัดตารางสอนอัตโนมัติ

> สถานะ: Draft (รอ confirm scope ก่อน implement)
> วันที่: 2026-04-26

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
| `subjects.periods_per_week` | ✅ มี |
| `subjects.consecutive_pattern` (jsonb) | ❌ ต้องเพิ่ม |
| `subjects.same_day_unique` (bool) | ❌ ต้องเพิ่ม |
| `subjects.max_consecutive` (int) | ❌ ต้องเพิ่ม |
| `subject_preferred_rooms` (table) | ❌ ต้องเพิ่ม |
| `instructor_room_assignments` | ✅ มี (ใช้เป็น fallback) |
| `timetable_locked_slots` | ✅ มี (ใช้ pin เพิ่มเติมได้) |

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

- คลิก ▼ บน row ครู → ขยายแสดง:
  1. **Grid 7×N** — วัน × คาบ → คลิกเพื่อ toggle "ไม่ว่าง" (เก็บใน `hard_unavailable_slots`)
  2. **ห้องประจำของครู** — multi-select rooms (default ของห้องที่ครูสอน)
  3. **List วิชา** — แสดงเฉพาะวิชาที่ครูสอน (จาก `classroom_course_instructors`)
     - radio button เลือก consecutive pattern
     - checkbox `same_day_unique` (default ✓)
     - **multi-select ห้อง** เฉพาะวิชา (override ห้องของครู)

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
ALTER TABLE subjects ADD COLUMN consecutive_pattern jsonb DEFAULT '[1]';
-- เก็บ array เช่น [1,1,1], [2,1], [3] — ยืดหยุ่นรองรับทุกแบบ
-- 5 คาบ → [2,2,1], [3,1,1], [1,1,1,1,1], ...
```

**Validation:** `sum(pattern) == periods_per_week`

---

## 4.5 Room Assignment (ห้องเรียนของแต่ละวิชา)

### Hierarchy การเลือกห้อง (จากสำคัญสุด → ต่ำสุด)

```
1. Subject preferred rooms (multi)  ← ตั้งระดับวิชา (override ครู)
   เช่น ฟิสิกส์ → [Lab1, Lab2, Lab3]
   ────────────────────────────────────────
2. Instructor preferred rooms (multi) ← ตั้งระดับครู (default)
   เช่น ครูต้น → [ห้อง ม.3/1] (ห้องประจำชั้น)
   ────────────────────────────────────────
3. (no preference) → ใช้ห้องไหนก็ได้ที่ว่าง / หรือไม่กำหนดห้อง
```

### Use cases

| สถานการณ์ | Subject rooms | Teacher rooms | Scheduler ทำ |
|-----------|--------------|---------------|--------------|
| คณิตประจำชั้น | (ไม่ระบุ) | [ม.3/1] | ใช้ห้อง ม.3/1 |
| ฟิสิกส์ | [Lab1, Lab2] | [ม.3/1] | Lab1 ก่อน → ถ้าเต็ม ลอง Lab2 |
| พละ | [สนามฟุตบอล, สนามบาส] | (ไม่ระบุ) | สนามฟุตบอลก่อน |
| HOMEROOM | (ไม่ระบุ) | (ไม่ระบุ) | ใช้ห้อง classroom (เช่น ม.3/1) |
| คณิตเสริม (extra) | [ห้องคอม] | [ม.3/1] | ห้องคอมก่อน (subject ชนะ) |

### DB schema

```sql
-- 1. ห้องที่วิชานี้ใช้สอนได้ (multi, ranked)
CREATE TABLE subject_preferred_rooms (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id uuid NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    room_id uuid NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    rank int NOT NULL DEFAULT 1,  -- 1 = ใช้เป็นอันดับแรก
    is_required bool DEFAULT false,  -- true = ห้ามใช้ห้องอื่น (fail ถ้าทุกห้องเต็ม)
    UNIQUE (subject_id, room_id)
);
CREATE INDEX idx_spr_subject ON subject_preferred_rooms(subject_id, rank);

-- 2. ห้องของครู — มีอยู่แล้ว (instructor_room_assignments)
--    ใช้ตรงนี้เลย ไม่ต้องสร้างใหม่
--    fields: instructor_id, room_id, is_preferred, is_required, for_subjects (jsonb)
```

### Algorithm — เลือกห้องสำหรับ assignment

```python
def pick_room(course, slot, day, period):
    # 1. ลองห้องที่ subject กำหนด (sorted by rank)
    for room in subject_preferred_rooms[course.subject_id]:
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
- "ห้องประจำของครู" — multi-select
- ในวิชาแต่ละวิชา → "ห้องเฉพาะวิชา (override)" — multi-select + drag เพื่อเรียงลำดับ
- toggle "บังคับ (is_required)" — ถ้าเต็มทุกห้อง → fail แทน fallback

ที่หน้า `/staff/academic/subjects` (ที่มีอยู่แล้ว) เพิ่ม:
- field "ห้องที่ใช้สอน" — multi-select rooms + ลำดับ + bool required

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

### 6.2 Soft constraints (อยากได้ — เป็น penalty ไม่ใช่ fail)

| # | Constraint | แหล่งข้อมูล |
|---|-----------|-------------|
| S1 | คาบติดกันไหม (consecutive pattern) | `subjects.consecutive_pattern` |
| S2 | ห้ามวันเดียวกันรหัสซ้ำ | `subjects.same_day_unique` |
| S3 | ครูสอนติดไม่เกิน N คาบ | `subjects.max_consecutive` (default 4) |
| S4 | คาบเรียนยาก (คณิต/วิทย์) → ช่วงเช้า | `subjects.prefer_morning` |
| S5 | First/last period restrictions | `subjects.avoid_first` / `avoid_last` |
| S6 | Priority order | `instructor_preferences.priority` |

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
    for course in courses:
        instructor = course.primary_instructor
        pattern = course.consecutive_pattern or [1]*course.periods_per_week

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

## 8. ข้อเพิ่มเติมที่แนะนำ (เกินจาก scope ที่ user ระบุ)

```
┌─────────────────────────────────────────────────────┐
│ ข้อเพิ่มเติมที่ระบบโรงเรียนไทยมักต้องการ                │
├─────────────────────────────────────────────────────┤
│                                                      │
│ 1. ครูสอนติดเกิน X คาบไม่ได้ (S3)                     │
│    → "ครูเหนื่อย ต้องพักบ้าง"                          │
│    → default 4 คาบติด                               │
│                                                      │
│ 2. คาบเรียนยาก → ช่วงเช้า prefer (S4)                │
│    → soft constraint บน subject                     │
│                                                      │
│ 3. ห้องพิเศษ (lab, สนาม, คอม)                        │
│    → วิชาฟิสิกส์ต้อง lab → "ถ้า lab ว่างเท่านั้น"      │
│    → Subject ↔ Room compatibility table             │
│                                                      │
│ 4. คาบ first/last ของวัน (S5)                        │
│    → วิชาบางอย่างไม่ควรเป็นคาบแรก/สุดท้าย              │
│                                                      │
│ 5. ครูคนเดียวสอนหลายห้องวิชาเดียว                     │
│    → ต้องไม่สอนพร้อมกัน (auto handle จาก H1)          │
│                                                      │
│ 6. กิจกรรมที่ pin ไว้แล้ว (Phase 1)                   │
│    → scheduler อ่าน existing entries → respect (H6)  │
│                                                      │
│ 7. แสดงผลถ้าจัดไม่สำเร็จ                              │
│    → "ครู ก. คณิต ม.3/1 จัดไม่ได้ — ครูชนกับ X"        │
│    → ระบุเหตุผลชัดเจน ไม่ใช่แค่ fail                  │
└─────────────────────────────────────────────────────┘
```

---

## 9. แผนการทำงาน (Phased Implementation)

### Phase A — Foundation (priority + unavailable)
- [ ] Migration: เพิ่ม `instructor_preferences.priority` (int, default 100)
- [ ] Backend: API GET/PUT `/scheduling/instructor-prefs/order` — bulk update priority
- [ ] Backend: scheduler รับ priority order → sort ครูก่อน assign
- [ ] Backend: อ่าน existing entries → mark occupied (H6)
- [ ] Frontend: หน้าใหม่ `/staff/academic/scheduling-config`
- [ ] Frontend: DnD list ครู + grid วัน×คาบ toggle unavailable
- [ ] Frontend: ปุ่ม "บันทึกและจัดอัตโนมัติ" → call existing endpoint

### Phase B — Subject patterns
- [ ] Migration: `subjects.consecutive_pattern` jsonb, `same_day_unique` bool, `max_consecutive` int
- [ ] Backend: scheduler ใช้ consecutive_pattern กระจายคาบ
- [ ] Backend: ตรวจ same_day_unique + max_consecutive (S2, S3)
- [ ] Frontend: per-row expand → list วิชา + radio pattern + checkbox

### Phase C — Independent activities
- [ ] Backend: scheduler รวม activity independent ในรอบ schedule
- [ ] Backend: respect `activity_slot_classroom_assignments` (ครูประจำห้อง)
- [ ] Frontend: แสดง activity ในรายการวิชาที่จัด

### Phase D — Room assignment
- [ ] Migration: `subject_preferred_rooms` table (subject ↔ rooms multi + rank + required)
- [ ] Backend: scheduler ใช้ pick_room() hierarchy (subject > instructor > classroom)
- [ ] Backend: respect `is_required` → fail ถ้าห้องเต็ม (H9)
- [ ] Frontend: subjects page — เพิ่ม field "ห้องที่ใช้สอน"
- [ ] Frontend: scheduling-config — ห้องประจำครู + ห้องเฉพาะวิชา

### Phase E — Soft constraints
- [ ] Backend: prefer morning, avoid first/last (S4, S5)
- [ ] Backend: failure reasons ละเอียด

### Phase F — UX polish
- [ ] Realtime preview ก่อนกดจัด
- [ ] Conflict report — แสดงสาเหตุที่จัดไม่ได้
- [ ] Undo last auto-schedule

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
│   ├── XXX_instructor_priority.sql                  ← ใหม่
│   ├── YYY_subject_scheduling_pattern.sql           ← ใหม่
│   └── ZZZ_subject_preferred_rooms.sql              ← ใหม่
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
