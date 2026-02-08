# 📋 Auto Timetable Scheduling - Constraints Specification
## ข้อกำหนดและเงื่อนไขการจัดตารางสอนอัตโนมัติ

> **วัตถุประสงค์**: กำหนด Rules และ Constraints ทั้งหมดที่ระบบจัดตารางต้องปฏิบัติตาม
> 
> แบ่งเป็น 2 ประเภท:
> - **Hard Constraints**: ห้ามละเมิด (ถ้าละเมิด = ตารางไม่ valid)
> - **Soft Constraints**: ควรปฏิบัติตาม (ถ้าละเมิดได้ แต่จะลดคะแนนคุณภาพ)

---

## 🔴 Hard Constraints (ห้ามละเมิดเด็ดขาด)

### **HC-1: Classroom Conflict Prevention**
**กฎ**: ห้องเรียนหนึ่งห้อง ห้ามมีวิชาซ้อนกันในคาบเดียวกัน

```
❌ ผิด:
จันทร์ คาบ 1:
  - ห้อง ม.4/1: คณิตศาสตร์ 
  - ห้อง ม.4/1: ฟิสิกส์  ← ชนกัน!

✅ ถูก:
จันทร์ คาบ 1: ห้อง ม.4/1: คณิตศาสตร์
จันทร์ คาบ 2: ห้อง ม.4/1: ฟิสิกส์
```

**Implementation:**
```rust
fn check_classroom_conflict(
    classroom_id: Uuid,
    day: &str,
    period_id: Uuid,
    occupied: &HashMap<String, CourseInfo>
) -> bool {
    let key = format!("classroom_{}_{day}_{period_id}", classroom_id);
    occupied.contains_key(&key)
}
```

**Priority**: 🔴 Critical
**Penalty if violated**: Infinite (ตารางไม่ valid)

---

### **HC-2: Instructor Conflict Prevention**
**กฎ**: ครูหนึ่งคน ห้ามสอนหลายห้องในคาบเดียวกัน

```
❌ ผิด:
จันทร์ คาบ 1:
  - ม.4/1: คณิต (ครูสมชาย)
  - ม.5/1: คณิต (ครูสมชาย) ← ชนกัน!

✅ ถูก:
จันทร์ คาบ 1: ม.4/1: คณิต (ครูสมชาย)
จันทร์ คาบ 2: ม.5/1: คณิต (ครูสมชาย)
```

**Implementation:**
```rust
fn check_instructor_conflict(
    instructor_id: Uuid,
    day: &str,
    period_id: Uuid,
    occupied: &HashMap<String, CourseInfo>
) -> bool {
    let key = format!("instructor_{}_{day}_{period_id}", instructor_id);
    occupied.contains_key(&key)
}
```

**Priority**: 🔴 Critical
**Penalty if violated**: Infinite

---

### **HC-3: Room Availability**
**กฎ**: ห้องพิเศษ (LAB, สนาม, ห้องคอม) ห้ามใช้ซ้อนกันในคาบเดียวกัน

```
❌ ผิด:
จันทร์ คาบ 3:
  - ม.4/1: ฟิสิกส์ (ห้อง LAB-1)
  - ม.5/1: เคมี (ห้อง LAB-1) ← ชนกัน!

✅ ถูก:
จันทร์ คาบ 3: ม.4/1: ฟิสิกส์ (LAB-1)
จันทร์ คาบ 4: ม.5/1: เคมี (LAB-1)
```

**Implementation:**
```rust
fn check_room_conflict(
    room_id: Uuid,
    day: &str,
    period_id: Uuid,
    occupied: &HashMap<String, RoomUsage>
) -> bool {
    let key = format!("room_{}_{day}_{period_id}", room_id);
    occupied.contains_key(&key)
}
```

**Special Case**: 
- ถ้าวิชาไม่ระบุห้องพิเศษ (ใช้ห้องประจำชั้น) → ไม่ต้องเช็ค
- ถ้าระบุห้องพิเศษ → ต้องเช็ค

**Priority**: 🔴 Critical
**Penalty if violated**: Infinite

---

### **HC-4: Period Requirements**
**กฎ**: วิชาต้องได้รับจำนวนคาบที่กำหนดไว้ (ตาม subject_hours หรือ credit)

```
❌ ผิด:
คณิตศาสตร์ (กำหนด 4 คาบ/สัปดาห์)
→ จัดได้แค่ 3 คาบ ← ไม่ครบ!

✅ ถูก:
คณิตศาสตร์ (กำหนด 4 คาบ/สัปดาห์)
→ จัดได้ 4 คาบ ✓
```

