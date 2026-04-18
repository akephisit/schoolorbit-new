# Team Teaching + Activity Instructor Hiding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two junction tables (`timetable_entry_instructors`, `classroom_course_instructors`) so team teaching works and teachers can hide themselves from synchronized activity entries without affecting classroom timetables.

**Architecture:** Source tables (`classroom_course_instructors`, `activity_slot_instructors`, `activity_slot_classroom_assignments`) define "who should teach"; `timetable_entry_instructors` stores "who is actually scheduled for this specific entry" — populated on entry creation by copying from source, editable per-entry via new endpoints.

**Tech Stack:** Rust Axum (backend), PostgreSQL + sqlx, SvelteKit (frontend), TypeScript.

**Verification note:** This codebase does not use a runtime test framework. Verify backend with `cargo check` from `backend-school/`, frontend with `npx svelte-check --threshold error` from `frontend-school/`, and manual walkthrough per the spec's verification section.

---

## Phase 1: Database Migrations

### Task 1: Create Junction Tables Migration

**Files:**
- Create: `backend-school/migrations/076_team_teaching_junction.sql`

- [ ] **Step 1: Create the migration file**

Write `backend-school/migrations/076_team_teaching_junction.sql`:

```sql
-- ============================================
-- Junction tables: ครูต่อ timetable entry + ครูต่อ classroom_course (team teaching)
-- ============================================

CREATE TABLE IF NOT EXISTS timetable_entry_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_id UUID NOT NULL REFERENCES academic_timetable_entries(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_entry_instructor UNIQUE (entry_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_tei_entry ON timetable_entry_instructors(entry_id);
CREATE INDEX IF NOT EXISTS idx_tei_instructor ON timetable_entry_instructors(instructor_id);

COMMENT ON TABLE timetable_entry_instructors IS 'ครูที่ schedule จริงใน timetable entry (ต่อ entry × ครู)';

CREATE TABLE IF NOT EXISTS classroom_course_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    classroom_course_id UUID NOT NULL REFERENCES classroom_courses(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_course_instructor UNIQUE (classroom_course_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_cci_course ON classroom_course_instructors(classroom_course_id);
CREATE INDEX IF NOT EXISTS idx_cci_instructor ON classroom_course_instructors(instructor_id);

COMMENT ON TABLE classroom_course_instructors IS 'ครูของ classroom_course (รองรับ team teaching)';
```

- [ ] **Step 2: Commit**

```bash
git add backend-school/migrations/076_team_teaching_junction.sql
git commit -m "feat(db): migration 076 junction tables for team teaching + entry instructors"
```

---

### Task 2: Create Data Population Migration

**Files:**
- Create: `backend-school/migrations/077_populate_junction.sql`

- [ ] **Step 1: Create the migration file**

Write `backend-school/migrations/077_populate_junction.sql`:

```sql
-- ============================================
-- Populate junction tables จากข้อมูลเดิม
-- ============================================

-- 1. classroom_courses.primary_instructor_id → classroom_course_instructors
INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
SELECT id, primary_instructor_id, 'primary'
FROM classroom_courses
WHERE primary_instructor_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- 2. Regular course entries → timetable_entry_instructors
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, cc.primary_instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN classroom_courses cc ON te.classroom_course_id = cc.id
WHERE cc.primary_instructor_id IS NOT NULL
  AND te.is_active = true
ON CONFLICT DO NOTHING;

-- 3. Synchronized activity entries (copy ทุก slot_instructor เข้าทุก entry)
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asi.user_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slots asl ON te.activity_slot_id = asl.id
JOIN activity_slot_instructors asi ON asi.slot_id = asl.id
WHERE asl.scheduling_mode = 'synchronized'
  AND te.is_active = true
ON CONFLICT DO NOTHING;

-- 4. Independent activity entries
INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
SELECT te.id, asca.instructor_id, 'primary'
FROM academic_timetable_entries te
JOIN activity_slot_classroom_assignments asca
  ON asca.slot_id = te.activity_slot_id AND asca.classroom_id = te.classroom_id
WHERE te.is_active = true
ON CONFLICT DO NOTHING;
```

- [ ] **Step 2: Commit**

```bash
git add backend-school/migrations/077_populate_junction.sql
git commit -m "feat(db): migration 077 populate junction tables from existing sources"
```

---

## Phase 2: Backend Models

### Task 3: Add CourseInstructor Model

**Files:**
- Modify: `backend-school/src/modules/academic/models/course_planning.rs`

- [ ] **Step 1: Open the file and find the end of struct declarations**

Read `backend-school/src/modules/academic/models/course_planning.rs` to find where existing structs are defined.

- [ ] **Step 2: Append the new models at end of file**

Add to `backend-school/src/modules/academic/models/course_planning.rs`:

```rust
// ==========================================
// Classroom Course Instructors (team teaching)
// ==========================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CourseInstructor {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub instructor_id: Uuid,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub instructor_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddCourseInstructorRequest {
    pub instructor_id: Uuid,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCourseInstructorRoleRequest {
    pub role: String,
}
```

Note: if `Uuid`, `Serialize`, `Deserialize`, `chrono` are not imported at top of file, add them. Example at top:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output (no errors).

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/models/course_planning.rs
git commit -m "feat(backend): CourseInstructor model for team teaching"
```

---

### Task 4: Update TimetableEntry Model — instructor_names

**Files:**
- Modify: `backend-school/src/modules/academic/models/timetable.rs`

- [ ] **Step 1: Find the TimetableEntry struct**

In `backend-school/src/modules/academic/models/timetable.rs` find the line:

```rust
pub instructor_name: Option<String>,
```

- [ ] **Step 2: Replace with Vec<String>**

Change it to:

```rust
#[sqlx(default)]
#[serde(skip_serializing_if = "Option::is_none")]
pub instructor_names: Option<Vec<String>>,

// Keep for backward-compat UI display (first name). Populated from instructor_names[0].
#[sqlx(default)]
#[serde(skip_serializing_if = "Option::is_none")]
pub instructor_name: Option<String>,
```

Rationale: `instructor_name` (singular) is still used by some UI rendering paths; deriving it from `instructor_names[0]` keeps existing UI working while new UI uses the array.

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/models/timetable.rs
git commit -m "feat(backend): TimetableEntry add instructor_names array"
```

