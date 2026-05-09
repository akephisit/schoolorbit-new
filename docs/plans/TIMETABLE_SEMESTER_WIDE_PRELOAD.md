# Plan: Timetable refactor — Semester-wide preload

## เป้าหมาย

เปลี่ยนหน้า `/staff/academic/timetable` จาก **lazy per-view loading** เป็น **eager all-semester preload** เพื่อให้:

- เปลี่ยน view (เลือกห้อง/ครู/toggle ghost) ลื่นไหล 0ms
- Drop วิชา/activity แสดงครูครบทุกประเภททันที
- ลด bug ที่เกิดจาก state stale ระหว่าง view switching
- Code clean — single source of truth (`allEntries`) แทน dual state (`timetableEntries` + `rawTeamEntries`)

User OK กับ initial load ที่ช้าลง 1-2 วินาที แลกกับการใช้งานลื่น

---

## Flow ปัจจุบัน (Lazy per-view)

```
User เข้าหน้า → เลือก semester → เลือก view (CLASSROOM ม.1/1)
  ↓
โหลดเฉพาะข้อมูลของห้องที่เลือก (5 API calls):
  - timetableEntries (ม.1/1 entries)
  - courses (ม.1/1 courses)
  - sidebarActivitySlots (ที่ ม.1/1 ร่วม)
  - courseTeamsMap (teams ของ ม.1/1)
  - activityInstructorMap (ของ ม.1/1)
  - occupancyEntries (semester ทั้งหมด — มี Stage 1 อยู่แล้ว)

User เปลี่ยน view → ม.1/2
  ↓
❌ โหลดใหม่ทั้งหมด (~200-500ms)
```

## Flow ใหม่ (Eager all-semester)

```
User เข้าหน้า → เลือก semester
  ↓
🔄 โหลดทั้ง semester ครั้งเดียว (parallel):

  Stage A — หลัก (parallel 4 calls):
    allEntries           ← GET /timetable?semester_id=X (ไม่กรอง classroom/instructor)
    allCourses           ← GET /planning/courses?semester_id=X
    allActivitySlots     ← GET /activity-slots?semester_id=X
    occupancyEntries     ← GET /timetable/occupancy?semester_id=X

  Stage B — เสริม (parallel หลัง A เสร็จ):
    courseTeamsMap       ← batchListCourseInstructors(allCourses ids)
    activityInstructorMap ← parallel listSlotClassroomAssignments(slot.id)
                            สำหรับ independent slots

⏱️ รวม ~1-2 วินาที (ครั้งเดียว)
─────────────────────────────────────────────
User เปลี่ยน view → ม.1/2
  ↓
✨ filter จาก allEntries → 0ms
```

---

## ข้อมูลที่โหลด (ขนาดประมาณ — โรงเรียนกลาง 30 ห้อง)

### Stage A — โหลดหลัก

| ข้อมูล | API | ขนาด | ใช้ทำอะไร |
|---|---|---|---|
| `allEntries` | `GET /timetable?semester_id` | ~5000 entries × ~500B = 2-3MB | grid + occupancy + ghost calc |
| `allCourses` | `GET /planning/courses?semester_id` | ~900 × ~300B = 250KB | sidebar + tempEntry joined |
| `allActivitySlots` | `GET /activity-slots?semester_id` | ~30 × ~500B = 15KB | sidebar + tempEntry joined |
| `occupancyEntries` | `GET /timetable/occupancy?semester_id` | ~5000 × 100B = 500KB | computeValidMoves |

### Stage B — โหลดเสริม

| ข้อมูล | API | ขนาด | ใช้ทำอะไร |
|---|---|---|---|
| `courseTeamsMap` | `GET /planning/courses/instructors?course_ids=...` | ~2700 × 100B = 250KB | tempEntry team |
| `activityInstructorMap` | `GET /activity-slots/{id}/classroom-assignments × N` | ~300 × 100B = 30KB | tempEntry activity ครู |

### ข้อมูลที่มีอยู่แล้ว (ไม่เปลี่ยน)

| ข้อมูล | โหลดที่ไหน |
|---|---|
| `rooms`, `classrooms`, `periods`, `instructors` | onMount ครั้งเดียวต่อปีการศึกษา |
| `academicYears`, `allSemesters` | onMount |

