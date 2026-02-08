# แผนการพัฒนาระบบจัดตารางสอนอัตโนมัติ
# Auto Timetable Scheduling System

## 📋 ภาพรวมโครงการ

ระบบจัดตารางสอนอัตโนมัติจะช่วยให้โรงเรียนสามารถสร้างตารางเรียน/ตารางสอนได้โดยอัตโนมัติ ตามเงื่อนไขและข้อจำกัดต่าง ๆ แทนการลากวางด้วยมือทีละคาบ

## 🎯 วัตถุประสงค์

1. **ลดเวลาการจัดตาราง** จากหลายวันเหลือไม่กี่นาที
2. **ลดข้อผิดพลาด** ป้องกันครูสอนซ้อน, ห้องใช้ซ้อน
3. **กระจายคาบอย่างสมดุล** ไม่ให้วิชาเดียวกันอยู่ติดกันทุกวัน
4. **คำนึงถึงความต้องการของครู** เช่น วันที่ไม่สะดวก, คาบที่ต้องการ

## 🏗️ สถาปัตยกรรมระบบ

### Phase 1: Database Schema Extensions (1-2 วัน)

#### 1.1 เพิ่มตาราง Scheduling Preferences

```sql
-- ความต้องการของครูแต่ละคน
CREATE TABLE instructor_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- Unavailable slots (JSON array of {day, period_id})
    unavailable_slots JSONB DEFAULT '[]'::jsonb,
    
    -- Preferred slots (JSON array of {day, period_id})
    preferred_slots JSONB DEFAULT '[]'::jsonb,
    
    -- Maximum periods per day
    max_periods_per_day INTEGER DEFAULT 6,
    
    -- Maximum consecutive periods
    max_consecutive_periods INTEGER DEFAULT 3,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(instructor_id, academic_year_id)
);

-- Subject Constraints (ข้อจำกัดของวิชา)
CREATE TABLE subject_constraints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- เวลาที่เหมาะสม
    preferred_time_of_day VARCHAR(20), -- 'MORNING', 'AFTERNOON', 'ANY'
    
    -- ห้องพิเศษที่ต้องการ
    required_room_type VARCHAR(50), -- 'LAB', 'COMPUTER', 'GYM', NULL
    
    -- ควรจัดกระจายหรือรวมกัน
    scheduling_pattern VARCHAR(20) DEFAULT 'DISTRIBUTED', -- 'DISTRIBUTED', 'CONSECUTIVE', 'ANY'
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(subject_id, academic_year_id)
);

-- Scheduling Jobs (เก็บประวัติการ generate)
CREATE TABLE timetable_scheduling_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    classroom_id UUID REFERENCES class_rooms(id) ON DELETE SET NULL,
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,
    
    -- Scope
    scope VARCHAR(20) NOT NULL, -- 'SINGLE_CLASSROOM', 'GRADE_LEVEL', 'ALL_SCHOOL'
    scope_ids JSONB, -- array of classroom_ids if applicable
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING', -- 'PENDING', 'RUNNING', 'COMPLETED', 'FAILED', 'CANCELLED'
    
    -- Results
    total_courses INTEGER DEFAULT 0,
    scheduled_courses INTEGER DEFAULT 0,
    failed_courses INTEGER DEFAULT 0,
    
    -- Configuration
    config JSONB DEFAULT '{}'::jsonb, -- {force: bool, optimize: bool, etc.}
    
    -- Error/Warnings
    error_message TEXT,
    warnings JSONB DEFAULT '[]'::jsonb,
    
    -- Timing
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

#### 1.2 เพิ่ม metadata ใน subjects table

```sql
-- Add to existing subjects table
ALTER TABLE subjects 
ADD COLUMN IF NOT EXISTS periods_per_week INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS min_consecutive_periods INTEGER DEFAULT 1,
ADD COLUMN IF NOT EXISTS max_consecutive_periods INTEGER DEFAULT 2;

