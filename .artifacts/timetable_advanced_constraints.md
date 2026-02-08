# 🎯 Advanced Timetable Constraints
## เงื่อนไขขั้นสูงสำหรับการจัดตารางอัตโนมัติ

> **เพิ่มเติมจากเอกสารหลัก**: เงื่อนไขพิเศษที่ต้องใช้ในโรงเรียนจริง

---

## 📌 New Advanced Constraints

### **HC-7: Consecutive Period Requirements** 🔴 HARD
**กฎ**: บางวิชาเมื่อมีการเรียนในวันใด ต้องเรียนติดกันตามจำนวนคาบที่กำหนด

#### **สำคัญ! ความหมายของ "ติดกัน":**
```
"วิชาต้อง 2 คาบติด" หมายถึง:
  → วันไหนที่มีเรียน ต้องเรียน 2 คาบติดกัน
  → แต่ไม่จำเป็นต้องเรียนทั้งหมดในวันเดียว
  → สามารถแบ่งเป็นหลายวันได้

ตัวอย่าง - พละ 3 คาบ/สัปดาห์, ต้อง 2 คาบติด:
  ✅ วันจันทร์ คาบ 5-6: พละ (2 คาบติด)
  ✅ วันพฤหัส คาบ 3: พละ (1 คาบเดี่ยว - OK!)
  
  ❌ วันจันทร์ คาบ 5: พละ (1 คาบ)
  ❌ วันพฤหัส คาบ 3-4: พละ (2 คาบ)
     → ผิด! วันจันทร์ต้องมี 2 คาบติด ไม่ใช่ 1 คาบเดี่ยว

ตัวอย่าง - ปฏิบัติการเคมี 4 คาบ/สัปดาห์, ต้อง 2 คาบติด:
  ✅ วันอังคาร คาบ 3-4: LAB (2 คาบติด)
  ✅ วันพฤหัส คาบ 5-6: LAB (2 คาบติด)
  
  ❌ วันอังคาร คาบ 3-5: LAB (3 คาบติด)
  ❌ วันพฤหัส คาบ 7: LAB (1 คาบ)
     → ผิด! วันพฤหัสต้องมี 2 คาบติด ไม่ใช่ 1 คาบเดี่ยว
```

#### **Use Cases:**
```
✅ พละ 2-3 คาบ/สัปดาห์: ต้อง 2 คาบติดทุกครั้ง (อุ่นเครื่อง + เล่น + อาบน้ำ)
✅ ปฏิบัติการวิทย์ 4 คาบ/สัปดาห์: ต้อง 2 คาบติดทุกครั้ง (ตั้งอุปกรณ์ + ทดลอง)
✅ ปฏิบัติการคอม 3 คาบ/สัปดาห์: ต้อง 2 คาบติดทุกครั้ง (โหลดโปรแกรม + ทำงาน)
✅ ศิลปะ 4 คาบ/สัปดาห์: ต้อง 2 คาบติดทุกครั้ง (เตรียมของ + ฝึก)

❌ คณิต, ไทย, อังกฤษ: ไม่บังคับ (แยกเป็น 1 คาบได้)
```

#### **Database Schema:**

```sql
-- Add to subjects table
ALTER TABLE subjects 
ADD COLUMN IF NOT EXISTS min_consecutive_periods INTEGER DEFAULT 1,
ADD COLUMN IF NOT EXISTS max_consecutive_periods INTEGER DEFAULT 2,
ADD COLUMN IF NOT EXISTS allow_single_period BOOLEAN DEFAULT true;

COMMENT ON COLUMN subjects.min_consecutive_periods IS 'จำนวนคาบต่อเนื่องขั้นต่ำต่อวัน (1=ไม่บังคับ, 2+=ต้องติดกัน)';
COMMENT ON COLUMN subjects.max_consecutive_periods IS 'จำนวนคาบต่อเนื่องสูงสุดต่อวัน';
COMMENT ON COLUMN subjects.allow_single_period IS 'อนุญาตให้มี 1 คาบเดี่ยวได้ไหม (สำหรับคาบที่เหลือ)';

-- Examples:

-- 1. พละ: ต้อง 2 คาบติด, อนุญาต 1 คาบเดี่ยว (สำหรับคาบที่เหลือ)
UPDATE subjects SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 2,
    allow_single_period = true  -- อนุญาตให้มี 1 คาบเดี่ยว
WHERE subject_type = 'PE';

/* 
   ผลลัพถ์ - พละ 3 คาบ/สัปดาห์:
   ✅ จันทร์ คาบ 5-6: พละ (2 คาบติด)
   ✅ พฤหัส คาบ 3: พละ (1 คาบเดี่ยว - OK!)
*/

-- 2. ปฏิบัติการ LAB: ต้อง 2 คาบติด, ไม่อนุญาต 1 คาบเดี่ยว
UPDATE subjects SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 3,
    allow_single_period = false  -- ห้าม 1 คาบเดี่ยว
WHERE code LIKE 'LAB%';

/*
   ผลลัพถ์ - ปฏิบัติการเคมี 4 คาบ/สัปดาห์:
   ✅ อังคาร คาบ 3-4: LAB (2 คาบติด)
   ✅ พฤหัส คาบ 5-6: LAB (2 คาบติด)
   ❌ อังคาร คาบ 3-5: LAB (3 คาบติด)
   ❌ พฤหัส คาบ 7: LAB (1 คาบเดี่ยว - ไม่อนุญาต!)
*/

-- 3. วิชาทั่วไป: ไม่บังคับติดกัน
UPDATE subjects SET 
    min_consecutive_periods = 1,  -- ไม่บังคับ
    max_consecutive_periods = 2,
    allow_single_period = true
WHERE subject_type = 'CORE';

/*
   ผลลัพถ์ - คณิต 4 คาบ/สัปดาห์:
   ✅ จันทร์ คาบ 1: คณิต (1 คาบ OK)
   ✅ อังคาร คาบ 3-4: คณิต (2 คาบติด OK)
   ✅ พฤหัส คาบ 7: คณิต (1 คาบ OK)
*/
```