**รวม: ~3.5 MB JSON ต่อ session, memory ~10-15 MB**

---

## เปรียบเทียบ Performance

| Action | ก่อน | หลัง |
|---|---|---|
| โหลดหน้าครั้งแรก | 200-500ms | **1-2s** ⚠️ ช้าลง |
| เปลี่ยน CLASSROOM ม.1/1 → ม.1/2 | 200-500ms | **0ms** ✨ |
| เปลี่ยน CLASSROOM → INSTRUCTOR | 200-500ms | **0ms** ✨ |
| Toggle "แสดงคาบในทีม" (ghost) | 0ms | 0ms |
| Drop COURSE (CREATE) | 0ms render + 100ms DB | 0ms render + 100ms DB |
| Drop activity (CREATE) | ครูไม่ขึ้น 100ms | **ครูขึ้นทันที** ✨ |
| Drag เริ่ม (validity) | 0ms | 0ms |

---

## Architecture changes

### State refactor

```ts
// ก่อน
let timetableEntries = $state<TimetableEntry[]>([]);  // per-view
let rawTeamEntries   = $state<TimetableEntry[]>([]);  // INSTRUCTOR view raw
let courses          = $state<ClassroomCourse[]>([]); // per-view
let sidebarActivitySlots = $state<ActivitySlot[]>([]); // per-view

// หลัง
let allEntries       = $state<TimetableEntry[]>([]);  // semester-wide
let allCourses       = $state<ClassroomCourse[]>([]); // semester-wide
let allActivitySlots = $state<ActivitySlot[]>([]);    // semester-wide
// rawTeamEntries → ลบ
```

### Derived (display filtering)

```ts
const displayEntries = $derived.by(() => {
    if (viewMode === 'CLASSROOM') {
        return allEntries.filter(e => e.classroom_id === selectedClassroomId);
    }
    // INSTRUCTOR view
    const myInstr = (e: TimetableEntry) =>
        (e.instructor_ids ?? []).includes(selectedInstructorId);
    if (!showTeamGhosts) {
        return allEntries.filter(myInstr);
    }
    // Ghost mode: ของฉัน + entries ของ course ที่ฉันอยู่ในทีม
    const myTeamCourseIds = new Set(
        Array.from(courseTeamsMap.entries())
            .filter(([, team]) =>
                team.some(m => m.instructor_id === selectedInstructorId))
            .map(([cid]) => cid)
    );
    return allEntries.filter(e =>
        myInstr(e) ||
        (e.classroom_course_id && myTeamCourseIds.has(e.classroom_course_id))
    );
});

const displayCourses = $derived.by(() => {
    if (viewMode === 'CLASSROOM') {
        return allCourses.filter(c => c.classroom_id === selectedClassroomId);
    }
    return allCourses.filter(c => {
        const team = courseTeamsMap.get(c.id) ?? [];
        return team.some(m => m.instructor_id === selectedInstructorId);
    });
});

const displaySidebarSlots = $derived.by(() => {
    if (viewMode === 'CLASSROOM') {
        return allActivitySlots.filter(slot =>
            (slot.classroom_ids ?? []).includes(selectedClassroomId));
    }
    // INSTRUCTOR view: synchronized slots ที่ครูอยู่ + independent slots ที่มี assignment
    // (ใช้ logic เดิมจาก loadSidebarActivitySlots แต่เป็น derived)
    ...
});

const displayInstructorActivityItems = $derived.by(() => {
    // INSTRUCTOR view: independent slot × classroom ที่ครูสอน
    if (viewMode !== 'INSTRUCTOR') return [];
    const items = [];
    for (const slot of allActivitySlots) {
        if (slot.scheduling_mode !== 'independent') continue;
        for (const [key, instructor] of activityInstructorMap.entries()) {
            const [slotId, classroomId] = key.split('|');
            if (slotId === slot.id && instructor.id === selectedInstructorId) {
                items.push({ slot, classroom_id: classroomId, classroom_name: ... });
            }
        }
    }
    return items;
});
```

### Loader (รวมเป็นตัวเดียว)