COMMENT ON COLUMN subjects.periods_per_week IS 'จำนวนคาบต่อสัปดาห์ที่ควรจะมี (0 = calculate from hours/credit)';
COMMENT ON COLUMN subjects.min_consecutive_periods IS 'คาบต่อเนื่องขั้นต่ำ (เช่น พละอาจต้อง 2 คาบติดกัน)';
COMMENT ON COLUMN subjects.max_consecutive_periods IS 'คาบต่อเนื่องสูงสุด (ไม่ให้เรียนวิชาเดียวเกินนี้)';
```

### Phase 2: Backend Auto-Scheduling Algorithm (3-5 วัน)

#### 2.1 สร้าง Scheduling Service

**File: `backend-school/src/modules/academic/services/auto_scheduler.rs`**

```rust
// Core structures
pub struct SchedulingRequest {
    pub classroom_ids: Vec<Uuid>,
    pub semester_id: Uuid,
    pub config: SchedulingConfig,
}

pub struct SchedulingConfig {
    pub force_overwrite: bool,          // ลบตารางเดิมหรือไม่
    pub optimize_level: OptimizeLevel,  // FAST, BALANCED, BEST
    pub respect_preferences: bool,      // คำนึงถึง preferences ของครูหรือไม่
    pub max_iterations: u32,            // จำนวนรอบสูงสุดที่จะพยายาม
}

pub struct TimeSlot {
    pub day: DayOfWeek,
    pub period_id: Uuid,
}

pub struct SchedulingConstraints {
    pub hard_constraints: Vec<HardConstraint>,
    pub soft_constraints: Vec<SoftConstraint>,
}

// Main algorithm
impl AutoScheduler {
    /// Main entry point
    pub async fn generate_timetable(
        pool: &PgPool,
        request: SchedulingRequest,
    ) -> Result<SchedulingResult> {
        let job_id = Self::create_job(pool, &request).await?;
        
        // 1. Gather all data
        let courses = Self::get_courses_to_schedule(pool, &request).await?;
        let periods = Self::get_available_periods(pool, &request).await?;
        let constraints = Self::build_constraints(pool, &request).await?;
        
        // 2. Run scheduling algorithm
        let assignments = match request.config.optimize_level {
            OptimizeLevel::FAST => Self::greedy_schedule(&courses, &periods, &constraints)?,
            OptimizeLevel::BALANCED => Self::backtracking_schedule(&courses, &periods, &constraints)?,
            OptimizeLevel::BEST => Self::simulated_annealing_schedule(&courses, &periods, &constraints)?,
        };
        
        // 3. Save to database
        Self::save_timetable(pool, &assignments, &request).await?;
        
        // 4. Update job status
        Self::complete_job(pool, job_id, &assignments).await?;
        
        Ok(SchedulingResult {
            job_id,
            scheduled: assignments.len(),
            failed: courses.len() - assignments.len(),
        })
    }
    
    /// Greedy algorithm (เร็วแต่อาจไม่ได้ผลลัพธ์ดีที่สุด)
    fn greedy_schedule(
        courses: &[CourseToSchedule],
        periods: &[TimeSlot],
        constraints: &SchedulingConstraints,
    ) -> Result<Vec<Assignment>> {
        let mut assignments = Vec::new();
        let mut occupied = HashMap::new();
        
        // Sort courses by difficulty (วิชาที่ยากจัดก่อน)
        let mut sorted_courses = courses.to_vec();
        sorted_courses.sort_by_key(|c| {
            // ยากถ้า: ครูสอนหลายห้อง, วิชามีข้อจำกัดเยอะ, ห้องพิเศษน้อย
            -(c.constraint_count() as i32)
        });
        
        for course in sorted_courses {
            // Find first available slot that satisfies constraints
            if let Some(slot) = Self::find_best_slot(&course, periods, &occupied, constraints) {
                assignments.push(Assignment {
                    course_id: course.id,
                    time_slot: slot.clone(),
                    room_id: Self::assign_room(&course, &slot, &occupied)?,
                });
                Self::mark_occupied(&mut occupied, &slot, &course);
            } else {
                // Cannot schedule this course
                warn!("Cannot schedule course: {:?}", course);
            }
        }
        
        Ok(assignments)
    }
    
