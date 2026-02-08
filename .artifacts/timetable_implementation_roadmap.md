# 🚀 Auto Timetable Scheduling - Implementation Roadmap
## Hybrid Approach (9-12 วัน)

> **กลยุทธ์**: Build MVP First, Then Enhance
> 
> สร้างระบบพื้นฐานที่ใช้งานได้ก่อน (4 วัน) แล้วค่อยปรับปรุงเป็น Full Version (5 วัน)
> ลด risk และได้ feedback เร็วขึ้น

---

## 📅 Timeline Overview

```
Week 1 (Day 1-5):   Foundation + MVP
Week 2 (Day 6-9):   Enhancement to Full
Week 2 (Day 10-12): Testing + Polish
```

---

## Week 1: Foundation + MVP

### **Day 1-2: Database & Models** 🗄️

**Goal**: วางโครงสร้างให้รองรับ Full Version ตั้งแต่แรก

#### ✅ Tasks:

**1. Migration Files**
```bash
migrations/039_scheduling_system.sql
```

```sql
-- Table 1: Instructor Preferences (สำหรับ Phase 2 แต่สร้างไว้เลย)
CREATE TABLE instructor_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id),
    unavailable_slots JSONB DEFAULT '[]'::jsonb, -- [{day: "MON", period_id: "..."}]
    preferred_slots JSONB DEFAULT '[]'::jsonb,
    max_periods_per_day INTEGER DEFAULT 8,
    max_consecutive_periods INTEGER DEFAULT 4,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(instructor_id, academic_year_id)
);

-- Table 2: Subject Constraints
CREATE TABLE subject_constraints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    preferred_time_of_day VARCHAR(20), -- 'MORNING', 'AFTERNOON', 'ANYTIME'
    required_room_type VARCHAR(50),
    min_gap_between_sessions INTEGER DEFAULT 0, -- วิชาควรห่างกันกี่วัน
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(subject_id)
);

-- Table 3: Scheduling Jobs (Track progress)
CREATE TABLE timetable_scheduling_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scope VARCHAR(20) NOT NULL, -- 'SINGLE', 'MULTIPLE', 'GRADE', 'ALL'
    classroom_ids JSONB, -- array of UUIDs
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id),
    
    -- Configuration
    algorithm VARCHAR(20) DEFAULT 'GREEDY', -- 'GREEDY', 'BACKTRACKING', 'HYBRID'
    force_overwrite BOOLEAN DEFAULT false,
    respect_preferences BOOLEAN DEFAULT true,
    
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING', -- PENDING, RUNNING, COMPLETED, FAILED
    progress INTEGER DEFAULT 0,
    total_courses INTEGER DEFAULT 0,
    scheduled_courses INTEGER DEFAULT 0,
    
    -- Results
    result_summary JSONB, -- {success: 35, failed: 2, warnings: [...]}
    error_message TEXT,
    
    -- Timing
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Table 4: Add metadata to subjects (if not exists)
ALTER TABLE subjects 
ADD COLUMN IF NOT EXISTS periods_per_week INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS allow_split BOOLEAN DEFAULT true;
```

**2. Rust Models**
```rust
// backend-school/src/modules/academic/models/scheduling.rs

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct InstructorPreference {
    pub id: Uuid,
    pub instructor_id: Uuid,
    pub academic_year_id: Uuid,
    pub unavailable_slots: serde_json::Value,
    pub preferred_slots: serde_json::Value,
    pub max_periods_per_day: i32,
    pub max_consecutive_periods: i32,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SchedulingJob {
    pub id: Uuid,
    pub scope: String,
    pub classroom_ids: Option<serde_json::Value>,
    pub academic_semester_id: Uuid,
    pub algorithm: String,
    pub status: String,
    pub progress: i32,
    pub total_courses: i32,
    pub scheduled_courses: i32,
    // ... other fields
}

#[derive(Debug, Deserialize)]
pub struct AutoScheduleRequest {
    pub classroom_ids: Vec<Uuid>,
    pub semester_id: Uuid,
    pub algorithm: Option<String>, // "greedy" or "backtracking"
    pub force: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SchedulingResult {
    pub job_id: Uuid,
    pub total: usize,
    pub scheduled: usize,
    pub failed: usize,
    pub warnings: Vec<String>,
}
```