---

## Phase 3: Backend — Entry Creation Populates Junction

### Task 5: Add Helper Function to Copy Instructors into Junction

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Locate the file's top-level imports and any existing helpers**

Read `backend-school/src/modules/academic/handlers/timetable.rs` top 30 lines to confirm imports include `sqlx`, `Uuid`.

- [ ] **Step 2: Add the helper function near the top of the impl section (before `create_timetable_entry`)**

Insert this function into `backend-school/src/modules/academic/handlers/timetable.rs` before the first `pub async fn` handler:

```rust
/// Populate timetable_entry_instructors from the source table for a newly-created entry.
/// Accepts either an executor reference or a transaction via generic bounds.
async fn populate_entry_instructors(
    executor: impl sqlx::PgExecutor<'_> + Copy,
    entry_id: Uuid,
    classroom_course_id: Option<Uuid>,
    activity_slot_id: Option<Uuid>,
    classroom_id: Uuid,
) -> Result<(), sqlx::Error> {
    if let Some(cc_id) = classroom_course_id {
        sqlx::query(
            "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
             SELECT $1, instructor_id, role FROM classroom_course_instructors
             WHERE classroom_course_id = $2
             ON CONFLICT DO NOTHING"
        )
        .bind(entry_id)
        .bind(cc_id)
        .execute(executor)
        .await?;
        return Ok(());
    }

    if let Some(slot_id) = activity_slot_id {
        // Check scheduling_mode
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT scheduling_mode FROM activity_slots WHERE id = $1"
        )
        .bind(slot_id)
        .fetch_optional(executor)
        .await?;

        match mode.as_deref() {
            Some("independent") => {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, instructor_id, 'primary'
                     FROM activity_slot_classroom_assignments
                     WHERE slot_id = $2 AND classroom_id = $3
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id)
                .bind(slot_id)
                .bind(classroom_id)
                .execute(executor)
                .await?;
            }
            _ => {
                // synchronized or null → copy all slot_instructors
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, user_id, 'primary'
                     FROM activity_slot_instructors
                     WHERE slot_id = $2
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id)
                .bind(slot_id)
                .execute(executor)
                .await?;
            }
        }
    }

    Ok(())
}
```