    /// Backtracking algorithm (ดีกว่า greedy, หาคำตอบที่ดีขึ้น)
    fn backtracking_schedule(
        courses: &[CourseToSchedule],
        periods: &[TimeSlot],
        constraints: &SchedulingConstraints,
    ) -> Result<Vec<Assignment>> {
        let mut assignments = Vec::new();
        let mut occupied = HashMap::new();
        
        if Self::backtrack(courses, 0, periods, constraints, &mut assignments, &mut occupied) {
            Ok(assignments)
        } else {
            Err(AppError::BadRequest("Cannot find valid schedule".to_string()))
        }
    }
    
    fn backtrack(
        courses: &[CourseToSchedule],
        index: usize,
        periods: &[TimeSlot],
        constraints: &SchedulingConstraints,
        assignments: &mut Vec<Assignment>,
        occupied: &mut HashMap<String, CourseInfo>,
    ) -> bool {
        // Base case: all courses scheduled
        if index >= courses.len() {
            return true;
        }
        
        let course = &courses[index];
        
        // Try each possible slot
        for slot in periods {
            if Self::is_valid_assignment(course, slot, occupied, constraints) {
                // Make assignment
                let assignment = Assignment {
                    course_id: course.id,
                    time_slot: slot.clone(),
                    room_id: Self::assign_room(course, slot, occupied).ok(),
                };
                assignments.push(assignment.clone());
                Self::mark_occupied(occupied, slot, course);
                
                // Recurse
                if Self::backtrack(courses, index + 1, periods, constraints, assignments, occupied) {
                    return true;
                }
                
                // Undo (backtrack)
                assignments.pop();
                Self::unmark_occupied(occupied, slot, course);
            }
        }
        
        false
    }
    
    /// Check hard constraints
    fn is_valid_assignment(
        course: &CourseToSchedule,
        slot: &TimeSlot,
        occupied: &HashMap<String, CourseInfo>,
        constraints: &SchedulingConstraints,
    ) -> bool {
        // 1. Classroom conflict
        let classroom_key = format!("classroom_{}_{}", course.classroom_id, slot.key());
        if occupied.contains_key(&classroom_key) {
            return false;
        }
        
        // 2. Instructor conflict
        if let Some(instructor_id) = course.instructor_id {
            let instructor_key = format!("instructor_{}_{}", instructor_id, slot.key());
            if occupied.contains_key(&instructor_key) {
                return false;
            }
            
            // 3. Instructor preferences
            if let Some(prefs) = constraints.instructor_prefs.get(&instructor_id) {
                if prefs.unavailable_slots.contains(slot) {
                    return false;
                }
            }
        }
        
        // 4. Room conflict (if requires special room)
        if let Some(required_room_type) = &course.required_room_type {
            // Check if any room of this type is available
            if !Self::has_available_room(required_room_type, slot, occupied) {
                return false;
            }
        }
        
        // 5. Subject constraints
        if let Some(subject_constraints) = constraints.subject_constraints.get(&course.subject_id) {
            // Check time of day preference
            if let Some(pref_time) = &subject_constraints.preferred_time_of_day {
                if !Self::matches_time_preference(slot, pref_time) {
                    // This is soft constraint in some cases, hard in others
                    // For now, treat as soft (allow but penalize)
                }
            }
        }
        
        true
    }
    