**Time**: 2 days
**Deliverable**: Database ready, Models created

---

### **Day 3-4: Greedy Algorithm (MVP)** ⚡

**Goal**: ระบบที่ใช้งานได้ จัดตารางเบ็ดเสร็จ

#### ✅ Tasks:

**1. Core Algorithm**
```rust
// backend-school/src/modules/academic/services/scheduler.rs

use std::collections::{HashMap, HashSet};

pub struct Scheduler {
    pool: PgPool,
}

#[derive(Clone)]
struct CourseToSchedule {
    id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    subject_code: String,
    subject_name: String,
    instructor_id: Option<Uuid>,
    periods_needed: i32,
    required_room_type: Option<String>,
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct TimeSlot {
    day: String, // "MON", "TUE", etc.
    period_id: Uuid,
}

struct Assignment {
    course_id: Uuid,
    time_slot: TimeSlot,
    room_id: Option<Uuid>,
}

impl Scheduler {
    /// Main entry point - Greedy Algorithm
    pub async fn schedule_greedy(
        &self,
        classroom_ids: &[Uuid],
        semester_id: Uuid,
        force: bool,
    ) -> Result<SchedulingResult, AppError> {
        // 1. Create job
        let job_id = self.create_job(classroom_ids, semester_id, "GREEDY").await?;
        
        // 2. Clear existing if force
        if force {
            self.clear_existing_timetable(classroom_ids, semester_id).await?;
        }
        
        // 3. Get all courses to schedule
        let courses = self.get_courses_to_schedule(classroom_ids, semester_id).await?;
        
        // 4. Get available time slots
        let time_slots = self.get_available_time_slots(semester_id).await?;
        
        // 5. Sort courses by difficulty (most constrained first)
        let sorted_courses = self.sort_by_difficulty(courses);
        
        // 6. Schedule each course
        let mut assignments = Vec::new();
        let mut occupied = HashMap::new();
        let mut failed = Vec::new();
        
        for course in sorted_courses {
            match self.schedule_course(
                &course,
                &time_slots,
                &mut occupied,
            ).await {
                Ok(course_assignments) => {
                    assignments.extend(course_assignments);
                }
                Err(e) => {
                    failed.push(course.subject_code.clone());
                    eprintln!("Failed to schedule {}: {}", course.subject_code, e);
                }
            }
        }
        
        // 7. Save assignments to database
        self.save_assignments(&assignments).await?;
        
        // 8. Update job
        self.complete_job(job_id, assignments.len(), failed.len()).await?;
        
        Ok(SchedulingResult {
            job_id,
            total: courses.len(),
            scheduled: assignments.len(),
            failed: failed.len(),
            warnings: failed,
        })
    }
    
    /// Schedule a single course (find slots for all periods)
    async fn schedule_course(
        &self,
        course: &CourseToSchedule,
        time_slots: &[TimeSlot],
        occupied: &mut HashMap<String, CourseInfo>,
    ) -> Result<Vec<Assignment>, AppError> {
        let mut assignments = Vec::new();
        let periods_needed = course.periods_needed;
        
        'outer: for _ in 0..periods_needed {
            // Try each time slot
            for slot in time_slots {
                if self.is_slot_available(course, slot, occupied).await? {
                    // Assign this slot
                    let assignment = Assignment {
                        course_id: course.id,
                        time_slot: slot.clone(),
                        room_id: None, // TODO: room assignment
                    };
                    
                    // Mark as occupied
                    self.mark_occupied(occupied, course, slot);
                    
                    assignments.push(assignment);
                    continue 'outer;
                }
            }
            
            // Could not find slot for this period
            return Err(AppError::BadRequest(
                format!("Cannot find slot for {}", course.subject_code)
            ));
        }
        
        Ok(assignments)
    }
    
    /// Check if a time slot is available for this course
    async fn is_slot_available(
        &self,
        course: &CourseToSchedule,
        slot: &TimeSlot,
        occupied: &HashMap<String, CourseInfo>,
    ) -> Result<bool, AppError> {
        // 1. Check classroom conflict
        let classroom_key = format!("classroom_{}_{}", course.classroom_id, slot.key());
        if occupied.contains_key(&classroom_key) {
            return Ok(false);
        }
        
        // 2. Check instructor conflict
        if let Some(instructor_id) = course.instructor_id {
            let instructor_key = format!("instructor_{}_{}", instructor_id, slot.key());
            if occupied.contains_key(&instructor_key) {
                return Ok(false);
            }
        }
        
        // All checks passed
        Ok(true)
    }
    
    /// Sort courses by difficulty (heuristic)
    fn sort_by_difficulty(&self, mut courses: Vec<CourseToSchedule>) -> Vec<CourseToSchedule> {
        courses.sort_by_key(|c| {
            // Higher score = more difficult = schedule first
            let mut difficulty = 0;
            
            // More periods = more difficult
            difficulty += c.periods_needed * 10;
            
            // Has special room requirement = more difficult
            if c.required_room_type.is_some() {
                difficulty += 50;
            }
            
            // TODO: Check how many other classrooms this instructor teaches
            // More classrooms = more conflicts = more difficult
            
            -difficulty // Reverse (highest first)
        });
        
        courses
    }
    
    /// Mark a slot as occupied
    fn mark_occupied(
        &self,
        occupied: &mut HashMap<String, CourseInfo>,
        course: &CourseToSchedule,
        slot: &TimeSlot,
    ) {
        // Mark classroom occupied
        let classroom_key = format!("classroom_{}_{}", course.classroom_id, slot.key());
        occupied.insert(classroom_key, CourseInfo {
            subject_code: course.subject_code.clone(),
        });
        
        // Mark instructor occupied
        if let Some(instructor_id) = course.instructor_id {
            let instructor_key = format!("instructor_{}_{}", instructor_id, slot.key());
            occupied.insert(instructor_key, CourseInfo {
                subject_code: course.subject_code.clone(),
            });
        }
    }
    
    /// Save assignments to database
    async fn save_assignments(&self, assignments: &[Assignment]) -> Result<(), AppError> {
        for assignment in assignments {
            sqlx::query(
                "INSERT INTO academic_timetable_entries 
                 (classroom_course_id, day_of_week, period_id, room_id)
                 VALUES ($1, $2, $3, $4)"
            )
            .bind(assignment.course_id)
            .bind(&assignment.time_slot.day)
            .bind(assignment.time_slot.period_id)
            .bind(assignment.room_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}

impl TimeSlot {
    fn key(&self) -> String {
        format!("{}_{}", self.day, self.period_id)
    }
}

struct CourseInfo {
    subject_code: String,
}
```