#### **Implementation:**

```rust
struct ConsecutiveRequirement {
    subject_id: Uuid,
    min_consecutive: i32,      // จำนวนคาบต่อเนื่องขั้นต่ำ (เช่น 2)
    max_consecutive: i32,      // จำนวนคาบต่อเนื่องสูงสุด (เช่น 2)
    allow_single_period: bool, // อนุญาตให้มี 1 คาบเดี่ยวได้ไหม (สำหรับคาบที่เหลือ)
}

fn validate_consecutive_periods(
    course: &CourseToSchedule,
    assignments: &[Assignment],
    requirement: &ConsecutiveRequirement,
) -> Result<(), String> {
    // ถ้าไม่บังคับ consecutive = ไม่ต้องเช็ค
    if requirement.min_consecutive <= 1 {
        return Ok(());
    }
    
    // Group assignments by day
    let assignments_by_day = group_by_day(assignments);
    
    // Check each day separately
    for (day, day_assignments) in assignments_by_day {
        let period_count = day_assignments.len() as i32;
        
        // ถ้ามีแค่ 1 คาบในวันนี้
        if period_count == 1 {
            // เช็คว่าอนุญาตให้มี 1 คาบเดี่ยวไหม
            if !requirement.allow_single_period {
                return Err(format!(
                    "Subject {} requires at least {} consecutive periods per day, but {} has only 1 period",
                    course.subject_code,
                    requirement.min_consecutive,
                    day
                ));
            }
            // OK - อนุญาตให้มี 1 คาบเดี่ยว (สำหรับคาบที่เหลือ)
            continue;
        }
        
        // ถ้ามี 2 คาบขึ้นไป ต้องเช็คว่าติดกันไหม
        let periods = get_period_numbers(&day_assignments);
        
        // Check if consecutive
        if !is_consecutive(&periods) {
            return Err(format!(
                "Subject {} periods on {} must be consecutive, got {:?}",
                course.subject_code,
                day,
                periods
            ));
        }
        
        // Check min/max consecutive
        if period_count < requirement.min_consecutive {
            return Err(format!(
                "Subject {} requires at least {} consecutive periods on {}, got {}",
                course.subject_code,
                requirement.min_consecutive,
                day,
                period_count
            ));
        }
        
        if period_count > requirement.max_consecutive {
            return Err(format!(
                "Subject {} allows max {} consecutive periods on {}, got {}",
                course.subject_code,
                requirement.max_consecutive,
                day,
                period_count
            ));
        }
    }
    
    Ok(())
}

// Helper: check if period numbers are consecutive
fn is_consecutive(periods: &[i32]) -> bool {
    if periods.len() <= 1 {
        return true;
    }
    
    let mut sorted = periods.to_vec();
    sorted.sort();
    
    for i in 1..sorted.len() {
        if sorted[i] != sorted[i-1] + 1 {
            return false; // Gap found
        }
    }
    
    true
}

// During scheduling, must ensure consecutive periods
fn schedule_course_with_consecutive(
    course: &CourseToSchedule,
    time_slots: &[TimeSlot],
    requirement: &ConsecutiveRequirement,
) -> Result<Vec<Assignment>, AppError> {
    let periods_needed = course.periods_needed;
    let min_consecutive = requirement.min_consecutive;
    
    // If must be consecutive
    if min_consecutive > 1 {
        // Try to find consecutive slots
        return schedule_consecutive_slots(
            course,
            time_slots,
            periods_needed,
            min_consecutive,
        );
    } else {
        // Can be split
        return schedule_normal(course, time_slots, periods_needed);
    }
}

fn schedule_consecutive_slots(
    course: &CourseToSchedule,
    time_slots: &[TimeSlot],
    total_needed: i32,
    consecutive_size: i32,
) -> Result<Vec<Assignment>, AppError> {
    let mut assignments = Vec::new();
    let mut remaining = total_needed;
    
    // Group slots by day
    let slots_by_day = group_slots_by_day(time_slots);
    
    while remaining > 0 {
        let chunk_size = consecutive_size.min(remaining);
        
        // Find consecutive slots
        for (day, day_slots) in &slots_by_day {
            if let Some(consecutive_slots) = find_consecutive_available_slots(
                day_slots,
                chunk_size as usize,
            ) {
                // Assign these slots
                for slot in consecutive_slots {
                    assignments.push(Assignment {
                        course_id: course.id,
                        time_slot: slot.clone(),
                        room_id: None,
                    });
                }
                remaining -= chunk_size;
                break;
            }
        }
        
        if remaining > 0 && remaining == total_needed {
            // Could not find any consecutive slots
            return Err(AppError::BadRequest(format!(
                "Cannot find {} consecutive periods for {}",
                consecutive_size, course.subject_code
            )));
        }
    }
    
    Ok(assignments)
}

fn find_consecutive_available_slots(
    slots: &[TimeSlot],
    count: usize,
) -> Option<Vec<TimeSlot>> {
    // Sort by period order
    let mut sorted_slots = slots.to_vec();
    sorted_slots.sort_by_key(|s| get_period_order(&s.period_id));
    
    // Find consecutive window
    for i in 0..=sorted_slots.len().saturating_sub(count) {
        let window = &sorted_slots[i..i + count];
        
        // Check if truly consecutive
        if is_consecutive_periods(window) {
            return Some(window.to_vec());
        }
    }
    
    None
}
```