    /// Calculate quality score (for optimization)
    fn calculate_score(
        assignments: &[Assignment],
        constraints: &SchedulingConstraints,
    ) -> f64 {
        let mut score = 0.0;
        
        // Positive: spread well, respects preferences
        // Negative: too many consecutive, ignores preferences
        
        // TODO: Implement scoring heuristics
        
        score
    }
}
```

#### 2.2 สร้าง API Handlers

**File: `backend-school/src/modules/academic/handlers/auto_scheduler.rs`**

```rust
/// POST /api/academic/timetable/auto-generate
pub async fn auto_generate_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AutoGenerateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers)?;
    check_permission(&pool, &headers, "academic:timetable:manage").await?;
    
    let request = SchedulingRequest {
        classroom_ids: payload.classroom_ids,
        semester_id: payload.semester_id,
        config: SchedulingConfig {
            force_overwrite: payload.force.unwrap_or(false),
            optimize_level: payload.optimize_level.unwrap_or(OptimizeLevel::BALANCED),
            respect_preferences: payload.respect_preferences.unwrap_or(true),
            max_iterations: payload.max_iterations.unwrap_or(1000),
        },
    };
    
    // Run async (for large schools, this may take time)
    let result = AutoScheduler::generate_timetable(&pool, request).await?;
    
    Ok(Json(json!({
        "success": true,
        "job_id": result.job_id,
        "scheduled": result.scheduled,
        "failed": result.failed,
    })))
}

/// GET /api/academic/timetable/jobs/{job_id}
pub async fn get_scheduling_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers)?;
    
    let job = sqlx::query_as::<_, SchedulingJob>(
        "SELECT * FROM timetable_scheduling_jobs WHERE id = $1"
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await?;
    
    Ok(Json(job))
}
```

### Phase 3: Frontend UI (2-3 วัน)

#### 3.1 เพิ่มปุ่ม "จัดตารางอัตโนมัติ" ในหน้า Timetable

**File: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`**

เพิ่ม UI components:
1. ปุ่ม "🤖 จัดตารางอัตโนมัติ" ที่มุมบนขวา
2. Dialog สำหรับตั้งค่า:
   - เลือกห้องเรียน (1 ห้อง หรือหลายห้อง)
   - ระดับการ Optimize (เร็ว / ปานกลาง / ดีที่สุด)
   - ลบตารางเดิมหรือไม่
   - คำนึงถึงความต้องการของครูหรือไม่
3. Progress indicator ระหว่าง generate
4. แสดงผลลัพธ์ (สำเร็จกี่วิชา, ล้มเหลวกี่วิชา)