Note: sqlx::PgExecutor impls for both `&PgPool` and `&mut PgConnection`; passing `&*tx` inside a transaction and `&pool` outside both work.

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "feat(backend): helper populate_entry_instructors"
```

---

### Task 6: Call Helper in create_timetable_entry

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Find create_timetable_entry function**

Search for `pub async fn create_timetable_entry` in `backend-school/src/modules/academic/handlers/timetable.rs`. Locate the line after the `INSERT INTO academic_timetable_entries ... RETURNING *` query returns `entry`.

- [ ] **Step 2: Add the populate call after successful INSERT**

After the INSERT returns the inserted row (the line starting `let entry = sqlx::query_as::<_, TimetableEntry>(`), and before the final `Ok(...)` response, add:

```rust
// Populate junction from source tables
if let Err(e) = populate_entry_instructors(
    &pool,
    entry.id,
    entry.classroom_course_id,
    entry.activity_slot_id,
    entry.classroom_id,
).await {
    eprintln!("Failed to populate entry instructors: {}", e);
}
```

(Non-fatal log: the entry exists; missing instructors can be re-synced later.)

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "feat(backend): create_timetable_entry populates junction"
```

---

### Task 7: Call Helper in create_batch_timetable_entries

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Find the batch INSERT loop**

Locate `pub async fn create_batch_timetable_entries`. Inside the loop that `INSERT INTO academic_timetable_entries ...` using `&mut *tx`, find where each row is inserted (uses `.execute(&mut *tx)` on the INSERT). The INSERT uses `ON CONFLICT DO NOTHING` and doesn't RETURNING id — we need the inserted id.

- [ ] **Step 2: Change batch INSERT to RETURNING id**

Modify the batch INSERT to return id. Find the query (around line 940-970) and change `.execute(&mut *tx)` with `ON CONFLICT DO NOTHING` to return the inserted id:

```rust
let inserted_id: Option<Uuid> = sqlx::query_scalar(
    r#"
    INSERT INTO academic_timetable_entries (
        id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
        entry_type, title, is_active, created_by, updated_by,
        classroom_course_id, note, activity_slot_id
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $9, $10, $11, $12)
    ON CONFLICT DO NOTHING
    RETURNING id
    "#
)
.bind(Uuid::new_v4())
.bind(classroom_id)
.bind(payload.academic_semester_id)
.bind(&payload.day_of_week)
.bind(period_id)
.bind(payload.room_id)
.bind(&entry_type)
.bind(&title)
.bind(user_id)
.bind(classroom_course_id)
.bind(&payload.note)
.bind(payload.activity_slot_id)
.fetch_optional(&mut *tx)
.await
.map_err(|e| {
    eprintln!("Failed to batch insert for classroom {}: {}", classroom_id, e);
    AppError::InternalServerError("Failed to batch create entries".to_string())
})?;

if let Some(new_entry_id) = inserted_id {
    populate_entry_instructors(
        &mut *tx,
        new_entry_id,
        classroom_course_id,
        payload.activity_slot_id,
        *classroom_id,
    )
    .await
    .map_err(|e| {
        eprintln!("Failed to populate batch instructors: {}", e);
        AppError::InternalServerError("Failed to populate instructors".to_string())
    })?;
}
```

Remove the old `let result = sqlx::query(...)...execute()` block and its error handling — the block above replaces it.

Note on helper signature: `populate_entry_instructors` takes `impl sqlx::PgExecutor<'_> + Copy`. `&mut *tx` is not `Copy`. Change the helper's executor parameter to `impl sqlx::Acquire<'_> + Send` OR make two overloads. Simpler: change helper signature to accept `&sqlx::PgPool` and for the batch case execute queries inline without the helper. If you change the helper, adjust Task 5 and Task 6 accordingly.

**Simplest fix:** Keep helper as `&PgPool`-only. In batch path, inline the same logic directly using `&mut *tx`:

```rust
if let Some(new_entry_id) = inserted_id {
    if let Some(cc_id) = classroom_course_id {
        sqlx::query(
            "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
             SELECT $1, instructor_id, role FROM classroom_course_instructors
             WHERE classroom_course_id = $2 ON CONFLICT DO NOTHING"
        ).bind(new_entry_id).bind(cc_id).execute(&mut *tx).await
          .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else if let Some(slot_id) = payload.activity_slot_id {
        let mode: Option<String> = sqlx::query_scalar(
            "SELECT scheduling_mode FROM activity_slots WHERE id = $1"
        ).bind(slot_id).fetch_optional(&mut *tx).await.ok().flatten();
        if mode.as_deref() == Some("independent") {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, instructor_id, 'primary'
                 FROM activity_slot_classroom_assignments
                 WHERE slot_id = $2 AND classroom_id = $3 ON CONFLICT DO NOTHING"
            ).bind(new_entry_id).bind(slot_id).bind(classroom_id).execute(&mut *tx).await
              .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        } else {
            sqlx::query(
                "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                 SELECT $1, user_id, 'primary' FROM activity_slot_instructors
                 WHERE slot_id = $2 ON CONFLICT DO NOTHING"
            ).bind(new_entry_id).bind(slot_id).execute(&mut *tx).await
              .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        }
    }
}
```

Adjust helper in Task 5 to take `&sqlx::PgPool` instead of generic executor to avoid this complication:

```rust
async fn populate_entry_instructors(
    pool: &sqlx::PgPool,
    entry_id: Uuid,
    classroom_course_id: Option<Uuid>,
    activity_slot_id: Option<Uuid>,
    classroom_id: Uuid,
) -> Result<(), sqlx::Error> { /* same body but .execute(pool) */ }
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "feat(backend): create_batch_timetable_entries populates junction"
```

---

## Phase 4: Backend — Timetable Query

### Task 8: Update SELECT to JOIN junction + ARRAY_AGG instructor_names

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Locate the list_timetable_entries SELECT**

Find `pub async fn list_timetable_entries` in `backend-school/src/modules/academic/handlers/timetable.rs`. Locate the SELECT string literal.

- [ ] **Step 2: Rewrite the SELECT to use the junction**

Replace the current instructor_name `CASE WHEN ... END AS instructor_name` expression and related JOINs with an aggregated subquery approach. Change the full SELECT to:

```sql
SELECT
    te.*,
    s.code   AS subject_code,
    s.name_th AS subject_name_th,
    (SELECT ARRAY_AGG(concat(u2.first_name, ' ', u2.last_name) ORDER BY tei2.role, tei2.created_at)
     FROM timetable_entry_instructors tei2
     JOIN users u2 ON u2.id = tei2.instructor_id
     WHERE tei2.entry_id = te.id) AS instructor_names,
    (SELECT concat(u3.first_name, ' ', u3.last_name)
     FROM timetable_entry_instructors tei3
     JOIN users u3 ON u3.id = tei3.instructor_id
     WHERE tei3.entry_id = te.id
     ORDER BY tei3.role, tei3.created_at
     LIMIT 1) AS instructor_name,
    cr.name  AS classroom_name,
    r.code   AS room_code,
    ap.name  AS period_name,
    ap.start_time,
    ap.end_time,
    asl.name AS activity_slot_name,
    asl.activity_type AS activity_type,
    asl.scheduling_mode AS activity_scheduling_mode
FROM academic_timetable_entries te
LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
LEFT JOIN subjects s ON cc.subject_id = s.id
JOIN class_rooms cr ON te.classroom_id = cr.id
JOIN academic_periods ap ON te.period_id = ap.id
LEFT JOIN rooms r ON te.room_id = r.id
LEFT JOIN activity_slots asl ON te.activity_slot_id = asl.id
WHERE te.is_active = true
```

Remove the old `LEFT JOIN users u ON cc.primary_instructor_id = u.id`, `LEFT JOIN activity_slot_classroom_assignments asca ...`, and `LEFT JOIN users u2 ON asca.instructor_id = u2.id` (they're now unused).

- [ ] **Step 3: Update the INSTRUCTOR view filter**

Find the block that appends instructor_id filter (the one with `activity_slot_instructors` + `asca.instructor_id`). Replace with:

```rust
if let Some(_) = query.instructor_id {
    idx += 1;
    sql.push_str(&format!(
        " AND EXISTS (SELECT 1 FROM timetable_entry_instructors tei WHERE tei.entry_id = te.id AND tei.instructor_id = ${idx})"
    ));
}
```

- [ ] **Step 4: Remove the old references to u, u2, asca**

Search the function for `AS instructor_name,` remnants, `u.first_name`, `u2.first_name`, `asca.` — remove those that refer to the removed JOINs.

- [ ] **Step 5: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "feat(backend): list_timetable_entries uses junction for instructor_names"
```

---

### Task 9: Update RETURNING Clauses in create_timetable_entry

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Find RETURNING in create_timetable_entry**

Locate the INSERT ... RETURNING in `create_timetable_entry`. It returns `NULL::TEXT AS ... instructor_name ... NULL::TEXT AS activity_scheduling_mode` today.

- [ ] **Step 2: Add instructor_names to RETURNING**

Add `NULL::TEXT[] AS instructor_names` to the RETURNING list alongside the existing `NULL::TEXT AS instructor_name`. After change:

```rust
RETURNING *, NULL::TEXT AS subject_code, NULL::TEXT AS subject_name_th,
          NULL::TEXT[] AS instructor_names,
          NULL::TEXT AS instructor_name, NULL::TEXT AS classroom_name,
          NULL::TEXT AS room_code, NULL::TEXT AS period_name,
          NULL::TIME AS start_time, NULL::TIME AS end_time,
          NULL::TEXT AS activity_slot_name, NULL::TEXT AS activity_type,
          NULL::TEXT AS activity_scheduling_mode
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "feat(backend): RETURNING clause adds instructor_names placeholder"
```

---

## Phase 5: Backend — Unified Conflict Check

### Task 10: Refactor Instructor Conflict Check to Use Junction

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`

- [ ] **Step 1: Find validate_timetable_entry**

Locate `async fn validate_timetable_entry`. Find the blocks that check instructor conflicts via `cc.primary_instructor_id`, `activity_slot_instructors`, and `asca.instructor_id`.

- [ ] **Step 2: Replace with unified junction-based check**

Replace the three separate instructor conflict checks with a single check that works for both classroom_course and activity entries. The unified query finds candidate instructors for the incoming payload, then checks if any of them has another entry in the same slot:

```rust
// Determine candidate instructor ids based on payload source
let candidate_instructors: Vec<Uuid> = if let Some(cc_id) = payload.classroom_course_id {
    sqlx::query_scalar(
        "SELECT instructor_id FROM classroom_course_instructors WHERE classroom_course_id = $1"
    )
    .bind(cc_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
} else if let Some(slot_id) = payload.activity_slot_id {
    // Check slot mode
    let mode: Option<String> = sqlx::query_scalar(
        "SELECT scheduling_mode FROM activity_slots WHERE id = $1"
    ).bind(slot_id).fetch_optional(pool).await.ok().flatten();
    if mode.as_deref() == Some("independent") {
        if let Some(cls_id) = payload.classroom_id {
            sqlx::query_scalar(
                "SELECT instructor_id FROM activity_slot_classroom_assignments
                 WHERE slot_id = $1 AND classroom_id = $2"
            ).bind(slot_id).bind(cls_id).fetch_all(pool).await.unwrap_or_default()
        } else { Vec::new() }
    } else {
        sqlx::query_scalar(
            "SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1"
        ).bind(slot_id).fetch_all(pool).await.unwrap_or_default()
    }
} else {
    Vec::new()
};

if !candidate_instructors.is_empty() {
    let conflict_instructors: Vec<(String,)> = sqlx::query_as(
        r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name)
           FROM academic_timetable_entries te
           JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
           JOIN users u ON u.id = tei.instructor_id
           WHERE tei.instructor_id = ANY($1)
             AND te.day_of_week = $2
             AND te.period_id = $3
             AND te.is_active = true"#
    )
    .bind(&candidate_instructors)
    .bind(&payload.day_of_week)
    .bind(payload.period_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (name,) in &conflict_instructors {
        conflicts.push(ConflictInfo {
            conflict_type: "INSTRUCTOR_CONFLICT".to_string(),
            message: format!("{} มีสอนในคาบนี้อยู่แล้ว", name),
            existing_entry: None,
        });
    }
}
```

Delete the three old blocks (around lines 500-640 covering `course_info` checks, `activity_slot_instructors`, and `asca.instructor_id`). Keep the **classroom conflict** check and **room conflict** check — only replace the instructor checks.

- [ ] **Step 3: Also update batch instructor conflict check**

In `create_batch_timetable_entries` find the "Check slot instructor conflicts" block. Change the source lookup:

```rust
// 2. Check candidate instructor conflicts via junction
let candidate_instructors: Vec<Uuid> = if let Some(slot_id) = payload.activity_slot_id {
    let mode: Option<String> = sqlx::query_scalar(
        "SELECT scheduling_mode FROM activity_slots WHERE id = $1"
    ).bind(slot_id).fetch_optional(&pool).await.ok().flatten();
    if mode.as_deref() == Some("independent") {
        sqlx::query_scalar(
            "SELECT instructor_id FROM activity_slot_classroom_assignments
             WHERE slot_id = $1 AND classroom_id = ANY($2)"
        ).bind(slot_id).bind(&payload.classroom_ids).fetch_all(&pool).await.unwrap_or_default()
    } else {
        sqlx::query_scalar(
            "SELECT user_id FROM activity_slot_instructors WHERE slot_id = $1"
        ).bind(slot_id).fetch_all(&pool).await.unwrap_or_default()
    }
} else if let Some(subject_id) = payload.subject_id {
    // Regular subject batch: get instructors via classroom_courses for these classrooms
    sqlx::query_scalar(
        "SELECT DISTINCT cci.instructor_id FROM classroom_course_instructors cci
         JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
         WHERE cc.classroom_id = ANY($1) AND cc.subject_id = $2"
    ).bind(&payload.classroom_ids).bind(subject_id).fetch_all(&pool).await.unwrap_or_default()
} else { Vec::new() };

if !candidate_instructors.is_empty() {
    let instructor_conflicts: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT DISTINCT concat(u.first_name, ' ', u.last_name), COALESCE(s.name_th, te.title, '')
           FROM academic_timetable_entries te
           JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
           JOIN users u ON u.id = tei.instructor_id
           LEFT JOIN classroom_courses cc ON te.classroom_course_id = cc.id
           LEFT JOIN subjects s ON cc.subject_id = s.id
           WHERE tei.instructor_id = ANY($1)
             AND te.day_of_week = $2
             AND te.period_id = ANY($3)
             AND te.is_active = true
             AND (te.activity_slot_id IS DISTINCT FROM $4 OR te.activity_slot_id IS NULL)"#
    )
    .bind(&candidate_instructors)
    .bind(&payload.day_of_week)
    .bind(&payload.period_ids)
    .bind(payload.activity_slot_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (teacher_name, existing_subject) in &instructor_conflicts {
        conflicts.push(serde_json::json!({
            "conflict_type": "INSTRUCTOR_CONFLICT",
            "message": format!("{} มีสอน {} ในคาบนี้อยู่แล้ว", teacher_name, existing_subject)
        }));
    }
}
```

Delete the old block that queried `activity_slot_instructors` and joined `activity_slot_classroom_assignments` directly.

- [ ] **Step 4: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs
git commit -m "refactor(backend): unified conflict check via junction"
```

---

## Phase 6: Backend — New Endpoints

### Task 11: Entry Instructor Add/Remove Endpoints

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/mod.rs`

- [ ] **Step 1: Add handlers at end of timetable.rs**

Append to `backend-school/src/modules/academic/handlers/timetable.rs`:

```rust
/// POST /api/academic/timetable/:id/instructors
#[derive(Debug, serde::Deserialize)]
pub struct AddEntryInstructorRequest {
    pub instructor_id: Uuid,
    pub role: Option<String>,
}

pub async fn add_entry_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(entry_id): Path<Uuid>,
    Json(body): Json<AddEntryInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let role = body.role.unwrap_or_else(|| "primary".to_string());
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
    )
    .bind(entry_id)
    .bind(body.instructor_id)
    .bind(role)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// DELETE /api/academic/timetable/:id/instructors/:uid
pub async fn remove_entry_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((entry_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    sqlx::query("DELETE FROM timetable_entry_instructors WHERE entry_id = $1 AND instructor_id = $2")
        .bind(entry_id)
        .bind(instructor_id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // If entry has no instructors left AND it's a regular course entry, delete the entry too
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors WHERE entry_id = $1"
    ).bind(entry_id).fetch_one(&pool).await.unwrap_or(1);
    if remaining == 0 {
        // Only auto-delete regular course entries (activity entries can be left without instructors)
        let is_course: bool = sqlx::query_scalar(
            "SELECT classroom_course_id IS NOT NULL FROM academic_timetable_entries WHERE id = $1"
        ).bind(entry_id).fetch_optional(&pool).await.ok().flatten().unwrap_or(false);
        if is_course {
            sqlx::query("DELETE FROM academic_timetable_entries WHERE id = $1")
                .bind(entry_id).execute(&pool).await.ok();
        }
    }

    Ok(Json(json!({ "success": true })).into_response())
}
```

Note: ensure `json` macro and `StatusCode` are imported. If `json` is imported as `serde_json::json`, good; otherwise add `use serde_json::json;` at top.

- [ ] **Step 2: Register routes in mod.rs**

In `backend-school/src/modules/academic/mod.rs` near existing timetable routes, add:

```rust
.route("/timetable/{id}/instructors", post(handlers::timetable::add_entry_instructor))
.route("/timetable/{id}/instructors/{uid}", axum::routing::delete(handlers::timetable::remove_entry_instructor))
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs backend-school/src/modules/academic/mod.rs
git commit -m "feat(backend): endpoints add/remove entry instructor"
```

---

### Task 12: Restore Instructor Across All Slot Entries

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/timetable.rs`
- Modify: `backend-school/src/modules/academic/mod.rs`

- [ ] **Step 1: Add handler**

Append to `backend-school/src/modules/academic/handlers/timetable.rs`:

```rust
/// POST /api/academic/timetable/slots/:slot_id/instructors/:uid/restore
/// Adds the instructor back to every active entry of the slot (for the current semester).
pub async fn restore_instructor_to_slot_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let affected = sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         SELECT te.id, $2, 'primary' FROM academic_timetable_entries te
         WHERE te.activity_slot_id = $1 AND te.is_active = true
         ON CONFLICT DO NOTHING"
    )
    .bind(slot_id)
    .bind(instructor_id)
    .execute(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true, "inserted": affected.rows_affected() })).into_response())
}
```

- [ ] **Step 2: Register route in mod.rs**

Add to `backend-school/src/modules/academic/mod.rs`:

```rust
.route("/timetable/slots/{slot_id}/instructors/{uid}/restore",
       post(handlers::timetable::restore_instructor_to_slot_entries))
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/timetable.rs backend-school/src/modules/academic/mod.rs
git commit -m "feat(backend): restore_instructor_to_slot_entries endpoint"
```

---

### Task 13: Course Instructor CRUD with Primary Sync

**Files:**
- Modify: `backend-school/src/modules/academic/handlers/course_planning.rs`
- Modify: `backend-school/src/modules/academic/mod.rs`

- [ ] **Step 1: Append handlers to course_planning.rs**

Append to `backend-school/src/modules/academic/handlers/course_planning.rs`:

```rust
use crate::modules::academic::models::course_planning::{CourseInstructor, AddCourseInstructorRequest, UpdateCourseInstructorRoleRequest};

/// GET /api/academic/planning/courses/:id/instructors
pub async fn list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let rows: Vec<CourseInstructor> = sqlx::query_as(
        r#"SELECT cci.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM classroom_course_instructors cci
           JOIN users u ON u.id = cci.instructor_id
           WHERE cci.classroom_course_id = $1
           ORDER BY cci.role, cci.created_at"#
    )
    .bind(course_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "data": rows })).into_response())
}

/// POST /api/academic/planning/courses/:id/instructors
pub async fn add_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
    Json(body): Json<AddCourseInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // If inserting as primary, demote existing primary to secondary
    if role == "primary" {
        sqlx::query(
            "UPDATE classroom_course_instructors SET role = 'secondary'
             WHERE classroom_course_id = $1 AND role = 'primary'"
        ).bind(course_id).execute(&mut *tx).await
          .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "INSERT INTO classroom_course_instructors (classroom_course_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (classroom_course_id, instructor_id) DO UPDATE SET role = EXCLUDED.role"
    )
    .bind(course_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    // Sync classroom_courses.primary_instructor_id
    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// DELETE /api/academic/planning/courses/:id/instructors/:uid
pub async fn remove_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sqlx::query("DELETE FROM classroom_course_instructors WHERE classroom_course_id = $1 AND instructor_id = $2")
        .bind(course_id).bind(instructor_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// PUT /api/academic/planning/courses/:id/instructors/:uid
pub async fn update_course_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCourseInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    if body.role == "primary" {
        // Demote current primary (if it's not this user)
        sqlx::query(
            "UPDATE classroom_course_instructors SET role = 'secondary'
             WHERE classroom_course_id = $1 AND role = 'primary' AND instructor_id <> $2"
        ).bind(course_id).bind(instructor_id).execute(&mut *tx).await
          .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }
    sqlx::query(
        "UPDATE classroom_course_instructors SET role = $3
         WHERE classroom_course_id = $1 AND instructor_id = $2"
    ).bind(course_id).bind(instructor_id).bind(&body.role).execute(&mut *tx).await
      .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    sync_primary_instructor(&mut *tx, course_id).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    tx.commit().await.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(Json(json!({ "success": true })).into_response())
}

/// Keep classroom_courses.primary_instructor_id in sync with the oldest primary in the junction.
async fn sync_primary_instructor(
    tx: &mut sqlx::PgConnection,
    course_id: Uuid,
) -> Result<(), sqlx::Error> {
    // Pick primary by role='primary' else oldest entry else NULL
    let chosen: Option<Uuid> = sqlx::query_scalar(
        "SELECT instructor_id FROM classroom_course_instructors
         WHERE classroom_course_id = $1
         ORDER BY (role = 'primary') DESC, created_at ASC LIMIT 1"
    ).bind(course_id).fetch_optional(&mut *tx).await?;
    sqlx::query(
        "UPDATE classroom_courses SET primary_instructor_id = $1 WHERE id = $2"
    ).bind(chosen).bind(course_id).execute(&mut *tx).await?;
    Ok(())
}
```

Note: sync_primary_instructor uses the single promoted primary if any exists; otherwise promotes the oldest member (regardless of role). This keeps the denormalized column meaningful when team has only secondaries.

- [ ] **Step 2: Register routes in mod.rs**

Add to `backend-school/src/modules/academic/mod.rs`:

```rust
.route("/planning/courses/{id}/instructors",
       get(handlers::course_planning::list_course_instructors)
       .post(handlers::course_planning::add_course_instructor))
.route("/planning/courses/{id}/instructors/{uid}",
       axum::routing::delete(handlers::course_planning::remove_course_instructor)
       .put(handlers::course_planning::update_course_instructor_role))
```

- [ ] **Step 3: Verify compilation**

Run from `backend-school/`: `cargo check 2>&1 | grep "^error"`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add backend-school/src/modules/academic/handlers/course_planning.rs backend-school/src/modules/academic/mod.rs
git commit -m "feat(backend): course instructor CRUD with primary sync"
```

---

## Phase 7: Frontend API

### Task 14: Add instructor_names to TimetableEntry Interface

**Files:**
- Modify: `frontend-school/src/lib/api/timetable.ts`

- [ ] **Step 1: Locate TimetableEntry interface**

Find `export interface TimetableEntry` in `frontend-school/src/lib/api/timetable.ts`. It currently includes `instructor_name?: string;`.

- [ ] **Step 2: Add instructor_names field**

Add below `instructor_name`:

```typescript
    instructor_names?: string[];
```

(Keep `instructor_name` for backward compat with existing rendering code.)

- [ ] **Step 3: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 4: Commit**

```bash
git add frontend-school/src/lib/api/timetable.ts
git commit -m "feat(frontend): TimetableEntry add instructor_names array"
```

---

### Task 15: Add New API Client Functions

**Files:**
- Modify: `frontend-school/src/lib/api/timetable.ts`
- Modify: `frontend-school/src/lib/api/academic.ts`

- [ ] **Step 1: Append entry-instructor endpoints to timetable.ts**

Append to `frontend-school/src/lib/api/timetable.ts`:

```typescript
export const addEntryInstructor = async (
    entryId: string,
    instructorId: string,
    role: 'primary' | 'secondary' = 'secondary'
) => {
    return await fetchApi(`/api/academic/timetable/${entryId}/instructors`, {
        method: 'POST',
        body: JSON.stringify({ instructor_id: instructorId, role })
    });
};

export const removeEntryInstructor = async (entryId: string, instructorId: string) => {
    return await fetchApi(`/api/academic/timetable/${entryId}/instructors/${instructorId}`, {
        method: 'DELETE'
    });
};

export const restoreInstructorToSlot = async (slotId: string, instructorId: string) => {
    return await fetchApi(`/api/academic/timetable/slots/${slotId}/instructors/${instructorId}/restore`, {
        method: 'POST'
    });
};
```

- [ ] **Step 2: Append course-instructor endpoints to academic.ts**

Append to `frontend-school/src/lib/api/academic.ts`:

```typescript
export interface CourseInstructor {
    id: string;
    classroom_course_id: string;
    instructor_id: string;
    role: 'primary' | 'secondary';
    instructor_name?: string;
}

export const listCourseInstructors = async (courseId: string): Promise<{ data: CourseInstructor[] }> => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors`);
};

export const addCourseInstructor = async (
    courseId: string,
    instructorId: string,
    role: 'primary' | 'secondary' = 'secondary'
) => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors`, {
        method: 'POST',
        body: JSON.stringify({ instructor_id: instructorId, role })
    });
};