```ts
async function loadAllSemesterData() {
    if (!selectedSemesterId) {
        allEntries = [];
        allCourses = [];
        allActivitySlots = [];
        occupancyEntries = [];
        courseTeamsMap = new Map();
        activityInstructorMap = new Map();
        return;
    }

    try {
        // Stage A — parallel
        const [coursesRes, slotsRes, entriesRes, occRes] = await Promise.all([
            listClassroomCourses({ semesterId: selectedSemesterId }),
            listActivitySlots({ semester_id: selectedSemesterId }),
            listTimetableEntries({
                academic_semester_id: selectedSemesterId
                // no classroom_id / instructor_id → semester-wide
            }),
            getTimetableOccupancy(selectedSemesterId)
        ]);
        allCourses = coursesRes.data;
        allActivitySlots = slotsRes.data;
        allEntries = entriesRes.data;
        occupancyEntries = occRes.data;
        if (typeof entriesRes.current_seq === 'number') {
            setInitialSeq(entriesRes.current_seq);
        }

        // Stage B — parallel (หลัง A)
        await Promise.all([
            loadAllCourseTeams(allCourses.map(c => c.id)),
            loadAllSlotAssignments(
                allActivitySlots.filter(s => s.scheduling_mode === 'independent')
            )
        ]);
    } catch (e) {
        toast.error('โหลดข้อมูลตารางไม่สำเร็จ');
    }
}

async function loadAllSlotAssignments(slots: ActivitySlot[]) {
    const newMap = new Map<string, { id: string; name: string }>();
    await Promise.all(slots.map(async slot => {
        try {
            const res = await listSlotClassroomAssignments(slot.id);
            for (const a of res.data ?? []) {
                newMap.set(`${slot.id}|${a.classroom_id}`, {
                    id: a.instructor_id,
                    name: a.instructor_name ?? ''
                });
            }
        } catch { /* skip */ }
    }));
    activityInstructorMap = newMap;
}
```

### $effect cleanup

```ts
// ลบทิ้ง — ไม่ต้องโหลดใหม่ตอน switch view
$effect(() => {
    if (selectedSemesterId) {
        if (CLASSROOM && selectedClassroomId || INSTRUCTOR && selectedInstructorId) {
            loadCourses();      // ลบ
            loadTimetable();    // ลบ
        }
    }
});

// เก็บไว้แค่ load semester data
$effect(() => {
    if (selectedSemesterId) {
        loadAllSemesterData();
    }
});

// ลบ — rawTeamEntries → displayEntries จัดการ ghost
$effect(() => {
    void showTeamGhosts;
    if (viewMode === 'INSTRUCTOR' && rawTeamEntries.length > 0) {
        timetableEntries = ...
    }
});
```

---

## Replacement guide (79 references)

### Rule

| Pattern | ใช้อะไร |
|---|---|
| Display read (cell render iteration, search by day/period) | `displayEntries` |
| Lookup by ID (snapshot/optimistic) | `allEntries` |
| WS patch / optimistic write | `allEntries = ...` |
| Sidebar render (vue) | `displayCourses` / `displaySidebarSlots` |
| Course lookup by ID | `allCourses` |

### ตัวอย่าง

```ts
// Before
timetableEntries = [...timetableEntries, tempEntry];
if (viewMode === 'INSTRUCTOR' && (tempEntry.instructor_ids ?? []).includes(selectedInstructorId)) {
    rawTeamEntries = [...rawTeamEntries, tempEntry];
}

// After
allEntries = [...allEntries, tempEntry];
// displayEntries auto-update (ไม่ต้อง dual-write)
```

```ts
// Before
function getEntryForSlot(day, periodId) {
    return timetableEntries.find(e =>
        e.day_of_week === day && e.period_id === periodId);
}

// After
function getEntryForSlot(day, periodId) {
    return displayEntries.find(e =>
        e.day_of_week === day && e.period_id === periodId);
}
```

```ts
// Before — lookup by ID for optimistic
const entry = timetableEntries.find(e => e.id === entryId)
    ?? rawTeamEntries.find(e => e.id === entryId);

// After
const entry = allEntries.find(e => e.id === entryId);
```