**2. API Handler**
```rust
// backend-school/src/modules/academic/handlers/scheduling.rs

/// POST /api/academic/timetable/auto-schedule
pub async fn auto_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AutoScheduleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers)?;
    check_permission(&pool, &headers, "academic:timetable:manage").await?;
    
    let scheduler = Scheduler::new(pool.clone());
    
    let algorithm = payload.algorithm.as_deref().unwrap_or("greedy");
    
    let result = match algorithm {
        "greedy" => {
            scheduler.schedule_greedy(
                &payload.classroom_ids,
                payload.semester_id,
                payload.force.unwrap_or(false),
            ).await?
        }
        _ => {
            return Err(AppError::BadRequest("Unsupported algorithm".to_string()));
        }
    };
    
    Ok(Json(json!({
        "success": true,
        "job_id": result.job_id,
        "total": result.total,
        "scheduled": result.scheduled,
        "failed": result.failed,
        "warnings": result.warnings,
    })))
}
```

**3. Register Routes**
```rust
// backend-school/src/modules/academic/mod.rs

.route("/timetable/auto-schedule", post(handlers::scheduling::auto_schedule))
.route("/timetable/jobs/:job_id", get(handlers::scheduling::get_job_status))
```

**Time**: 2 days
**Deliverable**: Working auto-scheduler (Greedy)