export const removeCourseInstructor = async (courseId: string, instructorId: string) => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors/${instructorId}`, {
        method: 'DELETE'
    });
};

export const updateCourseInstructorRole = async (
    courseId: string,
    instructorId: string,
    role: 'primary' | 'secondary'
) => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors/${instructorId}`, {
        method: 'PUT',
        body: JSON.stringify({ role })
    });
};
```

- [ ] **Step 3: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 4: Commit**

```bash
git add frontend-school/src/lib/api/timetable.ts frontend-school/src/lib/api/academic.ts
git commit -m "feat(frontend): API clients for entry + course instructors"
```

---

## Phase 8: Frontend — Course Planning UI

### Task 16: Team Teaching UI in Planning Page

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte`

- [ ] **Step 1: Read the existing page to find where instructor is currently displayed**

Read `frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte`. Look for how `primary_instructor_id` is edited today (likely a Select dropdown per assignment).

- [ ] **Step 2: Add state + imports**

At top of `<script lang="ts">`, add imports:

```typescript
import {
    listCourseInstructors,
    addCourseInstructor,
    removeCourseInstructor,
    updateCourseInstructorRole,
    type CourseInstructor
} from '$lib/api/academic';
```

Add state:

```typescript
let teamInstructorsMap = $state<Record<string, CourseInstructor[]>>({});
let showTeamDialog = $state(false);
let teamDialogCourseId = $state('');
let teamDialogSelectedInstructor = $state('');
let teamDialogRole = $state<'primary' | 'secondary'>('secondary');

async function loadTeamInstructors(courseId: string) {
    try {
        const res = await listCourseInstructors(courseId);
        teamInstructorsMap[courseId] = res.data ?? [];
        teamInstructorsMap = { ...teamInstructorsMap };
    } catch { /* ignore */ }
}

function openTeamDialog(courseId: string) {
    teamDialogCourseId = courseId;
    teamDialogSelectedInstructor = '';
    teamDialogRole = 'secondary';
    showTeamDialog = true;
    loadTeamInstructors(courseId);
}

async function handleAddTeamInstructor() {
    if (!teamDialogSelectedInstructor) return;
    await addCourseInstructor(teamDialogCourseId, teamDialogSelectedInstructor, teamDialogRole);
    await loadTeamInstructors(teamDialogCourseId);
    teamDialogSelectedInstructor = '';
}

async function handleRemoveTeamInstructor(courseId: string, userId: string) {
    await removeCourseInstructor(courseId, userId);
    await loadTeamInstructors(courseId);
}

async function handlePromoteToPrimary(courseId: string, userId: string) {
    await updateCourseInstructorRole(courseId, userId, 'primary');
    await loadTeamInstructors(courseId);
}
```