#### **Examples:**

```rust
// Example 1: พละ 3 คาบ/สัปดาห์ - ต้อง 2 คาบติด, อนุญาต 1 คาบเดี่ยว
ConsecutiveRequirement {
    subject_id: pe_subject_id,
    min_consecutive: 2,
    max_consecutive: 2,
    allow_single_period: true, // อนุญาต 1 คาบเดี่ยวสำหรับคาบที่เหลือ
}

// ผลลัพธ์:
✅ จันทร์ คาบ 5-6: พละ (2 คาบติด - ดี!)
✅ พฤหัส คาบ 3: พละ (1 คาบเดี่ยว - OK เพราะ allow_single_period = true)

❌ จันทร์ คาบ 5: พละ (1 คาบ)
   พฤหัส คาบ 3-4: พละ (2 คาบ)
   → ผิด! วันจันทร์มี 1 คาบ แต่ allow_single_period = true ควรจัด 2 คาบให้

❌ จันทร์ คาบ 5-7: พละ (3 คาบติด)
   → ผิด! เกิน max_consecutive (2)

---

// Example 2: ปฏิบัติการเคมี 4 คาบ/สัปดาห์ - ต้อง 2 คาบติด, ห้าม 1 คาบเดี่ยว
ConsecutiveRequirement {
    subject_id: chem_lab_id,
    min_consecutive: 2,
    max_consecutive: 3,
    allow_single_period: false, // ห้าม 1 คาบเดี่ยว
}

// ผลลัพธ์:
✅ อังคาร คาบ 3-4: LAB (2 คาบติด)
✅ พฤหัส คาบ 5-6: LAB (2 คาบติด)

✅ อังคาร คาบ 3-5: LAB (3 คาบติด - OK, ไม่เกิน max)
✅ พฤหัส คาบ 6: LAB (1 คาบเดี่ยว... เดี๋ยว!)
   → ผิด! allow_single_period = false ห้าม 1 คาบเดี่ยว

---

// Example 3: คณิต 4 คาบ/สัปดาห์ - ไม่บังคับติดกัน
ConsecutiveRequirement {
    subject_id: math_id,
    min_consecutive: 1, // ไม่บังคับ
    max_consecutive: 2,
    allow_single_period: true,
}

// ผลลัพธ์:
✅ จันทร์ คาบ 1: คณิต (1 คาบ OK)
✅ อังคาร คาบ 3-4: คณิต (2 คาบติด OK)
✅ พฤหัส คาบ 7: คณิต (1 คาบ OK)
→ ทุกอย่าง OK เพราะ min_consecutive = 1 (ไม่บังคับติดกัน)

---

// Example 4: ศิลปะ 2 คาบ/สัปดาห์ - ต้อง 2 คาบติด, ทั้งหมดในวันเดียว
ConsecutiveRequirement {
    subject_id: art_id,
    min_consecutive: 2,
    max_consecutive: 2,
    allow_single_period: false, // ห้าม 1 คาบเดี่ยว
}

// ผลลัพธ์:
✅ พฤหัส คาบ 5-6: ศิลปะ (2 คาบติด)
→ Perfect! ครบ 2 คาบในวันเดียว

❌ จันทร์ คาบ 5: ศิลปะ (1 คาบ)
   พฤหัส คาบ 6: ศิลปะ (1 คาบ)
   → ผิด! allow_single_period = false ห้ามแยก
```

**Priority**: 🔴 Critical (for specific subjects)
**Enforcement**: Hard constraint for subjects with `force_consecutive = true`

---

### **HC-8: Fixed Room Assignment** 🔴 HARD
**กฎ**: บางครูต้องสอนในห้องเฉพาะประจำเสมอ

#### **Use Cases:**
```
✅ ครูสมชาย (คณิต): สอนห้อง 201 ประจำเสมอ
✅ ครูสมหญิง (ภาษาไทย): สอนห้อง 305 ประจำเสมอ
✅ ครูพละ: ใช้สนามกีฬาเสมอ
✅ ครู Computer: ใช้ห้อง Computer Lab 1 เสมอ

❌ ครูทั่วไป: ไม่มีห้องประจำ (ใช้ห้องของแต่ละห้องเรียน)
```

#### **Database Schema:**