---

### **Day 5: Frontend UI (MVP)** 🎨

**Goal**: ปุ่มจัดตาราง + แสดงผลลัพธ์

#### ✅ Tasks:

**1. API Client**
```typescript
// frontend-school/src/lib/api/scheduling.ts

export interface AutoScheduleRequest {
    classroom_ids: string[];
    semester_id: string;
    algorithm?: 'greedy' | 'backtracking';
    force?: boolean;
}

export interface SchedulingResult {
    job_id: string;
    total: number;
    scheduled: number;
    failed: number;
    warnings: string[];
}

export async function autoSchedule(
    req: AutoScheduleRequest
): Promise<SchedulingResult> {
    return apiClient.post('/academic/timetable/auto-schedule', req);
}

export async function getJobStatus(jobId: string) {
    return apiClient.get(`/academic/timetable/jobs/${jobId}`);
}
```

**2. UI Component (Simple)**
```svelte
<!-- In timetable/+page.svelte -->

<script lang="ts">
    import { autoSchedule } from '$lib/api/scheduling';
    
    let showAutoDialog = $state(false);
    let selectedClassrooms = $state<string[]>([]);
    let isScheduling = $state(false);
    
    async function handleAutoSchedule() {
        try {
            isScheduling = true;
            
            const result = await autoSchedule({
                classroom_ids: selectedClassrooms,
                semester_id: selectedSemesterId,
                algorithm: 'greedy',
                force: false
            });
            
            toast.success(`จัดตารางสำเร็จ ${result.scheduled}/${result.total} วิชา`);
            
            if (result.failed > 0) {
                toast.warning(`จัดไม่ได้ ${result.failed} วิชา: ${result.warnings.join(', ')}`);
            }
            
            await loadTimetable();
            showAutoDialog = false;
            
        } catch (e: any) {
            toast.error(e.message || 'เกิดข้อผิดพลาด');
        } finally {
            isScheduling = false;
        }
    }
</script>

<!-- Add button -->
<Button onclick={() => showAutoDialog = true}>
    <Sparkles class="w-4 h-4 mr-2" />
    จัดตารางอัตโนมัติ
</Button>

<!-- Simple Dialog -->
<Dialog.Root bind:open={showAutoDialog}>
    <Dialog.Content>
        <Dialog.Header>
            <Dialog.Title>จัดตารางอัตโนมัติ (แบบเร็ว)</Dialog.Title>
        </Dialog.Header>
        
        <div class="space-y-4">
            <div>
                <Label.Root>เลือกห้องเรียน</Label.Root>
                <!-- Classroom selection UI -->
            </div>
            
            <div class="text-sm text-muted-foreground">
                ℹ️ ระบบจะจัดตารางโดยใช้อัลกอริทึมแบบเร็ว อาจจัดไม่ได้บางวิชา
            </div>
        </div>
        
        <Dialog.Footer>
            <Button onclick={handleAutoSchedule} disabled={isScheduling}>
                {#if isScheduling}
                    <Loader2 class="animate-spin mr-2" />
                    กำลังจัดตาราง...
                {:else}
                    เริ่มจัดตาราง
                {/if}
            </Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
```

**Time**: 1 day
**Deliverable**: Working UI, Users can test MVP

---

## ✅ End of Week 1: MVP Complete!

**What we have:**
- ✅ Database schema (ready for Full Version)
- ✅ Greedy algorithm (70-85% success rate)
- ✅ Basic UI (auto-schedule button)
- ✅ Users can start testing!

**What's missing:**
- ❌ Backtracking (better results)
- ❌ Preferences (instructor availability)
- ❌ Bulk scheduling (all grade levels)
- ❌ Quality optimization

---

## Week 2: Enhancement to Full Version

### **Day 6-7: Backtracking Algorithm** 🔄

**Goal**: เพิ่ม algorithm ที่ให้ผลลัพธ์ดีกว่า

#### ✅ Tasks:

**1. Implement Backtracking**
```rust
// Add to scheduler.rs

impl Scheduler {
    /// Backtracking Algorithm - Better quality
    pub async fn schedule_backtracking(
        &self,
        classroom_ids: &[Uuid],
        semester_id: Uuid,
        force: bool,
    ) -> Result<SchedulingResult, AppError> {
        // Similar setup as greedy
        let job_id = self.create_job(classroom_ids, semester_id, "BACKTRACKING").await?;
        
        if force {
            self.clear_existing_timetable(classroom_ids, semester_id).await?;
        }
        
        let courses = self.get_courses_to_schedule(classroom_ids, semester_id).await?;
        let time_slots = self.get_available_time_slots(semester_id).await?;
        
        // Backtracking
        let mut best_solution = Vec::new();
        let mut best_score = 0.0;
        let mut occupied = HashMap::new();
        
        self.backtrack(
            &courses,
            0,
            &time_slots,
            &mut Vec::new(),
            &mut occupied,
            &mut best_solution,
            &mut best_score,
        ).await?;
        
        // Save best solution
        self.save_assignments(&best_solution).await?;
        
        Ok(SchedulingResult {
            job_id,
            total: courses.len(),
            scheduled: best_solution.len(),
            failed: courses.len() - best_solution.len(),
            warnings: vec![],
        })
    }
    
    /// Recursive backtracking
    fn backtrack(
        &self,
        courses: &[CourseToSchedule],
        index: usize,
        time_slots: &[TimeSlot],
        current: &mut Vec<Assignment>,
        occupied: &mut HashMap<String, CourseInfo>,
        best: &mut Vec<Assignment>,
        best_score: &mut f64,
    ) -> Result<(), AppError> {
        // Base case: all courses scheduled
        if index >= courses.len() {
            let score = self.calculate_quality_score(current);
            if score > *best_score {
                *best_score = score;
                *best = current.clone();
            }
            return Ok(());
        }
        
        // Try to schedule current course
        let course = &courses[index];
        
        // Try all possible assignments for this course
        for assignments in self.generate_possible_assignments(course, time_slots, occupied) {
            // Make assignment
            for assignment in &assignments {
                current.push(assignment.clone());
                self.mark_occupied(occupied, course, &assignment.time_slot);
            }
            
            // Recurse
            self.backtrack(courses, index + 1, time_slots, current, occupied, best, best_score)?;
            
            // Backtrack (undo)
            for assignment in &assignments {
                current.pop();
                self.unmark_occupied(occupied, course, &assignment.time_slot);
            }
        }
        
        Ok(())
    }
    
    /// Calculate quality score for a schedule
    fn calculate_quality_score(&self, assignments: &[Assignment]) -> f64 {
        let mut score = 100.0;
        
        // Penalty: Same subject on consecutive days
        // Bonus: Well distributed subjects
        // etc.
        
        score
    }
}
```

**2. Update API to support both algorithms**
```rust
let result = match algorithm {
    "greedy" => scheduler.schedule_greedy(...).await?,
    "backtracking" => scheduler.schedule_backtracking(...).await?,
    _ => return Err(AppError::BadRequest("Unknown algorithm".into())),
};
```

**Time**: 2 days
**Deliverable**: Backtracking algorithm (90-95% success)

---

### **Day 8: Preferences System** 👨‍🏫

**Goal**: ครูสามารถบอกเวลาที่ไม่ว่างได้

#### ✅ Tasks:

**1. Backend Handler**
```rust
/// GET/POST /api/academic/instructor-preferences
pub async fn get_my_preferences(...) -> Result<...> {
    // Get current instructor's preferences
}

pub async fn update_my_preferences(
    Json(payload): Json<UpdatePreferencesRequest>
) -> Result<...> {
    // Update unavailable_slots, preferred_slots, etc.
}
```

