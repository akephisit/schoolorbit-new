# 📋 Phase 1 Progress Report: Database Schema & Models
## Auto Timetable Scheduling System

**Date**: 2026-02-08  
**Status**: ✅ COMPLETED  
**Duration**: ~30 minutes

---

## ✅ Completed Tasks

### 1. **Database Migrations Created** (5 files)

#### **039_add_auto_scheduling_to_subjects.sql**
- ✅ Added `min_consecutive_periods` (default: 1)
- ✅ Added `max_consecutive_periods` (default: 2)
- ✅ Added `allow_single_period` (default: true)
- ✅ Added `periods_per_week` (for scheduling)
- ✅ Added `preferred_time_of_day` (MORNING/AFTERNOON/ANYTIME)
- ✅ Added `required_room_type` (LAB/FIELD/COMPUTER)
- ✅ Set sensible defaults for PE, CORE, ELECTIVE, ACTIVITY subjects

#### **040_create_instructor_preferences.sql**
- ✅ Created `instructor_preferences` table
- ✅ Fields:
  - `hard_unavailable_slots` (JSONB) - ครูไม่ว่างเด็ดขาด
  - `preferred_slots` (JSONB) - ช่วงเวลาที่ชอบ
  - `max_periods_per_day`, `min_periods_per_day`
  - `preferred_days`, `avoid_days` (JSONB)
- ✅ Unique constraint: one record per instructor per year

#### **041_create_instructor_room_assignments.sql**
- ✅ Created `instructor_room_assignments` table
- ✅ Fields:
  - `is_preferred`, `is_required` (HARD vs SOFT)
  - `for_subjects` (JSONB) - ระบุเฉพาะวิชา
  - `reason` - เหตุผล
- ✅ Supports multiple room assignments per instructor

#### **042_create_timetable_locked_slots.sql**
- ✅ Created `timetable_locked_slots` table
- ✅ Flexible scoping:
  - `CLASSROOM` - เฉพาะห้อง
  - `GRADE_LEVEL` - ทั้งชั้น
  - `ALL_SCHOOL` - ทั้งโรงเรียน
- ✅ Fields:
  - `scope_type`, `scope_ids` (JSONB)
  - `subject_id`, `day_of_week`, `period_ids` (JSONB)
  - `room_id`, `instructor_id` (optional)
- ✅ GIN indexes for JSONB queries

#### **043_create_timetable_scheduling_jobs.sql**
- ✅ Created `timetable_scheduling_jobs` table
- ✅ Created ENUMs:
  - `scheduling_status`: PENDING, RUNNING, COMPLETED, FAILED, CANCELLED
  - `scheduling_algorithm`: GREEDY, BACKTRACKING, HYBRID
- ✅ Fields:
  - `classroom_ids` (JSONB) - ห้องที่ต้องการจัด
  - `algorithm`, `config` (JSONB)
  - `status`, `progress` (0-100)
  - `quality_score`, `scheduled_courses`, `total_courses`
  - `failed_courses` (JSONB)
  - Timing: `started_at`, `completed_at`, `duration_seconds`

---

### 2. **Rust Models Created**

#### **backend-school/src/modules/academic/models/scheduling.rs**
- ✅ `InstructorPreference` struct + FromRow
- ✅ `InstructorRoomAssignment` struct + FromRow
- ✅ `TimetableLockedSlot` struct + FromRow
- ✅ `TimetableSchedulingJob` struct + FromRow
- ✅ Request/Response types for all models:
  - `CreateInstructorPreferenceRequest`, `UpdateInstructorPreferenceRequest`
  - `CreateInstructorRoomAssignmentRequest`, `UpdateInstructorRoomAssignmentRequest`
  - `CreateLockedSlotRequest`, `UpdateLockedSlotRequest`
  - `CreateSchedulingJobRequest`, `SchedulingJobResponse`
- ✅ ENUMs:
  - `LockedSlotScope`: Classroom, GradeLevel, AllSchool
  - `SchedulingStatus`: Pending, Running, Completed, Failed, Cancelled
  - `SchedulingAlgorithm`: Greedy, Backtracking, Hybrid
- ✅ `SchedulingConfig` struct with Default implementation
- ✅ `FailedCourseInfo` struct for reporting

#### **Updated mod.rs**
- ✅ Added `pub mod scheduling;` to exports

---

## 📊 Database Schema Summary