- [ ] **Step 3: Add UI in the course card template**

In the template where each course is rendered (per classroom assignment), add a "ครูผู้สอน" section:

```svelte
<div class="flex items-center gap-2 flex-wrap">
    <span class="text-xs font-semibold text-muted-foreground">ครูผู้สอน:</span>
    {#each (teamInstructorsMap[course.id] ?? []) as instr}
        <Badge variant={instr.role === 'primary' ? 'default' : 'secondary'} class="gap-1">
            {instr.role === 'primary' ? 'หลัก' : 'ร่วม'}: {instr.instructor_name}
            {#if instr.role === 'secondary'}
                <button class="ml-1 text-[10px]" onclick={() => handlePromoteToPrimary(course.id, instr.instructor_id)} title="เปลี่ยนเป็นครูหลัก">↑</button>
            {/if}
            <button class="ml-1 text-destructive" onclick={() => handleRemoveTeamInstructor(course.id, instr.instructor_id)}>×</button>
        </Badge>
    {/each}
    <Button variant="outline" size="sm" class="h-6 text-xs" onclick={() => openTeamDialog(course.id)}>+ เพิ่มครู</Button>
</div>
```

Where courses are first rendered, call `loadTeamInstructors(course.id)` once (e.g. in a `$effect` block that iterates `courses`).