**Calculation:**
```rust
fn calculate_required_periods(subject: &Subject) -> i32 {
    // Priority: periods_per_week > hours > credit
    if subject.periods_per_week > 0 {
        return subject.periods_per_week;
    }
    
    if subject.hours > 0 {
        // สมมติ 1 ภาคเรียน = 20 สัปดาห์
        return (subject.hours as f32 / 20.0).ceil() as i32;
    }
    
    if subject.credit > 0.0 {
        // 1 หน่วยกิต ≈ 2 คาบ/สัปดาห์
        return (subject.credit * 2.0).ceil() as i32;
    }
    
    // Default
    return 2;
}
```

**Priority**: 🔴 Critical
**Handling**: 
- ถ้าจัดไม่ครบ → Report as "Failed to schedule"
- อนุโลมได้ถ้า user เลือก "force_partial" mode

---

### **HC-5: Valid Time Slot**
**กฎ**: คาบเรียนต้องอยู่ในช่วงเวลาที่กำหนด (MON-FRI, คาบ 1-8)

```
❌ ผิด:
วันเสาร์ คาบ 1: คณิต ← ไม่มีเรียนวันเสาร์!

✅ ถูก:
วันจันทร์ คาบ 1: คณิต
```

**Implementation:**
```rust
fn is_valid_time_slot(day: &str, period_id: Uuid, periods: &[Period]) -> bool {
    // Check day is in allowed list
    const ALLOWED_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI"];
    if !ALLOWED_DAYS.contains(&day) {
        return false;
    }
    
    // Check period exists and is active
    periods.iter().any(|p| p.id == period_id && p.is_active)
}
```

**Priority**: 🔴 Critical

---

### **HC-6: Instructor Unavailability (Hard)**
**กฎ**: ครูต้องไม่สอนในช่วงที่ระบุว่า "ไม่สะดวก" (hard unavailable)

```
❌ ผิด:
ครูสมชาย: ระบุไม่สะดวกวันพุธคาบ 7-8 (ไปประชุม)
→ ระบบจัด: พุธ คาบ 7 สอน ม.4/1 ← ผิด!

✅ ถูก:
ครูสมชาย: ระบุไม่สะดวกวันพุธคาบ 7-8
→ ระบบจัด: พุธ คาบ 7 ว่าง ✓
```

**Implementation:**
```rust
fn check_instructor_unavailable(
    instructor_id: Uuid,
    day: &str,
    period_id: Uuid,
    preferences: &InstructorPreferences
) -> bool {
    preferences.hard_unavailable_slots.iter().any(|slot| {
        slot.day == day && slot.period_id == period_id
    })
}
```

**Priority**: 🔴 Critical
**Note**: แยกจาก Soft Preference (preferred slots)

---

## 🟡 Soft Constraints (ควรปฏิบัติตาม แต่อนุโลมได้)

### **SC-1: Subject Distribution**
**กฎ**: วิชาเดียวกันควรกระจายตลอดสัปดาห์ ไม่อยู่ติดกันทุกวัน

```
⚠️ ไม่ดี (แต่ valid):
คณิต: จันทร์ อังคาร พุธ พฤหัส (4 วันติด) 
→ Quality Score: 50/100

✅ ดี:
คณิต: จันทร์ พุธ ศุกร์ + อังคาร
→ Quality Score: 90/100
```

**Scoring:**
```rust
fn calculate_distribution_score(assignments: &[Assignment]) -> f64 {
    let mut score = 100.0;
    
    // Group by course
    for (course_id, slots) in group_by_course(assignments) {
        // Check consecutive days
        let days: Vec<_> = slots.iter().map(|s| day_to_number(&s.day)).collect();
        let max_consecutive = find_max_consecutive(&days);
        
        // Penalty for too many consecutive days
        if max_consecutive >= 4 {
            score -= 30.0; // Very bad
        } else if max_consecutive == 3 {
            score -= 15.0; // OK but not great
        }
        
        // Bonus for well-distributed
        if is_well_distributed(&days) {
            score += 10.0;
        }
    }
    
    score
}
```

**Weight**: 30%
**Priority**: 🟡 High

---

### **SC-2: Consecutive Period Limit**
**กฎ**: วิชาเดียวกันไม่ควรอยู่ติดกันเกิน 2-3 คาบ (ยกเว้นวิชาพิเศษ เช่น พละ, ปฏิบัติการ)