```sql
-- New table: Instructor Room Assignments
CREATE TABLE instructor_room_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- Priority
    is_preferred BOOLEAN DEFAULT false,  -- ชอบใช้ห้องนี้ (soft)
    is_required BOOLEAN DEFAULT false,   -- ต้องใช้ห้องนี้ (hard)
    
    -- Conditions
    for_subjects JSONB DEFAULT '[]'::jsonb, -- ระบุเฉพาะวิชา [], null=ทุกวิชา
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique: instructor can have multiple room assignments
    CONSTRAINT unique_instructor_room_year UNIQUE(instructor_id, room_id, academic_year_id)
);

CREATE INDEX idx_instructor_room_instructor ON instructor_room_assignments(instructor_id);
CREATE INDEX idx_instructor_room_room ON instructor_room_assignments(room_id);

-- Examples:
INSERT INTO instructor_room_assignments (
    instructor_id, room_id, academic_year_id,
    is_required, for_subjects
) VALUES 
-- ครูสมชาย สอนคณิตที่ห้อง 201 เสมอ
('uuid-teacher-somchai', 'uuid-room-201', 'uuid-year-2568', true, '["MATH"]'),

-- ครูพละ ใช้สนามเสมอ
('uuid-teacher-pe', 'uuid-field-1', 'uuid-year-2568', true, '["PE"]'),

-- ครูคอม ชอบใช้ Lab 1 (ไม่บังคับ)
('uuid-teacher-comp', 'uuid-lab-comp-1', 'uuid-year-2568', false, '["COMPUTER"]');
```

#### **Implementation:**

```rust
struct InstructorRoomAssignment {
    instructor_id: Uuid,
    room_id: Uuid,
    is_required: bool,
    for_subjects: Vec<String>, // Empty = all subjects
}

async fn get_instructor_room_assignment(
    pool: &PgPool,
    instructor_id: Uuid,
    subject_id: Uuid,
) -> Result<Option<InstructorRoomAssignment>, AppError> {
    let assignment = sqlx::query_as::<_, InstructorRoomAssignment>(
        "SELECT * FROM instructor_room_assignments
         WHERE instructor_id = $1
           AND (for_subjects = '[]'::jsonb 
                OR for_subjects @> to_jsonb($2::text))
           AND is_required = true"
    )
    .bind(instructor_id)
    .bind(subject_id.to_string())
    .fetch_optional(pool)
    .await?;
    
    Ok(assignment)
}

fn assign_room_for_course(
    course: &CourseToSchedule,
    assignment: &Assignment,
    instructor_room: &Option<InstructorRoomAssignment>,
) -> Option<Uuid> {
    // Priority 1: Fixed room for instructor
    if let Some(room_assignment) = instructor_room {
        if room_assignment.is_required {
            return Some(room_assignment.room_id);
        }
    }
    
    // Priority 2: Subject requires special room (LAB)
    if let Some(room_type) = &course.required_room_type {
        return find_available_room_by_type(room_type);
    }
    
    // Priority 3: Instructor prefers certain room
    if let Some(room_assignment) = instructor_room {
        if !room_assignment.is_required {
            return Some(room_assignment.room_id);
        }
    }
    
    // Priority 4: Use classroom's default room
    None // NULL = use classroom's home room
}

// Validation: Check if assigned room is available
async fn validate_room_assignment(
    pool: &PgPool,
    assignment: &Assignment,
    instructor_room: &Option<InstructorRoomAssignment>,
) -> Result<(), AppError> {
    let room_id = match instructor_room {
        Some(r) if r.is_required => r.room_id,
        _ => return Ok(()), // No fixed room = OK
    };
    
    // Check if room is available at this time
    let conflict = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
            SELECT 1 FROM academic_timetable_entries
            WHERE room_id = $1
              AND day_of_week = $2
              AND period_id = $3
              AND id != $4
        )"
    )
    .bind(room_id)
    .bind(&assignment.time_slot.day)
    .bind(assignment.time_slot.period_id)
    .bind(assignment.id)
    .fetch_one(pool)
    .await?;
    
    if conflict {
        return Err(AppError::BadRequest(format!(
            "Required room {} is not available at {} {}",
            room_id, assignment.time_slot.day, assignment.time_slot.period_id
        )));
    }
    
    Ok(())
}
```

#### **UI: Setting Instructor Room Assignment**

```svelte
<!-- /staff/academic/instructor-rooms/+page.svelte -->

<script lang="ts">
    let instructors = $state<Instructor[]>([]);
    let rooms = $state<Room[]>([]);
    let assignments = $state<InstructorRoomAssignment[]>([]);
    
    async function assignRoom(instructorId: string, roomId: string, isRequired: boolean) {
        await apiClient.post('/academic/instructor-room-assignments', {
            instructor_id: instructorId,
            room_id: roomId,
            is_required: isRequired,
            for_subjects: [] // All subjects
        });
    }
</script>

<Table>
    <TableHeader>
        <TableRow>
            <TableHead>ครู</TableHead>
            <TableHead>ห้องประจำ</TableHead>
            <TableHead>บังคับ</TableHead>
        </TableRow>
    </TableHeader>
    <TableBody>
        {#each instructors as instructor}
            <TableRow>
                <TableCell>{instructor.name}</TableCell>
                <TableCell>
                    <Select bind:value={instructor.assigned_room_id}>
                        <SelectTrigger>
                            <SelectValue placeholder="เลือกห้อง" />
                        </SelectTrigger>
                        <SelectContent>
                            {#each rooms as room}
                                <SelectItem value={room.id}>{room.code}</SelectItem>
                            {/each}
                        </SelectContent>
                    </Select>
                </TableCell>
                <TableCell>
                    <Checkbox bind:checked={instructor.is_room_required} />
                </TableCell>
            </TableRow>
        {/each}
    </TableBody>
</Table>
```