- [ ] **Step 4: Add the Add Instructor Dialog**

At end of the template:

```svelte
<Dialog.Root bind:open={showTeamDialog}>
    <Dialog.Content class="max-w-sm">
        <Dialog.Header>
            <Dialog.Title>เพิ่มครูผู้สอน</Dialog.Title>
        </Dialog.Header>
        <div class="space-y-2 py-2">
            <Label>ครู</Label>
            <Select.Root type="single" bind:value={teamDialogSelectedInstructor}>
                <Select.Trigger class="w-full">
                    {instructors.find((i) => i.id === teamDialogSelectedInstructor)?.name || 'เลือกครู'}
                </Select.Trigger>
                <Select.Content class="max-h-[280px] overflow-y-auto">
                    {#each instructors as i}
                        <Select.Item value={i.id}>{i.name}</Select.Item>
                    {/each}
                </Select.Content>
            </Select.Root>

            <Label>บทบาท</Label>
            <Select.Root type="single" bind:value={teamDialogRole}>
                <Select.Trigger class="w-full">{teamDialogRole === 'primary' ? 'ครูหลัก' : 'ครูร่วม'}</Select.Trigger>
                <Select.Content>
                    <Select.Item value="primary">ครูหลัก</Select.Item>
                    <Select.Item value="secondary">ครูร่วม</Select.Item>
                </Select.Content>
            </Select.Root>
        </div>
        <Dialog.Footer>
            <Button variant="outline" onclick={() => { showTeamDialog = false; }}>ปิด</Button>
            <Button onclick={handleAddTeamInstructor}>เพิ่ม</Button>
        </Dialog.Footer>
    </Dialog.Content>
</Dialog.Root>
```