```ts
// Before — applyPatchToState
function applyPatchToState(patch) {
    const updateEntries = (fn) => {
        timetableEntries = fn(timetableEntries);
        rawTeamEntries = fn(rawTeamEntries);
    };
    updateEntries(arr => [...arr, patch.entry]);
}

// After
function applyPatchToState(patch) {
    allEntries = [...allEntries, patch.entry];
    // displayEntries auto-update
}
```

---

## Backend — ไม่ต้องแก้

3 endpoints รองรับ semester-only filter อยู่แล้ว:

- `GET /api/academic/timetable?academic_semester_id=X` (ไม่ส่ง classroom_id/instructor_id)
- `GET /api/academic/planning/courses?semester_id=X` (ไม่ส่ง classroom_id/instructor_id)
- `GET /api/academic/activity-slots?semester_id=X`

หมายเหตุ: `listClassroomCourses` ปัจจุบันถ้าไม่มี filter → return empty (line 118-121 ของ backend handler). แต่ถ้าส่ง `semester_id` → returns ทุก course ของ semester. ✓ ใช้ได้

---

## Edge cases

| Case | วิธีจัดการ |
|---|---|
| User เปลี่ยน semester | re-load all (เหมือนเปิดหน้าใหม่) |
| WS reconnect หลัง offline | replay (existing seq tracking) → patch allEntries |
| Course/Slot เพิ่มใหม่ระหว่าง session | ปัจจุบันไม่มี WS event สำหรับ course CRUD → ต้อง refresh manual |
| `CourseTeamChanged` event | ปัจจุบัน trigger `refreshTrigger` (full reload) — ใช้ต่อ |
| Instructor assignment ของ slot เปลี่ยน | ไม่มี WS event — refresh manual หรือเพิ่ม event ใหม่ |

---

## ลำดับการทำ

1. **เพิ่ม state ใหม่** — `allEntries`, `allCourses`, `allActivitySlots`
2. **เพิ่ม derived** — `displayEntries`, `displayCourses`, `displaySidebarSlots`, `displayInstructorActivityItems`
3. **รวม loader** — `loadTimetable + loadCourses + loadSidebarActivitySlots` → `loadAllSemesterData`
4. **แก้ $effect** — ตัด trigger ที่โหลดใหม่ตอน switch view
5. **แก้ cell render** — ใช้ `displayEntries`
6. **แก้ optimistic + WS patch** — เขียนไปที่ `allEntries`
7. **ลบ `rawTeamEntries`** + $effect ที่เกี่ยวข้อง
8. **svelte-check** — ตรวจ type errors → fix iteratively
9. **Manual test** — โหลดหน้า, switch view, drop, ghost toggle, WS sync
10. **Commit**

---

## ความเสี่ยงและการ rollback

⚠️ **Breaking change** — โค้ด 79 ที่ใช้ `timetableEntries` ต้อง audit ว่าเป็น `allEntries` (lookup) หรือ `displayEntries` (render). svelte-check จะจับได้ส่วนใหญ่ — แต่ runtime bugs อาจเหลือ

⚠️ **Performance edge case** — โรงเรียนใหญ่มาก (200+ ห้อง, 10000+ entries) อาจถึง 20+ MB → memory pressure บน mobile

⚠️ **Initial latency** — 1-2 วินาที load (user ยอมรับแล้ว)

**Rollback strategy**: refactor เป็น 1 commit ใหญ่ — ถ้าพังให้ revert ทั้ง commit. ก่อน push, test พฤติกรรมหลักทุกอย่าง:
- เปลี่ยน semester
- เปลี่ยน CLASSROOM/INSTRUCTOR view
- Drop CREATE/MOVE/SWAP/REPLACE
- WS sync (เปิด 2 tab)
- Toggle ghost cells
- Edit instructor (popover)

---

## Effort

~2-3 ชั่วโมง implement + 30 นาที test

---

## Status

- 📅 สร้างแผน: 2026-05-09
- 🟡 รอ implement
- ผู้เกี่ยวข้อง: kruakemaths

## Reference

- Phase 2 architecture commits: `82a4db1b` (Stage 1), `d4a6bdf7` (Stage 2), `2cdcd682` (Stage 3), `2ef73a08` (Stage 4-6), `08cb8909` (Stage 7-9), `e1cfc4dc` (timeout/block fixes), `0289e1be` (activity instructor fix)
- ไฟล์หลัก: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`