#### **Examples:**

```rust
// ครูสมชาย สอนคณิตที่ห้อง 201 เสมอ
InstructorRoomAssignment {
    instructor_id: somchai_id,
    room_id: room_201_id,
    is_required: true,
    for_subjects: vec!["MATH".to_string()],
}

// ผลลัพธ์:
✅ ม.4/1 คาบ 1: คณิต (ครูสมชาย) → ห้อง 201
✅ ม.5/2 คาบ 3: คณิต (ครูสมชาย) → ห้อง 201
❌ ม.4/1 คาบ 1: คณิต (ครูสมชาย) → ห้อง 305 (ผิด! ต้อง 201)

// ครูพละ ใช้สนามเสมอ
InstructorRoomAssignment {
    instructor_id: pe_teacher_id,
    room_id: field_id,
    is_required: true,
    for_subjects: vec!["PE".to_string()],
}

// ผลลัพถ์:
✅ ม.4/1 คาบ 5-6: พละ → สนามกีฬา
❌ ม.4/1 คาบ 5-6: พละ → ห้อง 101 (ผิด!)
```

**Priority**: 🔴 Critical
**Enforcement**: Hard constraint if `is_required = true`, Soft if `is_preferred = true`

---

### **HC-9: Pre-Assigned / Locked Slots** 🔴 HARD
**กฎ**: บางคาบถูก lock ไว้แล้ว ห้ามเปลี่ยนแปลง

#### **Use Cases:**
```
✅ ทุกห้อง ม.4 เรียนพละวันพุธคาบ 5-6 (โรงเรียนกำหนด)
✅ ทุกห้อง ม.1-ม.6 เรียนแนะแนววันศุกร์คาบ 1 (โรงเรียนกำหนด)
✅ ห้อง ม.4/1 เรียนคณิตวันจันทร์คาบ 1 (ผู้บริหารกำหนดพิเศษ)
✅ ห้อง ม.5 เรียนชุมนุมวันพฤหัสคาบ 7-8 (โรงเรียนกำหนด)

❌ วิชาอื่น ๆ: จัดอัตโนมัติได้
```

#### **Database Schema:**

```sql
-- New table: Pre-assigned Timetable Slots
CREATE TABLE timetable_locked_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,
    
    -- Scope (กำหนดขอบเขต)
    scope_type VARCHAR(20) NOT NULL, -- 'CLASSROOM', 'GRADE_LEVEL', 'ALL_SCHOOL'
    scope_ids JSONB, -- classroom_ids or grade_level_ids (null if ALL_SCHOOL)
    
    -- Subject (วิชาที่ lock)
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    
    -- Time (เวลาที่ lock)
    day_of_week VARCHAR(3) NOT NULL,
    period_ids JSONB NOT NULL, -- Array of period UUIDs
    
    -- Optional: Room (ถ้าต้องการระบุห้อง)
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
    
    -- Optional: Instructor (ถ้าต้องการระบุครู)
    instructor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Metadata
    reason TEXT, -- เหตุผลที่ lock
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_scope CHECK (
        scope_type IN ('CLASSROOM', 'GRADE_LEVEL', 'ALL_SCHOOL')
    )
);

CREATE INDEX idx_locked_slots_semester ON timetable_locked_slots(academic_semester_id);
CREATE INDEX idx_locked_slots_subject ON timetable_locked_slots(subject_id);
CREATE INDEX idx_locked_slots_day ON timetable_locked_slots(day_of_week);

-- Examples:
-- 1. ทุกห้อง ม.4 เรียนพละวันพุธคาบ 5-6
INSERT INTO timetable_locked_slots (
    academic_semester_id, scope_type, scope_ids,
    subject_id, day_of_week, period_ids, reason
) VALUES (
    'uuid-semester-1-2568',
    'GRADE_LEVEL',
    '["uuid-grade-m4"]'::jsonb,
    'uuid-subject-pe',
    'WED',
    '["uuid-period-5", "uuid-period-6"]'::jsonb,
    'โรงเรียนกำหนดให้ ม.4 ทุกห้องเรียนพละพุธบ่าย'
);

-- 2. ทุกห้องเรียนแนะแนววันศุกร์คาบ 1
INSERT INTO timetable_locked_slots (
    academic_semester_id, scope_type, scope_ids,
    subject_id, day_of_week, period_ids, reason
) VALUES (
    'uuid-semester-1-2568',
    'ALL_SCHOOL',
    null,
    'uuid-subject-guidance',
    'FRI',
    '["uuid-period-1"]'::jsonb,
    'โรงเรียนกำหนดให้ทุกห้องเรียนแนะแนวศุกร์เช้า'
);

-- 3. ห้อง ม.4/1 เฉพาะห้อง เรียนคณิตจันทร์คาบ 1
INSERT INTO timetable_locked_slots (
    academic_semester_id, scope_type, scope_ids,
    subject_id, day_of_week, period_ids,
    instructor_id, reason
) VALUES (
    'uuid-semester-1-2568',
    'CLASSROOM',
    '["uuid-classroom-m4-1"]'::jsonb,
    'uuid-subject-math',
    'MON',
    '["uuid-period-1"]'::jsonb,
    'uuid-teacher-somchai',
    'ผู้อำนวยการกำหนดให้ ม.4/1 เรียนคณิตกับครูสมชายจันทร์เช้าเท่านั้น'
);
```