```
⚠️ ไม่ดี:
จันทร์ คาบ 1-4: คณิต (4 คาบติด) ← เบื่อ!

✅ ดี:
จันทร์ คาบ 1-2: คณิต (2 คาบ)
จันทร์ คาบ 5-6: วิทย์ LAB (2 คาบติด - OK เพราะเป็น LAB)
```

**Configuration:**
```rust
struct SubjectConstraint {
    subject_id: Uuid,
    min_consecutive: i32, // Default: 1
    max_consecutive: i32, // Default: 2
}

// Special cases:
// - พละ: min=2, max=2 (ต้อง 2 คาบติด)
// - ปฏิบัติการวิทย์: min=2, max=3
// - วิชาทั่วไป: min=1, max=2
```

**Scoring:**
```rust
fn check_consecutive_periods(
    course_id: Uuid,
    day: &str,
    period_ids: &[Uuid],
    constraint: &SubjectConstraint
) -> f64 {
    let consecutive_count = count_consecutive_periods(period_ids);
    
    if consecutive_count < constraint.min_consecutive {
        return 0.0; // Too few
    }
    if consecutive_count > constraint.max_consecutive {
        return 50.0; // Too many (penalty)
    }
    
    100.0 // Perfect
}
```

**Weight**: 20%
**Priority**: 🟡 High

---

### **SC-3: Time of Day Preference**
**กฎ**: บางวิชาควรอยู่ในช่วงเวลาที่เหมาะสม

```
✅ ดี:
- คณิต, วิทย์: คาบเช้า (1-4) → นักเรียนสมองดี
- พละ: คาบบ่าย (5-7) → ไม่ร้อนเกินไป, ไม่ชนตลาด
- ศิลปะ, ดนตรี: คาบบ่าย → ผ่อนคลาย

⚠️ ไม่ดี:
- พละ: คาบ 6-7 (เที่ยง-บ่าย 2) → ร้อนมาก!
- คณิต: คาบ 8 (บ่าย 3) → นักเรียนเหนื่อย
```

**Configuration:**
```rust
enum TimeOfDay {
    Morning,    // คาบ 1-4 (08:00-12:00)
    Afternoon,  // คาบ 5-8 (13:00-16:00)
    Anytime,
}

struct SubjectTimePreference {
    subject_type: String, // "CORE", "ELECTIVE", "ACTIVITY", "PE"
    preferred_time: TimeOfDay,
    avoid_time: Option<TimeOfDay>,
}

// Examples:
// - CORE (คณิต, ไทย, วิทย์): preferred=Morning
// - PE (พละ): preferred=Afternoon, avoid=คาบ 6-7 (ร้อนสุด)
// - ACTIVITY: preferred=Afternoon
```

**Scoring:**
```rust
fn time_of_day_score(
    subject: &Subject,
    period: &Period,
    preference: &SubjectTimePreference
) -> f64 {
    let time = classify_time_of_day(&period.start_time);
    
    match preference.preferred_time {
        Morning if time == Morning => 100.0,
        Afternoon if time == Afternoon => 100.0,
        Anytime => 80.0,
        _ => 60.0, // Not preferred but allowed
    }
}
```

**Weight**: 15%
**Priority**: 🟡 Medium

---

### **SC-4: Instructor Preference (Soft)**
**กฎ**: ครูต้องการสอนในช่วงเวลาที่ระบุ (preferred slots)

```
ครูสมชาย: ชอบสอนช่วงเช้า (คาบ 1-4)

✅ ดี: จัดให้ครูสมชายสอน 80% ช่วงเช้า
⚠️ OK: จัดช่วงบ่ายบางวัน (ยอมรับได้)
```

**Implementation:**
```rust
fn instructor_preference_score(
    instructor_id: Uuid,
    assignments: &[Assignment],
    preferences: &InstructorPreferences
) -> f64 {
    let total = assignments.len() as f64;
    let mut satisfied = 0.0;
    
    for assignment in assignments {
        if is_in_preferred_slots(&assignment.time_slot, preferences) {
            satisfied += 1.0;
        }
    }
    
    // % ที่ตรงความต้องการ
    (satisfied / total) * 100.0
}
```

**Weight**: 15%
**Priority**: 🟡 Medium

---

### **SC-5: Daily Load Balance**
**กฎ**: นักเรียนไม่ควรมีคาบเรียนหนักเกินไปในวันเดียว