Note: `instructors` should already be available from existing imports (lookup). If not, import `lookupStaff` as in other files.

- [ ] **Step 5: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 6: Commit**

```bash
git add "frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte"
git commit -m "feat(frontend): team teaching UI in course planning"
```

---

## Phase 9: Frontend — Timetable

### Task 17: Display instructor_names on Entry Card

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`

- [ ] **Step 1: Find where instructor_name is rendered**

Locate `entry.instructor_name || '-'` in the timetable entry card section (CLASSROOM view).

- [ ] **Step 2: Replace with instructor_names join**

Change:

```svelte
{entry.instructor_name || '-'}
```

To:

```svelte
{(entry.instructor_names && entry.instructor_names.length > 0) ? entry.instructor_names.join(', ') : (entry.instructor_name || '-')}
```

(Falls back to singular `instructor_name` if the array is missing for any reason.)

- [ ] **Step 3: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 4: Commit**

```bash
git add "frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte"
git commit -m "feat(frontend): timetable card shows multiple instructor names"
```

---

### Task 18: INSTRUCTOR View Delete Behavior — Per-Instructor

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`

- [ ] **Step 1: Add imports**

At top `<script>` imports, ensure:

```typescript
import { removeEntryInstructor, restoreInstructorToSlot } from '$lib/api/timetable';
```

- [ ] **Step 2: Update handleDeleteEntry**

Replace the existing `handleDeleteEntry` / related delete paths for INSTRUCTOR view. In the function find the INSTRUCTOR branch and replace:

```typescript
async function handleDeleteEntry(entry: TimetableEntry) {
    if (viewMode === 'INSTRUCTOR') {
        // Per-instructor removal
        if (entry.activity_slot_id) {
            const slot = sidebarActivitySlots.find((s) => s.id === entry.activity_slot_id)
                || instructorActivityItems.find((i) => i.slot.id === entry.activity_slot_id)?.slot;
            if (slot?.scheduling_mode === 'synchronized') {
                // Remove me from every entry of this slot
                if (!selectedInstructorId) return;
                // For each entry of this slot, remove junction row for me
                const entriesOfSlot = timetableEntries.filter((e) => e.activity_slot_id === entry.activity_slot_id);
                for (const e of entriesOfSlot) {
                    await removeEntryInstructor(e.id, selectedInstructorId);
                }
                toast.success('ลบครูออกจากกิจกรรมนี้แล้ว (ทุกห้อง)');
            } else {
                // Independent: one entry = one classroom; delete entry
                await deleteTimetableEntry(entry.id);
                toast.success('ลบกิจกรรมออกจากตารางสำเร็จ');
            }
        } else {
            // Regular course / team: remove this instructor from junction
            if (!selectedInstructorId) return;
            await removeEntryInstructor(entry.id, selectedInstructorId);
            toast.success('ลบคุณออกจากวิชานี้แล้ว');
        }
        if ($authStore.user) {
            sendTimetableEvent({ type: 'TableRefresh', payload: { user_id: $authStore.user.id } });
        }
        loadTimetable();
        loadSidebarActivitySlots();
        return;
    }

    // CLASSROOM view — existing behavior (keep below unchanged)
    // ... (preserve the current classroom-view logic)
}
```