#### **Implementation:**

```rust
struct LockedSlot {
    id: Uuid,
    scope_type: String, // CLASSROOM, GRADE_LEVEL, ALL_SCHOOL
    scope_ids: Vec<Uuid>,
    subject_id: Uuid,
    day_of_week: String,
    period_ids: Vec<Uuid>,
    room_id: Option<Uuid>,
    instructor_id: Option<Uuid>,
}

async fn get_locked_slots(
    pool: &PgPool,
    semester_id: Uuid,
    classroom_id: Option<Uuid>,
    grade_level_id: Option<Uuid>,
) -> Result<Vec<LockedSlot>, AppError> {
    let locked_slots = sqlx::query_as::<_, LockedSlot>(
        "SELECT * FROM timetable_locked_slots
         WHERE academic_semester_id = $1
           AND (
               scope_type = 'ALL_SCHOOL'
               OR (scope_type = 'GRADE_LEVEL' AND scope_ids @> to_jsonb($2::text))
               OR (scope_type = 'CLASSROOM' AND scope_ids @> to_jsonb($3::text))
           )"
    )
    .bind(semester_id)
    .bind(grade_level_id.map(|id| id.to_string()))
    .bind(classroom_id.map(|id| id.to_string()))
    .fetch_all(pool)
    .await?;
    
    Ok(locked_slots)
}

fn apply_locked_slots(
    schedule: &mut Schedule,
    locked_slots: &[LockedSlot],
) -> Result<(), AppError> {
    for locked in locked_slots {
        // For each classroom in scope
        let classroom_ids = match locked.scope_type.as_str() {
            "ALL_SCHOOL" => schedule.get_all_classroom_ids(),
            "GRADE_LEVEL" => schedule.get_classrooms_by_grade(&locked.scope_ids),
            "CLASSROOM" => locked.scope_ids.clone(),
            _ => vec![],
        };
        
        for classroom_id in classroom_ids {
            // Find the course for this subject in this classroom
            let course = schedule.courses.iter()
                .find(|c| c.classroom_id == classroom_id && c.subject_id == locked.subject_id);
            
            if let Some(course) = course {
                // Pre-assign these periods
                for period_id in &locked.period_ids {
                    let assignment = Assignment {
                        course_id: course.id,
                        time_slot: TimeSlot {
                            day: locked.day_of_week.clone(),
                            period_id: *period_id,
                        },
                        room_id: locked.room_id,
                        is_locked: true, // Mark as locked
                    };
                    
                    schedule.add_assignment(assignment)?;
                }
                
                // Reduce required periods for this course
                course.periods_remaining -= locked.period_ids.len() as i32;
            }
        }
    }
    
    Ok(())
}

// During scheduling, skip locked slots
fn is_slot_locked(
    classroom_id: Uuid,
    day: &str,
    period_id: Uuid,
    locked_slots: &[LockedSlot],
) -> bool {
    locked_slots.iter().any(|locked| {
        // Check if this slot is locked for this classroom
        let is_in_scope = match locked.scope_type.as_str() {
            "ALL_SCHOOL" => true,
            "GRADE_LEVEL" => {
                // Check if classroom belongs to this grade
                locked.scope_ids.contains(&get_grade_level_id(classroom_id))
            },
            "CLASSROOM" => locked.scope_ids.contains(&classroom_id),
            _ => false,
        };
        
        // Check if day and period match
        is_in_scope 
            && locked.day_of_week == day 
            && locked.period_ids.contains(&period_id)
    })
}

// Modified scheduling algorithm
async fn schedule_with_locked_slots(
    pool: &PgPool,
    classroom_ids: &[Uuid],
    semester_id: Uuid,
) -> Result<Schedule, AppError> {
    // 1. Get locked slots
    let locked_slots = get_locked_slots(pool, semester_id, None, None).await?;
    
    // 2. Create initial schedule with locked slots
    let mut schedule = Schedule::new();
    apply_locked_slots(&mut schedule, &locked_slots)?;
    
    // 3. Get remaining courses to schedule
    let remaining_courses = get_remaining_courses(&schedule);
    
    // 4. Schedule remaining courses (avoiding locked slots)
    for course in remaining_courses {
        for slot in available_slots {
            // Skip if slot is locked
            if is_slot_locked(course.classroom_id, &slot.day, slot.period_id, &locked_slots) {
                continue;
            }
            
            // Try to assign
            if can_assign(course, slot, &schedule) {
                schedule.add_assignment(create_assignment(course, slot))?;
            }
        }
    }
    
    Ok(schedule)
}
```

#### **UI: Locking Slots**