```
⚠️ ไม่ดี:
จันทร์: เต็ม 8 คาบ
อังคาร: 3 คาบ
พุธ: 8 คาบ
พฤหัส: 2 คาบ
ศุกร์: 7 คาบ

✅ ดี:
จันทร์: 6 คาบ
อังคาร: 5 คาบ
พุธ: 6 คาบ
พฤหัส: 6 คาบ
ศุกร์: 5 คาบ
→ สมดุลกระจาย!
```

**Scoring:**
```rust
fn daily_load_balance_score(classroom_id: Uuid, assignments: &[Assignment]) -> f64 {
    let daily_counts = count_by_day(assignments);
    
    // Calculate variance (ยิ่งน้อยยิ่งดี)
    let mean = daily_counts.values().sum::<i32>() as f64 / 5.0;
    let variance: f64 = daily_counts.values()
        .map(|&count| {
            let diff = count as f64 - mean;
            diff * diff
        })
        .sum::<f64>() / 5.0;
    
    // Convert to score (0-100)
    // Low variance = high score
    100.0 - (variance.sqrt() * 10.0).min(100.0)
}
```

**Weight**: 10%
**Priority**: 🟡 Medium

---

### **SC-6: Instructor Daily Load Limit**
**กฎ**: ครูไม่ควรสอนเกิน 6-7 คาบต่อวัน

```
⚠️ ไม่ดี:
ครูสมชาย:
จันทร์: 8 คาบ ← เหนื่อยมาก!

✅ ดี:
ครูสมชาย:
จันทร์: 5 คาบ
อังคาร: 6 คาบ
```

**Implementation:**
```rust
fn check_instructor_daily_load(
    instructor_id: Uuid,
    day: &str,
    assignments: &[Assignment],
    max_periods: i32
) -> f64 {
    let count = count_instructor_periods(instructor_id, day, assignments);
    
    if count > max_periods {
        return 0.0; // Violation!
    }
    if count > max_periods - 1 {
        return 70.0; // Almost too much
    }
    
    100.0 // OK
}
```

**Weight**: 10%
**Priority**: 🟡 Low-Medium

---

### **SC-7: Avoid First/Last Period for Special Subjects**
**กฎ**: วิชาพิเศษ (เช่น ปฏิบัติการ) ไม่ควรอยู่คาบแรก/คาบสุดท้าย

```
⚠️ ไม่ดี:
คาบ 1: เคมี LAB ← อุปกรณ์ยังไม่พร้อม, เด็กมาสาย
คาบ 8: ฟิสิกส์ LAB ← ต้องเก็บของเร็ว, ทำไม่ทัน

✅ ดี:
คาบ 3-4: เคมี LAB
คาบ 5-6: ฟิสิกส์ LAB
```

**Configuration:**
```rust
struct SpecialSubjectRules {
    avoid_first_period: bool,
    avoid_last_period: bool,
}

// LAB subjects: avoid both first and last
// Regular subjects: no restriction
```

**Weight**: 5%
**Priority**: 🟢 Low

---

### **SC-8: Same Subject Spacing**
**กฎ**: วิชาเดียวกันควรห่างกันอย่างน้อย 1 วัน

```
⚠️ ไม่ดี:
จันทร์ คาบ 3: คณิต
อังคาร คาบ 1: คณิต ← ติดกัน 2 วันติด
อังคาร คาบ 5: คณิต ← วันเดียวกัน 2 คาบ (OK ถ้าไม่ติดกัน)

✅ ดี:
จันทร์ คาบ 3: คณิต
พุธ คาบ 2: คณิต
ศุกร์ คาบ 1: คณิต
→ กระจาย จันทร์-พุธ-ศุกร์
```

**Implementation:**
```rust
fn check_subject_spacing(subject_id: Uuid, assignments: &[Assignment]) -> f64 {
    let days = get_days_for_subject(subject_id, assignments);
    
    let mut min_gap = 7;
    for i in 1..days.len() {
        let gap = days[i] - days[i-1];
        min_gap = min_gap.min(gap);
    }
    
    match min_gap {
        0 => 50.0,  // Same day (allowed if not consecutive periods)
        1 => 70.0,  // Next day (not ideal)
        2..=3 => 100.0, // Perfect spacing
        _ => 90.0,  // Spaced out (OK)
    }
}
```

**Weight**: 5%
**Priority**: 🟢 Low

---

## 📊 Quality Scoring System

### **Overall Quality Score Formula:**