Preserve the existing CLASSROOM branch code below — do not delete it.

- [ ] **Step 3: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 4: Commit**

```bash
git add "frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte"
git commit -m "feat(frontend): INSTRUCTOR view delete removes per-instructor via junction"
```

---

### Task 19: Sidebar Restore Button for Hidden Synchronized Activities

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte`

- [ ] **Step 1: Find sidebar synchronized read-only block**

Locate the sidebar block for synchronized activities (currently renders `Lock` icon + "ใช้ Batch" text).

- [ ] **Step 2: In INSTRUCTOR view, show a restore button**

Modify the sidebar to branch on viewMode. Replace the synchronized block with:

```svelte
{#if !activity.is_draggable}
    <div class="border border-dashed rounded-lg p-2.5 opacity-60 bg-gray-50 border-gray-300">
        <div class="flex justify-between items-start mb-1">
            <Badge variant="outline" class="text-[10px]">
                {ACTIVITY_TYPE_LABELS[activity.activity_type] ?? activity.activity_type}
            </Badge>
            <Badge variant="secondary" class="text-[10px]">
                {activity.scheduled_count}/{activity.max_periods} คาบ
            </Badge>
        </div>
        <h4 class="font-medium text-sm line-clamp-1 leading-tight flex items-center gap-1">
            <Lock class="w-3 h-3 shrink-0" /> {activity.name}
        </h4>
        {#if viewMode === 'INSTRUCTOR' && selectedInstructorId}
            <Button variant="outline" size="sm" class="mt-1 h-6 text-xs w-full"
                onclick={async () => {
                    await restoreInstructorToSlot(activity.id, selectedInstructorId);
                    toast.success('แสดงในตารางแล้ว');
                    loadTimetable();
                    loadSidebarActivitySlots();
                }}>
                แสดงในตาราง
            </Button>
        {:else}
            <div class="text-[10px] text-muted-foreground mt-1">จัดพร้อมกัน — ใช้ Batch</div>
        {/if}
    </div>
{/if}
```

- [ ] **Step 3: Verify types**

Run from `frontend-school/`: `npx svelte-check --threshold error 2>&1 | tail -3`
Expected: `0 ERRORS`.

- [ ] **Step 4: Commit**

```bash
git add "frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte"
git commit -m "feat(frontend): sidebar restore button for hidden synchronized activities"
```

---

## Phase 10: Verification

### Task 20: End-to-End Verification Walkthrough

**No files to modify — manual steps.**

- [ ] **Step 1: Apply migrations**

Restart backend so migrations run. Verify the tables exist:

```bash
psql "$DATABASE_URL" -c "\d timetable_entry_instructors"
psql "$DATABASE_URL" -c "\d classroom_course_instructors"
```

Expected: both tables listed with the columns defined in Task 1.

- [ ] **Step 2: Verify data population**

```sql
SELECT COUNT(*) FROM timetable_entry_instructors;
SELECT COUNT(*) FROM classroom_course_instructors;
```

Expected: non-zero counts matching existing entries/courses.

- [ ] **Step 3: Test team teaching**

- In Course Planning UI: add a primary + a secondary teacher to a classroom_course
- Drag the course onto the timetable grid
- Verify entry card shows both names: "<primary>, <secondary>"
- Switch to INSTRUCTOR view with secondary teacher → see the entry

- [ ] **Step 4: Test synchronized hide**

- Batch add a synchronized activity (e.g., ชุมนุม) to multiple classrooms
- Switch to INSTRUCTOR view with one of the slot instructors → see entries
- Click delete on one entry → confirm
- Verify: the entry still exists in CLASSROOM view of other classrooms; sidebar of this instructor shows the activity as "ซ่อนอยู่" with "แสดงในตาราง" button
- Click "แสดงในตาราง" → entries reappear in grid

- [ ] **Step 5: Test conflict**

- Drag a team-taught course where the secondary teacher already has another subject at that period
- Verify: conflict toast shows "<ครู secondary> มีสอนในคาบนี้อยู่แล้ว"
- Batch add a synchronized activity when a slot instructor has another class
- Verify: conflict toast shows

- [ ] **Step 6: Test CLASSROOM delete**

- In CLASSROOM view, delete an entry (regular course)
- Verify junction rows deleted (CASCADE):

```sql
SELECT * FROM timetable_entry_instructors WHERE entry_id = '<deleted_entry_id>';
```

Expected: empty.

- [ ] **Step 7: Test primary sync**

- In Course Planning: add primary A + secondary B
- Verify `classroom_courses.primary_instructor_id = A`
- Remove A via the UI
- Verify `classroom_courses.primary_instructor_id = B` (promoted as oldest remaining)

---

## Self-Review (after writing plan)

**Spec coverage check:**
- ✅ Migration 076 (Task 1)
- ✅ Migration 077 populate (Task 2)
- ✅ `CourseInstructor` model (Task 3)
- ✅ `TimetableEntry.instructor_names` (Task 4)
- ✅ Populate junction on create (Tasks 5-7)
- ✅ Timetable query aggregates instructor_names + INSTRUCTOR filter via junction (Task 8-9)
- ✅ Unified conflict check (Task 10)
- ✅ Entry instructor CRUD + restore (Tasks 11-12)
- ✅ Course instructor CRUD + primary sync (Task 13)
- ✅ Frontend types + API clients (Tasks 14-15)
- ✅ Planning UI (Task 16)
- ✅ Timetable display multiple names (Task 17)
- ✅ INSTRUCTOR delete per-instructor (Task 18)
- ✅ Sidebar restore (Task 19)
- ✅ Verification (Task 20)

**Delete behavior table in spec**: covered across Task 18 (INSTRUCTOR) and existing CLASSROOM logic (preserved).

**Placeholder scan:** No TBD/TODO. All code blocks complete.

**Type consistency:** `instructor_names` is `Option<Vec<String>>` (Rust) / `string[]` (TS); `role` is `primary|secondary` everywhere.