```svelte
<!-- Component: LockSlotDialog -->

<script lang="ts">
    let scopeType = $state<'CLASSROOM' | 'GRADE_LEVEL' | 'ALL_SCHOOL'>('GRADE_LEVEL');
    let selectedIds = $state<string[]>([]);
    let selectedSubject = $state<string>('');
    let selectedDay = $state<string>('MON');
    let selectedPeriods = $state<string[]>([]);
    
    async function lockSlot() {
        await apiClient.post('/academic/timetable/lock-slot', {
            scope_type: scopeType,
            scope_ids: scopeType === 'ALL_SCHOOL' ? null : selectedIds,
            subject_id: selectedSubject,
            day_of_week: selectedDay,
            period_ids: selectedPeriods,
            reason: reason
        });
    }
</script>

<Dialog.Root>
    <Dialog.Content class="max-w-2xl">
        <Dialog.Header>
            <Dialog.Title>🔒 Lock ช่วงเวลาเฉพาะ</Dialog.Title>
        </Dialog.Header>
        
        <div class="space-y-4">
            <!-- Scope Selection -->
            <div>
                <Label.Root>ขอบเขต</Label.Root>
                <Select.Root bind:value={scopeType}>
                    <Select.Trigger>
                        <Select.Value />
                    </Select.Trigger>
                    <Select.Content>
                        <Select.Item value="CLASSROOM">ห้องเรียนเฉพาะ</Select.Item>
                        <Select.Item value="GRADE_LEVEL">ทั้งชั้น (เช่น ทุกห้อง ม.4)</Select.Item>
                        <Select.Item value="ALL_SCHOOL">ทั้งโรงเรียน</Select.Item>
                    </Select.Content>
                </Select.Root>
            </div>
            
            <!-- Classroom/Grade Selection (if not ALL_SCHOOL) -->
            {#if scopeType !== 'ALL_SCHOOL'}
                <div>
                    <Label.Root>
                        {scopeType === 'CLASSROOM' ? 'เลือกห้องเรียน' : 'เลือกชั้น'}
                    </Label.Root>
                    <!-- Multi-select component -->
                </div>
            {/if}
            
            <!-- Subject Selection -->
            <div>
                <Label.Root>วิชา</Label.Root>
                <Select.Root bind:value={selectedSubject}>
                    <!-- Subject options -->
                </Select.Root>
            </div>
            
            <!-- Day Selection -->
            <div>
                <Label.Root>วัน</Label.Root>
                <Select.Root bind:value={selectedDay}>
                    <Select.Item value="MON">จันทร์</Select.Item>
                    <Select.Item value="TUE">อังคาร</Select.Item>
                    <!-- ... -->
                </Select.Root>
            </div>
            
            <!-- Period Selection (Multi-select) -->
            <div>
                <Label.Root>คาบ</Label.Root>
                <div class="grid grid-cols-4 gap-2">
                    {#each periods as period}
                        <label class="flex items-center gap-2">
                            <Checkbox
                                checked={selectedPeriods.includes(period.id)}
                                onCheckedChange={(checked) => {
                                    if (checked) {
                                        selectedPeriods = [...selectedPeriods, period.id];
                                    } else {
                                        selectedPeriods = selectedPeriods.filter(p => p !== period.id);
                                    }
                                }}
                            />
                            <span>{period.name}</span>
                        </label>
                    {/each}
                </div>
            </div>
            
            <!-- Reason -->
            <div>
                <Label.Root>เหตุผล</Label.Root>
                <Textarea bind:value={reason} placeholder="เช่น โรงเรียนกำหนด..." />
            </div>
        </div>
        
        <Dialog.Footer>
            <Button onclick={lockSlot}>🔒 Lock ช่วงเวลานี้</Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>

<!-- Locked Slots List -->
<Card>
    <CardHeader>
        <CardTitle>ช่วงเวลาที่ถูก Lock</CardTitle>
    </CardHeader>
    <CardContent>
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead>ขอบเขต</TableHead>
                    <TableHead>วิชา</TableHead>
                    <TableHead>วัน-คาบ</TableHead>
                    <TableHead>เหตุผล</TableHead>
                    <TableHead></TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {#each lockedSlots as slot}
                    <TableRow>
                        <TableCell>{getScopeDisplay(slot)}</TableCell>
                        <TableCell>{slot.subject_name}</TableCell>
                        <TableCell>{slot.day} คาบ {slot.periods.join(', ')}</TableCell>
                        <TableCell class="text-sm text-muted-foreground">{slot.reason}</TableCell>
                        <TableCell>
                            <Button variant="ghost" size="icon" onclick={() => unlockSlot(slot.id)}>
                                <Trash2 class="w-4 h-4" />
                            </Button>
                        </TableCell>
                    </TableRow>
                {/each}
            </TableBody>
        </Table>
    </CardContent>
</Card>
```

#### **Examples:**