```svelte
<script lang="ts">
    // ... existing code ...
    
    let showAutoScheduleModal = $state(false);
    let autoScheduleClassrooms = $state<string[]>([]);
    let autoScheduleOptimize = $state<'FAST' | 'BALANCED' | 'BEST'>('BALANCED');
    let autoScheduleForce = $state(false);
    let autoScheduleRunning = $state(false);
    
    async function handleAutoSchedule() {
        if (autoScheduleClassrooms.length === 0) {
            toast.error('กรุณาเลือกห้องเรียนอย่างน้อย 1 ห้อง');
            return;
        }
        
        try {
            autoScheduleRunning = true;
            
            const response = await autoGenerateTimetable({
                classroom_ids: autoScheduleClassrooms,
                semester_id: selectedSemesterId,
                force: autoScheduleForce,
                optimize_level: autoScheduleOptimize,
                respect_preferences: true,
            });
            
            if (response.success) {
                toast.success(`จัดตารางสำเร็จ ${response.scheduled} วิชา`);
                if (response.failed > 0) {
                    toast.warning(`ไม่สามารถจัด ${response.failed} วิชา`);
                }
                await loadTimetable();
                showAutoScheduleModal = false;
            }
        } catch (e: any) {
            toast.error(e.message || 'เกิดข้อผิดพลาด');
        } finally {
            autoScheduleRunning = false;
        }
    }
</script>

<!-- Add button in header -->
<Button onclick={() => showAutoScheduleModal = true}>
    <Sparkles class="w-4 h-4 mr-2" />
    จัดตารางอัตโนมัติ
</Button>

<!-- Auto Schedule Dialog -->
<Dialog.Root bind:open={showAutoScheduleModal}>
    <Dialog.Content class="max-w-2xl">
        <Dialog.Header>
            <Dialog.Title>🤖 จัดตารางสอนอัตโนมัติ</Dialog.Title>
            <Dialog.Description>
                ระบบจะจัดตารางให้โดยอัตโนมัติตามเงื่อนไขและข้อจำกัดต่าง ๆ
            </Dialog.Description>
        </Dialog.Header>
        
        <div class="space-y-4">
            <!-- Classroom Selection -->
            <div>
                <Label.Root>เลือกห้องเรียน</Label.Root>
                <div class="grid grid-cols-2 gap-2 mt-2 max-h-64 overflow-y-auto">
                    {#each classrooms as classroom}
                        <label class="flex items-center gap-2 p-2 border rounded hover:bg-accent cursor-pointer">
                            <Checkbox
                                checked={autoScheduleClassrooms.includes(classroom.id)}
                                onCheckedChange={(checked) => {
                                    if (checked) {
                                        autoScheduleClassrooms = [...autoScheduleClassrooms, classroom.id];
                                    } else {
                                        autoScheduleClassrooms = autoScheduleClassrooms.filter(id => id !== classroom.id);
                                    }
                                }}
                            />
                            <span class="text-sm">{classroom.name}</span>
                        </label>
                    {/each}
                </div>
            </div>
            
            <!-- Optimize Level -->
            <div>
                <Label.Root>ระดับการปรับให้เหมาะสม</Label.Root>
                <Select.Root bind:value={autoScheduleOptimize}>
                    <Select.Trigger>
                        <Select.Value />
                    </Select.Trigger>
                    <Select.Content>
                        <Select.Item value="FAST">เร็ว (อาจไม่ได้ตารางที่ดีที่สุด)</Select.Item>
                        <Select.Item value="BALANCED">ปานกลาง (แนะนำ)</Select.Item>
                        <Select.Item value="BEST">ดีที่สุด (ใช้เวลานาน)</Select.Item>
                    </Select.Content>
                </Select.Root>
            </div>
            
            <!-- Force Overwrite -->
            <label class="flex items-center gap-2">
                <Checkbox bind:checked={autoScheduleForce} />
                <span class="text-sm">ลบตารางเดิมและสร้างใหม่ทั้งหมด</span>
            </label>
        </div>
        
        <Dialog.Footer>
            <Button variant="outline" onclick={() => showAutoScheduleModal = false}>
                ยกเลิก
            </Button>
            <Button 
                onclick={handleAutoSchedule}
                disabled={autoScheduleRunning || autoScheduleClassrooms.length === 0}
            >
                {#if autoScheduleRunning}
                    <Loader2 class="w-4 h-4 mr-2 animate-spin" />
                    กำลังจัดตาราง...
                {:else}
                    เริ่มจัดตาราง
                {/if}
            </Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
```

#### 3.2 สร้างหน้า Preferences Management (Optional แต่แนะนำ)

**File: `frontend-school/src/routes/(app)/staff/academic/scheduling-preferences/+page.svelte`**

ให้ครูสามารถตั้งค่า:
- วันเวลาที่ไม่สะดวกสอน
- วันเวลาที่ต้องการสอน
- จำนวนคาบสูงสุดต่อวัน

### Phase 4: Testing & Optimization (2-3 วัน)

1. **Unit Tests** สำหรับ algorithm
2. **Integration Tests** สำหรับ API
3. **Performance Testing** กับข้อมูลจริง (100+ ห้อง, 1000+ วิชา)
4. **User Acceptance Testing** กับผู้ใช้จริง

## 📊 ตัวอย่าง Algorithm Flow