```rust
fn calculate_overall_quality(schedule: &Schedule) -> f64 {
    let weights = [
        (30.0, calculate_distribution_score(schedule)),        // SC-1
        (20.0, calculate_consecutive_score(schedule)),         // SC-2
        (15.0, calculate_time_of_day_score(schedule)),        // SC-3
        (15.0, calculate_instructor_preference_score(schedule)), // SC-4
        (10.0, calculate_daily_load_balance_score(schedule)), // SC-5
        (5.0,  calculate_instructor_load_score(schedule)),    // SC-6
        (3.0,  calculate_avoid_edge_periods_score(schedule)), // SC-7
        (2.0,  calculate_subject_spacing_score(schedule)),    // SC-8
    ];
    
    let weighted_sum: f64 = weights.iter()
        .map(|(weight, score)| weight * score)
        .sum();
    
    let total_weight: f64 = weights.iter().map(|(w, _)| w).sum();
    
    weighted_sum / total_weight
}
```

### **Quality Levels:**

| Score | Level | Description |
|-------|-------|-------------|
| 90-100 | 🟢 Excellent | ตารางดีมาก แนะนำให้ใช้เลย |
| 80-89 | 🟢 Good | ตารางดี ใช้ได้ |
| 70-79 | 🟡 Acceptable | พอใช้ได้ อาจต้องปรับเล็กน้อย |
| 60-69 | 🟡 Fair | ใช้ได้แต่ไม่ดีนัก ควรปรับปรุง |
| < 60 | 🔴 Poor | ไม่แนะนำให้ใช้ ควร regenerate |

---

## 🎛️ Configuration Options

### **User Configurable Settings:**

```rust
pub struct SchedulingConfig {
    // Algorithm
    pub algorithm: Algorithm, // GREEDY, BACKTRACKING, HYBRID
    pub max_iterations: u32,  // For backtracking
    pub timeout_seconds: u32, // Stop if takes too long
    
    // Hard Constraints
    pub enforce_period_requirements: bool, // Default: true
    pub enforce_instructor_unavailability: bool, // Default: true
    
    // Soft Constraints
    pub optimize_distribution: bool,        // SC-1, default: true
    pub optimize_consecutive_limit: bool,   // SC-2, default: true
    pub optimize_time_of_day: bool,        // SC-3, default: true
    pub respect_preferences: bool,          // SC-4, default: true
    pub balance_daily_load: bool,          // SC-5, default: true
    
    // Special Options
    pub force_overwrite: bool,   // Delete existing timetable
    pub allow_partial: bool,     // Allow incomplete schedule (not all periods)
    pub min_quality_score: f64,  // Reject if score < this (default: 70.0)
    
    // Priority Weights (Custom)
    pub weight_distribution: Option<f64>,   // Override default 30%
    pub weight_consecutive: Option<f64>,    // Override default 20%
    // ... etc
}
```

### **Example Configurations:**

```rust
// Fast mode (for testing)
SchedulingConfig {
    algorithm: Algorithm::GREEDY,
    max_iterations: 100,
    timeout_seconds: 30,
    optimize_distribution: true,
    optimize_consecutive_limit: false, // Skip to save time
    optimize_time_of_day: false,       // Skip
    ..Default::default()
}

// Best Quality mode (production)
SchedulingConfig {
    algorithm: Algorithm::BACKTRACKING,
    max_iterations: 10000,
    timeout_seconds: 300,
    optimize_distribution: true,
    optimize_consecutive_limit: true,
    optimize_time_of_day: true,
    respect_preferences: true,
    balance_daily_load: true,
    min_quality_score: 80.0, // High standard
    ..Default::default()
}

// Strict mode (no compromises)
SchedulingConfig {
    algorithm: Algorithm::BACKTRACKING,
    enforce_period_requirements: true,
    allow_partial: false, // Must schedule ALL courses
    min_quality_score: 90.0, // Very high standard
    ..Default::default()
}
```

---

## 🧪 Validation & Testing

### **Pre-Scheduling Validation:**