| Table | Rows (est.) | Purpose | Hard/Soft |
|-------|-------------|---------|-----------|
| `subjects` (updated) | ~50 | Consecutive period rules, time preferences | Both |
| `instructor_preferences` | ~20 | Teacher time preferences & unavailability | Both |
| `instructor_room_assignments` | ~10 | Fixed rooms for teachers | Both |
| `timetable_locked_slots` | ~5 | Pre-assigned immutable slots | Hard |
| `timetable_scheduling_jobs` | ~100 | Job tracking & results | Meta |

**Total new columns**: 6 (in subjects)  
**Total new tables**: 4  
**Total new ENUMs**: 2

---

## 📁 Files Created

```
backend-school/
├── migrations/
│   ├── 039_add_auto_scheduling_to_subjects.sql
│   ├── 040_create_instructor_preferences.sql
│   ├── 041_create_instructor_room_assignments.sql
│   ├── 042_create_timetable_locked_slots.sql
│   └── 043_create_timetable_scheduling_jobs.sql
└── src/modules/academic/models/
    ├── scheduling.rs (NEW)
    └── mod.rs (UPDATED)
```

---

## 🎯 Constraints Supported

### **Hard Constraints (9 total)**
1. ✅ HC-1: Classroom conflict prevention
2. ✅ HC-2: Instructor conflict prevention
3. ✅ HC-3: Room availability
4. ✅ HC-4: Period requirements
5. ✅ HC-5: Valid time slot
6. ✅ HC-6: Instructor unavailability (hard)
7. ✅ **HC-7: Consecutive period requirements** ⭐ NEW
8. ✅ **HC-8: Fixed room assignment** ⭐ NEW
9. ✅ **HC-9: Pre-assigned/locked slots** ⭐ NEW

### **Soft Constraints (8 total)**
1. ✅ SC-1: Subject distribution
2. ✅ SC-2: Consecutive period limit
3. ✅ SC-3: Time of day preference
4. ✅ SC-4: Instructor preference (soft)
5. ✅ SC-5: Daily load balance
6. ✅ SC-6: Instructor daily load limit
7. ✅ SC-7: Avoid first/last period for special subjects
8. ✅ SC-8: Same subject spacing

---

## 🔧 Next Steps (Phase 2)

### **Phase 2.1: Core Scheduling Engine (Day 1-2)**
- [ ] Create `backend-school/src/modules/academic/services/scheduler/`
  - [ ] `mod.rs` - Module exports
  - [ ] `types.rs` - Internal data structures
  - [ ] `validator.rs` - Constraint validation
  - [ ] `greedy.rs` - Greedy algorithm
  - [ ] `quality.rs` - Quality scoring system

### **Phase 2.2: API Handlers (Day 2-3)**
- [ ] Create `backend-school/src/modules/academic/handlers/scheduling.rs`
  - [ ] POST `/api/academic/scheduling/auto-schedule` - Trigger scheduling
  - [ ] GET `/api/academic/scheduling/jobs/:id` - Get job status
  - [ ] GET `/api/academic/scheduling/jobs` - List jobs
  - [ ] POST `/api/academic/instructor-preferences` - Set preferences
  - [ ] POST `/api/academic/instructor-rooms` - Set room assignments
  - [ ] POST `/api/academic/timetable/lock-slot` - Lock slots
  - [ ] GET `/api/academic/timetable/locked-slots` - List locks
- [ ] Update router in `backend-school/src/modules/academic/routes.rs`

### **Phase 2.3: Frontend Integration (Day 3-4)**
- [ ] Create `frontend-school/src/lib/api/scheduling.ts`
- [ ] Create UI pages:
  - [ ] Auto-schedule trigger page
  - [ ] Job status monitor
  - [ ] Instructor preferences form
  - [ ] Room assignment manager
  - [ ] Locked slots manager

---

## 🎉 Phase 1 Summary

**✅ Database schema ready for auto-scheduling**  
**✅ All 17 constraints properly modeled**  
**✅ Rust models with full type safety**  
**✅ Ready for Phase 2 implementation**

---

**Migration Status**: ⚠️ **Not yet applied to database**  
*Note: Migrations will be applied when backend connects to the database*

---

**Estimated Progress**: 📊 **15% Complete**  
- [x] Phase 1: Database & Models (Day 1) - DONE ✅
- [ ] Phase 2: Scheduling Engine (Day 2-4)
- [ ] Phase 3: Backtracking Enhancement (Day 5-7)
- [ ] Phase 4: Frontend UI (Day 8-10)
- [ ] Phase 5: Testing & Polish (Day 11-12)

---

**Last Updated**: 2026-02-08 09:05 +07:00