```rust
// Example 1: ทุกห้อง ม.4 เรียนพละพุธบ่าย
LockedSlot {
    scope_type: "GRADE_LEVEL",
    scope_ids: vec![m4_grade_id],
    subject_id: pe_subject_id,
    day_of_week: "WED",
    period_ids: vec![period_5_id, period_6_id],
    reason: "โรงเรียนกำหนด",
}

// ผลลัพธ์:
✅ ม.4/1: พุธ 13:00-15:00 พละ
✅ ม.4/2: พุธ 13:00-15:00 พละ
✅ ม.4/3: พุธ 13:00-15:00 พละ
❌ ระบบ Auto จะไม่จัดวิชาอื่นในช่วงนี้สำหรับ ม.4 ทุกห้อง

// Example 2: ทุกห้องเรียนแนะแนวศุกร์เช้า
LockedSlot {
    scope_type: "ALL_SCHOOL",
    scope_ids: vec![],
    subject_id: guidance_subject_id,
    day_of_week: "FRI",
    period_ids: vec![period_1_id],
    reason: "นโยบายโรงเรียน",
}

// ผลลัพถ์:
✅ ทุกห้อง ม.1-ม.6: ศุกร์ คาบ 1 แนะแนว
❌ ระบบจะไม่จัดวิชาอื่นในศุกร์คาบ 1 สำหรับทุกห้อง
```

**Priority**: 🔴 Absolute (highest)
**Enforcement**: Must respect locked slots, cannot override

---

## 📊 Summary: Advanced Constraints

| ID | Constraint | Type | Example | Can Override? |
|----|-----------|------|---------|---------------|
| HC-7 | Consecutive Periods | Hard | พละ 2 คาบติด | ❌ No |
| HC-8 | Fixed Room | Hard | ครู A → ห้อง 201 | ❌ No (if required) |
| HC-9 | Locked Slots | Hard | ม.4 พละพุธบ่าย | ❌ Never |

---

## 🔄 Modified Scheduling Flow

```rust
async fn auto_schedule_with_advanced_constraints(
    pool: &PgPool,
    classroom_ids: &[Uuid],
    semester_id: Uuid,
) -> Result<Schedule, AppError> {
    // 1. Get all constraints
    let consecutive_reqs = get_consecutive_requirements(pool).await?;
    let instructor_rooms = get_instructor_room_assignments(pool).await?;
    let locked_slots = get_locked_slots(pool, semester_id, None, None).await?;
    
    // 2. Create schedule with locked slots first
    let mut schedule = Schedule::new();
    apply_locked_slots(&mut schedule, &locked_slots)?;
    
    // 3. Get remaining courses
    let mut courses = get_remaining_courses(pool, classroom_ids, semester_id, &schedule).await?;
    
    // 4. Sort by difficulty (considering consecutive requirements)
    courses.sort_by_key(|c| {
        let mut difficulty = 0;
        
        // Has consecutive requirement = more difficult
        if let Some(req) = consecutive_reqs.get(&c.subject_id) {
            if req.min_consecutive > 1 {
                difficulty += 100;
            }
        }
        
        // Has fixed room = more difficult
        if instructor_rooms.get(&c.instructor_id).is_some() {
            difficulty += 50;
        }
        
        // More periods = more difficult
        difficulty += c.periods_needed * 10;
        
        -difficulty // Reverse sort
    });
    
    // 5. Schedule each course
    for course in courses {
        // Get consecutive requirement
        let consecutive_req = consecutive_reqs.get(&course.subject_id);
        
        // Get fixed room
        let instructor_room = instructor_rooms.get(&course.instructor_id);
        
        // Try to schedule
        let assignments = schedule_course_with_constraints(
            &course,
            &schedule,
            consecutive_req,
            instructor_room,
            &locked_slots,
        )?;
        
        schedule.add_assignments(assignments)?;
    }
    
    // 6. Validate
    validate_schedule(&schedule)?;
    
    Ok(schedule)
}
```

---

## 🎯 Complete Example

```rust
// โรงเรียนมีกำหนด:
// 1. ทุกห้อง ม.4 เรียนพละวันพุธคาบ 5-6 (LOCKED)
// 2. ครูสมชาย (คณิต) สอนห้อง 201 ประจำ (FIXED ROOM)
// 3. ปฏิบัติการเคมี ต้อง 2 คาบติด (CONSECUTIVE)

let config = SchedulingConfig {
    locked_slots: vec![
        LockedSlot {
            scope_type: "GRADE_LEVEL",
            scope_ids: vec![m4_grade_id],
            subject: "PE",
            day: "WED",
            periods: vec![5, 6],
        }
    ],
    instructor_rooms: vec![
        InstructorRoom {
            instructor: "ครูสมชาย",
            room: "201",
            required: true,
        }
    ],
    consecutive_requirements: vec![
        ConsecutiveReq {
            subject: "เคมี LAB",
            min: 2,
            max: 2,
            force: true,
        }
    ],
};

let result = auto_schedule(classroom_ids, semester_id, config).await?;

// ผลลัพธ์ ม.4/1:
✅ จันทร์ 08:00 คณิต (ครูสมชาย ห้อง 201) ← Fixed room
✅ อังคาร 10:00-11:40 เคมี LAB (2 คาบติด) ← Consecutive
✅ พุธ 13:00-15:00 พละ (LOCKED) ← Pre-assigned
✅ พฤหัส 09:00 คณิต (ครูสมชาย ห้อง 201) ← Fixed room
✅ ...etc
```

---

**เอกสารนี้เพิ่มเติมจากเอกสารหลัก**: `timetable_constraints_specification.md`

**Version**: 1.1
**Last Updated**: 2026-02-08
**Author**: SchoolOrbit Development Team