**2. Frontend UI (Simple)**
```svelte
<!-- New page: /staff/academic/scheduling-preferences/+page.svelte -->

<script lang="ts">
    let unavailableSlots = $state<{day: string, period_id: string}[]>([]);
    
    function toggleSlot(day: string, periodId: string) {
        // Mark slot as unavailable
    }
</script>

<div class="grid">
    <!-- Show week grid, allow clicking to mark unavailable -->
    {#each DAYS as day}
        {#each periods as period}
            <button
                class:unavailable={isUnavailable(day.value, period.id)}
                onclick={() => toggleSlot(day.value, period.id)}
            >
                {period.name}
            </button>
        {/each}
    {/each}
</div>
```

**3. Integrate with Scheduler**
```rust
// In is_slot_available(), add:
if let Some(prefs) = self.get_instructor_preferences(instructor_id).await? {
    if prefs.unavailable_slots.contains(&slot) {
        return Ok(false);
    }
}
```

**Time**: 1 day
**Deliverable**: Instructors can set preferences

---

### **Day 9: Bulk Scheduling + UI Polish** 🚀

**Goal**: จัดหลายห้องพร้อมกัน + UI สวยขึ้น

#### ✅ Tasks:

**1. Bulk Scheduling**
```rust
// Already supported in API (classroom_ids: Vec<Uuid>)
// Just need to handle properly:

let all_courses: Vec<_> = classroom_ids
    .iter()
    .flat_map(|id| get_courses(*id, semester_id))
    .collect();

// Then schedule all courses together
```

**2. Enhanced UI**
```svelte
<!-- Better dialog with more options -->

<div>
    <Label.Root>ขอบเขต</Label.Root>
    <Select.Root bind:value={scope}>
        <Select.Item value="single">ห้องเดียว</Select.Item>
        <Select.Item value="grade">ทั้งชั้น</Select.Item>
        <Select.Item value="all">ทั้งโรงเรียน</Select.Item>
    </Select.Root>
</div>

<div>
    <Label.Root>อัลกอริทึม</Label.Root>
    <Select.Root bind:value={algorithm}>
        <Select.Item value="greedy">
            เร็ว ⚡ (1-5 วินาที, 70-85% สำเร็จ)
        </Select.Item>
        <Select.Item value="backtracking">
            ดีที่สุด 🎯 (5-30 วินาที, 90-98% สำเร็จ)
        </Select.Item>
    </Select.Root>
</div>

<!-- Progress indicator -->
{#if isScheduling}
    <div class="space-y-2">
        <div class="flex justify-between">
            <span>กำลังจัดตาราง...</span>
            <span>{progress}/{total}</span>
        </div>
        <Progress value={progress} max={total} />
    </div>
{/if}
```

**Time**: 1 day
**Deliverable**: Full featured UI

---

## Day 10-12: Testing & Polish

### **Day 10: Integration Testing** 🧪
- Test with real school data
- Edge cases (100 classrooms, shared instructors)
- Performance testing

### **Day 11: Bug Fixes & Optimization** 🐛
- Fix issues from testing
- Performance tuning
- Add logging

### **Day 12: Documentation & Deployment** 📚
- User guide
- Video tutorial
- Deploy to production

---

## 🎯 Final Deliverables (Day 12)

### **Features:**
✅ Auto-schedule (Greedy + Backtracking)
✅ Single classroom or bulk
✅ Instructor preferences
✅ Conflict detection
✅ Progress tracking
✅ Quality optimization
✅ Clean UI/UX

### **Performance:**
- ✅ Single classroom: < 5 seconds
- ✅ Grade level (10 rooms): < 30 seconds
- ✅ Whole school (50 rooms): < 2 minutes

### **Quality:**
- ✅ 90-98% success rate
- ✅ Well-distributed subjects
- ✅ Respects preferences
- ✅ No hard constraint violations

---

## 📊 Why This Approach?

| Aspect | Direct Full (21d) | Hybrid (9-12d) |
|--------|------------------|----------------|
| **Time to First Working Version** | 21 days | 4 days |
| **Risk** | High (all or nothing) | Low (incremental) |
| **Feedback Loop** | Late | Early |
| **User Adoption** | Later | Earlier |
| **Flexibility** | Low | High |
| **Final Quality** | Same | Same |

**Verdict**: Hybrid approach is **safer, faster, and more flexible** ✅