```rust
async fn validate_before_scheduling(
    pool: &PgPool,
    classroom_ids: &[Uuid],
    semester_id: Uuid
) -> Result<ValidationReport, AppError> {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    
    // 1. Check if courses exist
    let courses = get_courses(pool, classroom_ids, semester_id).await?;
    if courses.is_empty() {
        errors.push("No courses to schedule".to_string());
    }
    
    // 2. Check if periods exist
    let periods = get_periods(pool).await?;
    if periods.is_empty() {
        errors.push("No time periods defined".to_string());
    }
    
    // 3. Check for missing instructors
    for course in &courses {
        if course.instructor_id.is_none() {
            warnings.push(format!("Course {} has no instructor", course.subject_code));
        }
    }
    
    // 4. Check total periods required vs available
    let total_required: i32 = courses.iter()
        .map(|c| calculate_required_periods(c))
        .sum();
    let total_available = periods.len() as i32 * 5; // 5 days
    
    if total_required > total_available {
        warnings.push(format!(
            "Required {} periods but only {} available. Some courses may not be scheduled.",
            total_required, total_available
        ));
    }
    
    // 5. Check for special room requirements
    for course in &courses {
        if let Some(room_type) = &course.required_room_type {
            let available_rooms = count_rooms_by_type(pool, room_type).await?;
            if available_rooms == 0 {
                warnings.push(format!(
                    "Course {} requires {} but no rooms available",
                    course.subject_code, room_type
                ));
            }
        }
    }
    
    Ok(ValidationReport { warnings, errors })
}
```

### **Post-Scheduling Validation:**

```rust
async fn validate_schedule(
    schedule: &Schedule
) -> Result<ValidationReport, AppError> {
    let mut violations = Vec::new();
    
    // Check all hard constraints
    for assignment in &schedule.assignments {
        // HC-1: Classroom conflict
        if has_classroom_conflict(assignment, &schedule.assignments) {
            violations.push(format!(
                "Classroom conflict at {} {}",
                assignment.day, assignment.period_id
            ));
        }
        
        // HC-2: Instructor conflict
        if has_instructor_conflict(assignment, &schedule.assignments) {
            violations.push(format!(
                "Instructor conflict at {} {}",
                assignment.day, assignment.period_id
            ));
        }
        
        // HC-3: Room conflict
        if has_room_conflict(assignment, &schedule.assignments) {
            violations.push(format!(
                "Room conflict at {} {}",
                assignment.day, assignment.period_id
            ));
        }
    }
    
    // Check period requirements
    for course in &schedule.courses {
        let assigned = count_assigned_periods(course.id, &schedule.assignments);
        let required = calculate_required_periods(course);
        if assigned < required {
            violations.push(format!(
                "Course {} only got {}/{} periods",
                course.subject_code, assigned, required
            ));
        }
    }
    
    Ok(ValidationReport {
        violations,
        quality_score: calculate_overall_quality(schedule),
    })
}
```

---

## 📋 Summary Table

| ID | Constraint | Type | Weight | Can Violate? |
|----|-----------|------|--------|--------------|
| HC-1 | Classroom Conflict | Hard | ∞ | ❌ Never |
| HC-2 | Instructor Conflict | Hard | ∞ | ❌ Never |
| HC-3 | Room Availability | Hard | ∞ | ❌ Never |
| HC-4 | Period Requirements | Hard | ∞ | ⚠️ Optional |
| HC-5 | Valid Time Slot | Hard | ∞ | ❌ Never |
| HC-6 | Instructor Unavailability | Hard | ∞ | ❌ Never |
| SC-1 | Subject Distribution | Soft | 30% | ✅ Yes |
| SC-2 | Consecutive Limit | Soft | 20% | ✅ Yes |
| SC-3 | Time of Day | Soft | 15% | ✅ Yes |
| SC-4 | Instructor Preference | Soft | 15% | ✅ Yes |
| SC-5 | Daily Load Balance | Soft | 10% | ✅ Yes |
| SC-6 | Instructor Daily Load | Soft | 5% | ✅ Yes |
| SC-7 | Avoid Edge Periods | Soft | 3% | ✅ Yes |
| SC-8 | Subject Spacing | Soft | 2% | ✅ Yes |

**Total Soft Constraints Weight**: 100%

---

## 🚀 Next Steps

1. **Implement Hard Constraints** (Day 1-2)
   - Database schema ready
   - Validation functions

2. **Implement Soft Constraints** (Day 3-5)
   - Scoring system
   - Quality calculator

3. **Build Algorithm** (Day 6-9)
   - Greedy baseline
   - Backtracking optimizer
   - Configuration system

4. **Testing** (Day 10-12)
   - Unit tests for each constraint
   - Integration tests with real data
   - Performance benchmarks

---

**Version**: 1.0
**Last Updated**: 2026-02-08
**Author**: SchoolOrbit Development Team