```
1. Input:
   - Classroom: ม.4/1 (30 คน)
   - Semester: 1/2568
   - Courses: 
     * คณิตศาสตร์ (4 คาบ/สัปดาห์, ครูสมชาย)
     * ภาษาไทย (3 คาบ/สัปดาห์, ครูสมหญิง)
     * วิทยาศาสตร์ (4 คาบ/สัปดาห์, ครูสมชาย, ต้องใช้ LAB)
     * ...รวม 15 วิชา

2. Build Timetable Grid:
   - Days: MON-FRI (5 วัน)
   - Periods: 8 คาบ/วัน
   - Total slots: 40 ช่อง

3. Sort Courses by Difficulty:
   - วิทยาศาสตร์ (ยากสุด: ต้องใช้ LAB, ครูสอนหลายห้อง)
   - คณิตศาสตร์ (ครูสอนหลายห้อง)
   - ภาษาไทย
   - ...

4. Schedule (Greedy):
   For each course:
     For each period needed:
       Find first available slot where:
         ✅ ห้องว่าง
         ✅ ครูว่าง
         ✅ LAB ว่าง (ถ้าต้องการ)
         ✅ ไม่ขัดกับ preferences
         ✅ ไม่มี 4 คาบติดกันเกินไป
       Assign → Mark as occupied

5. Output:
   - Success: 58/60 คาบ (97%)
   - Failed: 2 วิชา (พละ 2 คาบ - สนามไม่ว่าง)
```

## ⚡ Optimization Strategies

### 1. **Pre-computation**
- Cache instructor availability
- Pre-filter impossible slots
- Build conflict graph

### 2. **Heuristics**
- **Most Constrained First**: จัดวิชาที่ยากก่อน
- **Least Constraining Value**: เลือก slot ที่กระทบน้อยที่สุด
- **Random Restart**: ถ้าติดก็เริ่มใหม่ด้วย random seed ต่างกัน

### 3. **Parallel Processing** (Advanced)
- แยกจัดแต่ละชั้นปีพร้อมกัน (ไม่มี conflict ข้ามชั้น)
- ใช้ Web Workers / Background Jobs

## 🚀 Deployment Plan

### MVP (Minimum Viable Product) - 7-10 วัน
- ✅ Basic greedy algorithm
- ✅ Hard constraints only
- ✅ Single classroom scheduling
- ✅ Simple UI

### V1.0 - 14-21 วัน (Full Features)
- ✅ Backtracking algorithm
- ✅ Soft constraints
- ✅ Batch scheduling (multiple classrooms)
- ✅ Preferences management
- ✅ Result visualization

### V2.0 - Future
- ✅ Machine Learning optimization
- ✅ Historical data analysis
- ✅ Conflict resolution suggestions
- ✅ What-if analysis

## 🎓 ตัวอย่างการใช้งาน

```typescript
// 1. Auto-generate for single classroom
await autoGenerateTimetable({
    classroom_ids: ['uuid-m4-1'],
    semester_id: 'uuid-sem-1-2568',
    optimize_level: 'BALANCED'
});

// 2. Auto-generate for entire grade level
const m4Classrooms = classrooms.filter(c => c.grade_level === 'M4');
await autoGenerateTimetable({
    classroom_ids: m4Classrooms.map(c => c.id),
    semester_id: 'uuid-sem-1-2568',
    force: true, // Clear existing
    optimize_level: 'BEST'
});

// 3. Check job status
const job = await getSchedulingJob('uuid-job-123');
console.log(`Status: ${job.status}, Progress: ${job.scheduled}/${job.total_courses}`);
```

## 🎯 Success Metrics

1. **Scheduling Success Rate**: > 95% of courses scheduled
2. **Performance**: < 30 seconds for 50 classrooms
3. **User Satisfaction**: Reduce manual work by > 80%
4. **Accuracy**: Zero hard constraint violations

## 📚 References

- [Constraint Satisfaction Problems](https://en.wikipedia.org/wiki/Constraint_satisfaction_problem)
- [Timetabling Problem](https://en.wikipedia.org/wiki/Timetabling)
- [Simulated Annealing](https://en.wikipedia.org/wiki/Simulated_annealing)
- [Genetic Algorithms for Timetabling](https://arxiv.org/abs/1903.07265)

---

**ทีมพัฒนา**: SchoolOrbit Development Team
**วันที่สร้าง**: 2026-02-08
**เวอร์ชัน**: 1.0 (Draft)
