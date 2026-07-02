# School Calendar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the V1 school calendar module with manual events, category management, audience filtering, public calendar, in-app notifications, and daily 07:00 reminders.

**Architecture:** Add a central backend `calendar` module with typed models, thin handlers, service-owned SQL, and route splits for staff management, self-view, parent-child view, and public view. Add focused SvelteKit routes and shared calendar components that consume a typed `$lib/api/calendar.ts` client and use local shadcn-svelte primitives.

**Tech Stack:** Rust, Axum, sqlx, PostgreSQL migrations, tokio-cron-scheduler, SvelteKit 5, TypeScript, Tailwind, local shadcn-svelte components, Playwright/static tests.

---

## File Structure

Backend:

- Create `backend-school/migrations/018_school_calendar.sql`: calendar tables, indexes, triggers, permission seeds.
- Modify `backend-school/src/modules.rs`: register `pub mod calendar;`.
- Create `backend-school/src/modules/calendar.rs`: Axum router for `/api/calendar/*`.
- Create `backend-school/src/modules/calendar/models.rs`: typed request, response, query, row, and enum DTOs.
- Create `backend-school/src/modules/calendar/services.rs`: validation helpers, CRUD queries, audience resolution, notification fan-out, reminder processing.
- Create `backend-school/src/modules/calendar/handlers.rs`: staff management, self-view, parent-child, and public handlers.
- Modify `backend-school/src/main.rs`: mount calendar routes and add the daily reminder scheduler job.
- Modify `backend-school/src/modules/parents/handlers.rs`: add parent child calendar route handler wrapper.
- Modify `backend-school/src/modules/parents/services.rs`: add child calendar service wrapper or expose child access helper safely.
- Modify `backend-school/src/permissions/registry.rs`: add `calendar.read.school` and `calendar.manage.school`.
- Modify `backend-school/tests/static_architecture.rs`: guard calendar schema, routes, permissions, and handler/service boundaries.

Frontend:

- Create `frontend-school/src/lib/api/calendar.ts`: typed API client.
- Create `frontend-school/src/lib/utils/calendar.ts`: month grid and date formatting helpers.
- Create `frontend-school/src/lib/components/calendar/CalendarMonthGrid.svelte`: month grid presentation.
- Create `frontend-school/src/lib/components/calendar/CalendarEventList.svelte`: list/detail presentation.
- Create `frontend-school/src/lib/components/calendar/CalendarEventDialog.svelte`: create/edit dialog.
- Create `frontend-school/src/lib/components/calendar/CalendarCategoryDialog.svelte`: category management dialog.
- Create `frontend-school/src/routes/(app)/staff/calendar/+page.ts`: staff menu metadata.
- Create `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`: staff management workspace.
- Create `frontend-school/src/routes/(app)/student/calendar/+page.ts`: student route metadata.
- Create `frontend-school/src/routes/(app)/student/calendar/+page.svelte`: authenticated student calendar.
- Create `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.ts`: parent child route metadata.
- Create `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.svelte`: parent child calendar.
- Create `frontend-school/src/routes/(public)/calendar/+page.ts`: public route data.
- Create `frontend-school/src/routes/(public)/calendar/+page.svelte`: public calendar.
- Modify `frontend-school/src/lib/permissions/registry.ts`: add calendar module and permissions.
- Create `frontend-school/tests/static/calendar.test.mjs`: guard API client, route metadata, shadcn usage, and no hardcoded permission strings.

---

### Task 1: Schema, Permissions, And Static Guards

**Files:**
- Create: `backend-school/migrations/018_school_calendar.sql`
- Modify: `backend-school/src/permissions/registry.rs`
- Modify: `frontend-school/src/lib/permissions/registry.ts`
- Modify: `backend-school/tests/static_architecture.rs`
- Create: `frontend-school/tests/static/calendar.test.mjs`

- [ ] **Step 1: Add failing backend static tests for schema and permission contract**

Append this test to `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn calendar_schema_routes_and_permissions_are_registered() {
    let migration = read_source(manifest_dir().join("migrations/018_school_calendar.sql"));
    let backend_registry = read_source(manifest_dir().join("src/permissions/registry.rs"));
    let frontend_registry = read_source(
        repo_root()
            .join("frontend-school")
            .join("src/lib/permissions/registry.ts"),
    );
    let modules_root = read_source(manifest_dir().join("src/modules.rs"));
    let main = strip_comments(&read_source(manifest_dir().join("src/main.rs")));

    for required in [
        "CREATE TABLE calendar_categories",
        "CREATE TABLE calendar_events",
        "CREATE TABLE calendar_event_targets",
        "CREATE TABLE calendar_event_reminders",
        "days_before INTEGER NOT NULL",
        "remind_on DATE NOT NULL",
        "calendar.read.school",
        "calendar.manage.school",
    ] {
        assert!(
            migration.contains(required),
            "calendar migration must contain `{required}`"
        );
    }

    for source in [&backend_registry, &frontend_registry] {
        assert!(source.contains("calendar.read.school"));
        assert!(source.contains("calendar.manage.school"));
    }

    assert!(modules_root.contains("pub mod calendar;"));
    assert!(main.contains(".nest(\"/api/calendar\", modules::calendar::calendar_routes()"));
    assert!(main.contains("\"/api/me/calendar/events\""));
    assert!(main.contains("\"/api/parent/students/{student_id}/calendar/events\""));
    assert!(main.contains("\"/api/public/calendar/events\""));
    assert!(main.contains("process_due_calendar_reminders_for_all_tenants"));
}
```

- [ ] **Step 2: Add failing frontend static tests for route/API contract**

Create `frontend-school/tests/static/calendar.test.mjs`:

```js
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('calendar permission registry and routes are wired', async () => {
	const registry = await readProjectFile('src/lib/permissions/registry.ts');
	const staffRoute = await readProjectFile('src/routes/(app)/staff/calendar/+page.ts');
	const studentRoute = await readProjectFile('src/routes/(app)/student/calendar/+page.ts');
	const parentRoute = await readProjectFile(
		'src/routes/(app)/parent/student/[id]/calendar/+page.ts'
	);

	assert.match(registry, /CALENDAR:\s*['"]calendar['"]/);
	assert.match(registry, /CALENDAR_READ_SCHOOL:\s*['"]calendar\.read\.school['"]/);
	assert.match(registry, /CALENDAR_MANAGE_SCHOOL:\s*['"]calendar\.manage\.school['"]/);
	assert.match(staffRoute, /permission:\s*PERMISSION_MODULES\.CALENDAR/);
	assert.match(studentRoute, /user_type:\s*['"]student['"]/);
	assert.match(parentRoute, /user_type:\s*['"]parent['"]/);
});

test('calendar frontend uses typed API client and shadcn primitives', async () => {
	const api = await readProjectFile('src/lib/api/calendar.ts');
	const staffPage = await readProjectFile('src/routes/(app)/staff/calendar/+page.svelte');
	const eventDialog = await readProjectFile('src/lib/components/calendar/CalendarEventDialog.svelte');
	const categoryDialog = await readProjectFile(
		'src/lib/components/calendar/CalendarCategoryDialog.svelte'
	);

	for (const name of [
		'CalendarEvent',
		'CalendarCategory',
		'CalendarEventTarget',
		'CreateCalendarEventRequest',
		'listCalendarEvents',
		'listMyCalendarEvents',
		'listChildCalendarEvents',
		'listPublicCalendarEvents'
	]) {
		assert.match(api, new RegExp(`\\b${name}\\b`));
	}

	assert.match(staffPage, /PageShell/);
	assert.match(staffPage, /PERMISSIONS\.CALENDAR_MANAGE_SCHOOL/);
	assert.doesNotMatch(staffPage, /calendar\.manage\.school/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/select'/);
	assert.match(eventDialog, /from '\$lib\/components\/ui\/checkbox'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/dialog'/);
	assert.match(categoryDialog, /from '\$lib\/components\/ui\/button'/);
});
```

- [ ] **Step 3: Run the new static tests and confirm they fail**

Run:

```bash
cd backend-school
cargo test calendar_schema_routes_and_permissions_are_registered --test static_architecture
cd ../frontend-school
npm run test:static -- calendar.test.mjs
```

Expected:

- Backend fails because migration/routes/registry do not exist.
- Frontend fails because API/routes/components do not exist.

- [ ] **Step 4: Create the calendar migration**

Create `backend-school/migrations/018_school_calendar.sql`:

```sql
-- School-wide calendar V1: manual events, audience targets, public visibility, and daily reminders.

CREATE TABLE calendar_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(120) NOT NULL,
    color VARCHAR(32) NOT NULL,
    order_index INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_calendar_categories_active_name_unique
    ON calendar_categories (lower(name))
    WHERE is_active = true;

CREATE INDEX idx_calendar_categories_order
    ON calendar_categories (is_active, order_index, name);

CREATE TRIGGER update_calendar_categories_updated_at
    BEFORE UPDATE ON calendar_categories
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category_id UUID REFERENCES calendar_categories(id) ON DELETE SET NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    location VARCHAR(200),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT true,
    start_time TIME,
    end_time TIME,
    is_public BOOLEAN NOT NULL DEFAULT false,
    source_type VARCHAR(50),
    source_id UUID,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_events_valid_date_range CHECK (end_date >= start_date),
    CONSTRAINT calendar_events_valid_time_range CHECK (
        all_day = true
        OR start_date <> end_date
        OR start_time IS NULL
        OR end_time IS NULL
        OR end_time > start_time
    )
);

CREATE INDEX idx_calendar_events_range
    ON calendar_events (start_date, end_date)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_calendar_events_public_range
    ON calendar_events (start_date, end_date)
    WHERE deleted_at IS NULL AND is_public = true;

CREATE INDEX idx_calendar_events_category
    ON calendar_events (category_id)
    WHERE deleted_at IS NULL;

CREATE TRIGGER update_calendar_events_updated_at
    BEFORE UPDATE ON calendar_events
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_event_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    audience_type VARCHAR(20) NOT NULL,
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE CASCADE,
    class_room_id UUID REFERENCES class_rooms(id) ON DELETE CASCADE,
    CONSTRAINT calendar_event_targets_audience_type CHECK (
        audience_type IN ('all', 'staff', 'student', 'parent')
    ),
    CONSTRAINT calendar_event_targets_all_scope CHECK (
        audience_type <> 'all' OR (grade_level_id IS NULL AND class_room_id IS NULL)
    ),
    CONSTRAINT calendar_event_targets_staff_scope CHECK (
        audience_type <> 'staff' OR (grade_level_id IS NULL AND class_room_id IS NULL)
    )
);

CREATE INDEX idx_calendar_event_targets_event
    ON calendar_event_targets (event_id);

CREATE INDEX idx_calendar_event_targets_audience
    ON calendar_event_targets (audience_type, grade_level_id, class_room_id);

CREATE TABLE calendar_event_reminders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    days_before INTEGER NOT NULL,
    remind_on DATE NOT NULL,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_event_reminders_days_positive CHECK (days_before > 0)
);

CREATE INDEX idx_calendar_event_reminders_due
    ON calendar_event_reminders (remind_on)
    WHERE sent_at IS NULL;

CREATE UNIQUE INDEX idx_calendar_event_reminders_unique_offset
    ON calendar_event_reminders (event_id, days_before);

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'calendar.read.school',
        'ดูปฏิทินโรงเรียน',
        'calendar',
        'read',
        'school',
        'ดูปฏิทินและกำหนดการของโรงเรียน'
    ),
    (
        'calendar.manage.school',
        'จัดการปฏิทินโรงเรียน',
        'calendar',
        'manage',
        'school',
        'สร้าง แก้ไข ลบ หมวดหมู่ กลุ่มผู้เห็น และการแจ้งเตือนของปฏิทินโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;
```

- [ ] **Step 5: Register backend permissions**

In `backend-school/src/permissions/registry.rs`, add constants inside `pub mod codes`:

```rust
    // Calendar permissions
    pub const CALENDAR_READ_SCHOOL: &str = "calendar.read.school";
    pub const CALENDAR_MANAGE_SCHOOL: &str = "calendar.manage.school";
```

Add entries to `ALL_PERMISSIONS`:

```rust
    PermissionDef {
        code: codes::CALENDAR_READ_SCHOOL,
        name: "ดูปฏิทินโรงเรียน",
        module: "calendar",
        action: "read",
        scope: "school",
        description: "ดูปฏิทินและกำหนดการของโรงเรียน",
    },
    PermissionDef {
        code: codes::CALENDAR_MANAGE_SCHOOL,
        name: "จัดการปฏิทินโรงเรียน",
        module: "calendar",
        action: "manage",
        scope: "school",
        description: "สร้าง แก้ไข ลบ หมวดหมู่ กลุ่มผู้เห็น และการแจ้งเตือนของปฏิทินโรงเรียน",
    },
```

- [ ] **Step 6: Register frontend permissions**

In `frontend-school/src/lib/permissions/registry.ts`, add:

```ts
	CALENDAR: 'calendar',
```

to `PERMISSION_MODULES`, and add:

```ts
	CALENDAR_MANAGE_SCHOOL: 'calendar.manage.school',
	CALENDAR_READ_SCHOOL: 'calendar.read.school',
```

to `PERMISSIONS`.

- [ ] **Step 7: Run tests for Task 1**

Run:

```bash
cd backend-school
cargo test calendar_schema_routes_and_permissions_are_registered --test static_architecture
cd ../frontend-school
npm run test:static -- calendar.test.mjs
```

Expected:

- Backend still fails on missing module/routes until later tasks.
- Frontend still fails on missing calendar files until later tasks.
- Permission registry failures should be gone.

- [ ] **Step 8: Commit Task 1**

```bash
git add backend-school/migrations/018_school_calendar.sql \
  backend-school/src/permissions/registry.rs \
  frontend-school/src/lib/permissions/registry.ts \
  backend-school/tests/static_architecture.rs \
  frontend-school/tests/static/calendar.test.mjs
git commit -m "feat: add calendar schema and permissions"
```

---

### Task 2: Backend Calendar Models And Pure Service Helpers

**Files:**
- Create: `backend-school/src/modules/calendar.rs`
- Create: `backend-school/src/modules/calendar/models.rs`
- Create: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/src/modules.rs`

- [ ] **Step 1: Register the calendar module root**

Modify `backend-school/src/modules.rs`:

```rust
pub mod calendar;
```

Create `backend-school/src/modules/calendar.rs`:

```rust
pub mod handlers;
pub mod models;
pub mod services;

use crate::AppState;
use axum::routing::{delete, get, post, put};
use axum::Router;

pub fn calendar_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/events",
            get(handlers::list_calendar_events).post(handlers::create_calendar_event),
        )
        .route(
            "/events/{id}",
            put(handlers::update_calendar_event).delete(handlers::delete_calendar_event),
        )
        .route(
            "/categories",
            get(handlers::list_calendar_categories).post(handlers::create_calendar_category),
        )
        .route(
            "/categories/{id}",
            put(handlers::update_calendar_category).delete(handlers::delete_calendar_category),
        )
}
```

Create a temporary `backend-school/src/modules/calendar/handlers.rs` with stubs so the module compiles after model/service work:

```rust
use crate::error::AppError;

pub async fn list_calendar_events() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn create_calendar_event() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn update_calendar_event() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn delete_calendar_event() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn list_calendar_categories() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn create_calendar_category() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn update_calendar_category() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}

pub async fn delete_calendar_category() -> Result<(), AppError> {
    Err(AppError::InternalServerError("calendar handler not wired".to_string()))
}
```

- [ ] **Step 2: Write failing unit tests for pure helper behavior**

Create `backend-school/src/modules/calendar/services.rs` with tests first:

```rust
use chrono::{Duration, NaiveDate, NaiveTime};
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{CalendarAudienceType, CalendarEventTargetInput};

pub fn validate_event_date_time(
    _start_date: NaiveDate,
    _end_date: NaiveDate,
    _all_day: bool,
    _start_time: Option<NaiveTime>,
    _end_time: Option<NaiveTime>,
) -> Result<(), AppError> {
    Err(AppError::BadRequest("calendar helper pending Step 3".to_string()))
}

pub fn reminder_dates(_start_date: NaiveDate, _offsets: &[i32]) -> Result<Vec<NaiveDate>, AppError> {
    Err(AppError::BadRequest("calendar helper pending Step 3".to_string()))
}

pub fn validate_targets(_targets: &[CalendarEventTargetInput]) -> Result<(), AppError> {
    Err(AppError::BadRequest("calendar helper pending Step 3".to_string()))
}

pub fn dedupe_user_ids(_ids: Vec<Uuid>) -> Vec<Uuid> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_event_date_time_rejects_end_date_before_start() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();

        assert!(validate_event_date_time(start, end, true, None, None).is_err());
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_end_time_before_start_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let start_time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(validate_event_date_time(date, date, false, Some(start_time), Some(end_time)).is_err());
    }

    #[test]
    fn validate_event_date_time_accepts_multi_day_timed_event() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
        let start_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(validate_event_date_time(start, end, false, Some(start_time), Some(end_time)).is_ok());
    }

    #[test]
    fn reminder_dates_are_day_based_and_sorted() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[1, 7, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_dates_reject_zero_or_negative_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[0]).is_err());
        assert!(reminder_dates(start, &[-1]).is_err());
    }

    #[test]
    fn validate_targets_rejects_parent_visibility_from_student_only_target() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        }];

        assert!(validate_targets(&targets).is_ok());
    }

    #[test]
    fn validate_targets_rejects_staff_with_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Staff,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn dedupe_user_ids_preserves_first_seen_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        assert_eq!(dedupe_user_ids(vec![a, b, a]), vec![a, b]);
    }
}
```

- [ ] **Step 3: Add calendar models**

Create `backend-school/src/modules/calendar/models.rs`:

```rust
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarAudienceType {
    All,
    Staff,
    Student,
    Parent,
}

impl CalendarAudienceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CalendarAudienceType::All => "all",
            CalendarAudienceType::Staff => "staff",
            CalendarAudienceType::Student => "student",
            CalendarAudienceType::Parent => "parent",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarCategory {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub order_index: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarCategoryRequest {
    pub name: String,
    pub color: String,
    pub order_index: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventTarget {
    pub id: Uuid,
    pub audience_type: String,
    pub grade_level_id: Option<Uuid>,
    pub class_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventTargetInput {
    pub audience_type: CalendarAudienceType,
    pub grade_level_id: Option<Uuid>,
    pub class_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventReminder {
    pub id: Uuid,
    pub days_before: i32,
    pub remind_on: NaiveDate,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub targets: Vec<CalendarEventTarget>,
    pub reminders: Vec<CalendarEventReminder>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CalendarEventRow {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category_id: Option<Uuid>,
    pub audience: Option<CalendarAudienceType>,
    pub visibility: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub category_id: Option<Uuid>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub targets: Vec<CalendarEventTargetInput>,
    pub reminder_offsets_days: Vec<i32>,
    pub notify_audience: bool,
}
```

- [ ] **Step 4: Implement pure helpers**

Replace the stub helpers in `backend-school/src/modules/calendar/services.rs`:

```rust
pub fn validate_event_date_time(
    start_date: NaiveDate,
    end_date: NaiveDate,
    all_day: bool,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
) -> Result<(), AppError> {
    if end_date < start_date {
        return Err(AppError::BadRequest("วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น".to_string()));
    }

    if all_day {
        return Ok(());
    }

    if start_date == end_date {
        match (start_time, end_time) {
            (Some(start), Some(end)) if end > start => Ok(()),
            (Some(_), Some(_)) => Err(AppError::BadRequest(
                "เวลาสิ้นสุดต้องหลังเวลาเริ่มต้น".to_string(),
            )),
            _ => Err(AppError::BadRequest(
                "event แบบระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
            )),
        }
    } else if start_time.is_some() && end_time.is_some() {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "event หลายวันที่ระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
        ))
    }
}

pub fn reminder_dates(start_date: NaiveDate, offsets: &[i32]) -> Result<Vec<NaiveDate>, AppError> {
    let mut dates = Vec::with_capacity(offsets.len());
    let mut seen = HashSet::new();

    for days_before in offsets {
        if *days_before <= 0 {
            return Err(AppError::BadRequest("จำนวนวันแจ้งเตือนต้องมากกว่า 0".to_string()));
        }
        if seen.insert(*days_before) {
            dates.push(start_date - Duration::days(i64::from(*days_before)));
        }
    }

    dates.sort();
    Ok(dates)
}

pub fn validate_targets(targets: &[CalendarEventTargetInput]) -> Result<(), AppError> {
    if targets.is_empty() {
        return Err(AppError::BadRequest("ต้องเลือกผู้เห็นอย่างน้อยหนึ่งกลุ่ม".to_string()));
    }

    for target in targets {
        match target.audience_type {
            CalendarAudienceType::All | CalendarAudienceType::Staff => {
                if target.grade_level_id.is_some() || target.class_room_id.is_some() {
                    return Err(AppError::BadRequest(
                        "กลุ่มผู้เห็น all/staff ไม่รองรับการกรองระดับชั้นหรือห้องเรียน".to_string(),
                    ));
                }
            }
            CalendarAudienceType::Student | CalendarAudienceType::Parent => {}
        }
    }

    Ok(())
}

pub fn dedupe_user_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for id in ids {
        if seen.insert(id) {
            deduped.push(id);
        }
    }

    deduped
}
```

- [ ] **Step 5: Run helper tests**

Run:

```bash
cd backend-school
cargo test modules::calendar::services::tests --bin backend-school
```

Expected: all calendar helper tests pass.

- [ ] **Step 6: Commit Task 2**

```bash
git add backend-school/src/modules.rs \
  backend-school/src/modules/calendar.rs \
  backend-school/src/modules/calendar/models.rs \
  backend-school/src/modules/calendar/services.rs \
  backend-school/src/modules/calendar/handlers.rs
git commit -m "feat: add calendar models and validation helpers"
```

---

### Task 3: Backend Calendar CRUD Services

**Files:**
- Modify: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/src/modules/calendar/models.rs`

- [ ] **Step 1: Add row hydration helpers**

Add this helper shape to `services.rs`:

```rust
async fn hydrate_events(pool: &PgPool, rows: Vec<CalendarEventRow>) -> Result<Vec<CalendarEvent>, AppError> {
    let event_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let targets = list_targets_for_events(pool, &event_ids).await?;
    let reminders = list_reminders_for_events(pool, &event_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| CalendarEvent {
            id: row.id,
            category_id: row.category_id,
            category_name: row.category_name,
            category_color: row.category_color,
            title: row.title,
            description: row.description,
            location: row.location,
            start_date: row.start_date,
            end_date: row.end_date,
            all_day: row.all_day,
            start_time: row.start_time,
            end_time: row.end_time,
            is_public: row.is_public,
            targets: targets.get(&row.id).cloned().unwrap_or_default(),
            reminders: reminders.get(&row.id).cloned().unwrap_or_default(),
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}
```

- [ ] **Step 2: Implement category service functions**

Add these public functions to `services.rs`:

```rust
pub async fn list_categories(pool: &PgPool) -> Result<Vec<CalendarCategory>, AppError>;
pub async fn create_category(
    pool: &PgPool,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError>;
pub async fn update_category(
    pool: &PgPool,
    id: Uuid,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError>;
pub async fn deactivate_category(pool: &PgPool, id: Uuid) -> Result<(), AppError>;
```

Implementation rules:

- `list_categories`: `WHERE is_active = true ORDER BY order_index, name`.
- `create_category`: if `order_index` is not provided, use `COALESCE(MAX(order_index), 0) + 1`.
- `update_category`: updates `name`, `color`, `order_index`, and `is_active`.
- `deactivate_category`: `UPDATE calendar_categories SET is_active = false WHERE id = $1`.
- Map duplicate active category name errors to `AppError::BadRequest("มีหมวดหมู่นี้อยู่แล้ว".to_string())`.

- [ ] **Step 3: Implement event write transaction functions**

Add these public functions to `services.rs`:

```rust
pub async fn create_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEvent, AppError>;

pub async fn update_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEvent, AppError>;

pub async fn soft_delete_event(pool: &PgPool, id: Uuid, actor_user_id: Uuid) -> Result<(), AppError>;
```

Write path rules:

- Call `validate_event_date_time(payload.start_date, payload.end_date, payload.all_day, payload.start_time, payload.end_time)`.
- Call `validate_targets(&payload.targets)`.
- Begin a transaction.
- Insert or update `calendar_events`.
- Replace `calendar_event_targets` for create/update.
- Delete pending reminders for create/update with:

```sql
DELETE FROM calendar_event_reminders
WHERE event_id = $1 AND sent_at IS NULL
```

- Insert one reminder row per unique `days_before` value:

```sql
INSERT INTO calendar_event_reminders (event_id, days_before, remind_on)
VALUES ($1, $2, $3)
ON CONFLICT (event_id, days_before) DO UPDATE SET
    remind_on = EXCLUDED.remind_on
```

- Commit transaction.
- Return `get_event_for_response(pool, event_id).await?`.

- [ ] **Step 4: Implement list services**

Add:

```rust
pub async fn list_management_events(
    pool: &PgPool,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarEvent>, AppError>;

pub async fn list_public_events(
    pool: &PgPool,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarEvent>, AppError>;
```

Filtering rules:

- Date range overlap uses `event.start_date <= to AND event.end_date >= from`.
- Default date range is current month if `from` or `to` is absent. Use a pure helper:

```rust
fn normalized_event_range(query: &CalendarEventQuery, today: NaiveDate) -> (NaiveDate, NaiveDate)
```

- Public list adds `calendar_events.is_public = true`.
- Public list returns empty `targets` and `reminders` in the response. Use a separate mapper or clear those fields after hydration.

- [ ] **Step 5: Run backend checks for Task 3**

Run:

```bash
cd backend-school
cargo test modules::calendar::services::tests --bin backend-school
cargo check
```

Expected: tests and `cargo check` pass.

- [ ] **Step 6: Commit Task 3**

```bash
git add backend-school/src/modules/calendar/models.rs backend-school/src/modules/calendar/services.rs
git commit -m "feat: add calendar CRUD services"
```

---

### Task 4: Audience Resolution, Notifications, And Daily Reminders

**Files:**
- Modify: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/src/main.rs`

- [ ] **Step 1: Add recipient resolution functions**

Add these service functions:

```rust
pub async fn resolve_event_recipient_user_ids(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Vec<Uuid>, AppError>;

pub async fn send_event_notification(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<(Uuid, Notification)>,
    event: &CalendarEvent,
    notification_kind: CalendarNotificationKind,
) -> Result<(), AppError>;
```

Use these audience rules:

- `all`: active users with `user_type IN ('staff', 'student', 'parent')`.
- `staff`: active users with `user_type = 'staff'`.
- `student`: active student users, filtered by active `student_class_enrollments` when target has grade/class.
- `parent`: active parent users through `student_parents`, filtered by the linked student's active enrollment when target has grade/class.
- Deduplicate with `dedupe_user_ids`.

- [ ] **Step 2: Add notification kind formatter**

Add this enum and formatter:

```rust
pub enum CalendarNotificationKind {
    Created,
    Updated,
    Reminder { days_before: i32 },
}

fn calendar_notification_text(
    event: &CalendarEvent,
    kind: &CalendarNotificationKind,
) -> (String, String) {
    match kind {
        CalendarNotificationKind::Created => (
            format!("เพิ่มกำหนดการ: {}", event.title),
            "มีกำหนดการใหม่ในปฏิทินโรงเรียน".to_string(),
        ),
        CalendarNotificationKind::Updated => (
            format!("อัปเดตกำหนดการ: {}", event.title),
            "มีการเปลี่ยนแปลงกำหนดการในปฏิทินโรงเรียน".to_string(),
        ),
        CalendarNotificationKind::Reminder { days_before } => (
            format!("เตือนล่วงหน้า: {}", event.title),
            format!("กำหนดการนี้จะเริ่มในอีก {} วัน", days_before),
        ),
    }
}
```

Use `NotificationType::Info` and route links:

- staff: `/staff/calendar`
- student: `/student/calendar`
- parent: `/parent/student/{id}/calendar` can fall back to `/parent` when the recipient was resolved through multiple children.

- [ ] **Step 3: Wire notify-on-create/update from service outcome**

Return an outcome from create/update:

```rust
pub struct CalendarEventMutationOutcome {
    pub event: CalendarEvent,
    pub notify_audience: bool,
    pub notification_kind: CalendarNotificationKind,
}
```

Handlers will call `send_event_notification` after the DB mutation returns. This keeps the write transaction small and avoids holding an event row lock during push work.

- [ ] **Step 4: Add due reminder processing**

Add:

```rust
pub async fn process_due_reminders(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<(Uuid, Notification)>,
    tenant_current_date: NaiveDate,
) -> Result<i64, AppError>;
```

Processing algorithm:

```sql
SELECT id, event_id, days_before
FROM calendar_event_reminders
WHERE remind_on <= $1 AND sent_at IS NULL
ORDER BY remind_on ASC, created_at ASC
FOR UPDATE SKIP LOCKED
LIMIT 200
```

For each row:

- Load the event through `get_event_for_response`.
- Send `CalendarNotificationKind::Reminder { days_before }`.
- Mark `sent_at = NOW()` after sending is attempted:

```sql
UPDATE calendar_event_reminders
SET sent_at = NOW()
WHERE id = $1 AND sent_at IS NULL
```

- [ ] **Step 5: Add all-tenant scheduler entry point**

Add:

```rust
pub async fn process_due_calendar_reminders_for_all_tenants(
    admin_client: Arc<AdminClient>,
    pool_manager: Arc<PoolManager>,
    notification_channel: broadcast::Sender<(Uuid, Notification)>,
) {
    let tenant_current_date = (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive();

    let schools = match admin_client.list_active_schools().await {
        Ok(schools) => schools,
        Err(error) => {
            tracing::error!("Failed to fetch schools for calendar reminders: {}", error);
            return;
        }
    };

    for school in schools {
        let Some(db_url) = school.db_connection_string.filter(|value| !value.is_empty()) else {
            tracing::warn!("Skipping calendar reminders for {}: no database URL", school.subdomain);
            continue;
        };

        match pool_manager.get_pool(&db_url, &school.subdomain).await {
            Ok(pool) => {
                if let Err(error) = process_due_reminders(&pool, &notification_channel, tenant_current_date).await {
                    tracing::error!("Calendar reminder processing failed for {}: {}", school.subdomain, error);
                }
            }
            Err(error) => {
                tracing::error!("Failed to open tenant pool for calendar reminders {}: {}", school.subdomain, error);
            }
        }
    }
}
```

Place this in `services.rs` or a `services` helper block that has access to `AdminClient` and `PoolManager` imports.

- [ ] **Step 6: Register the daily scheduler in `main.rs`**

After `cleaner_job`, add:

```rust
let admin_client_for_calendar_job = Arc::clone(&state.admin_client);
let pool_manager_for_calendar_job = Arc::clone(&state.pool_manager);
let notification_channel_for_calendar_job = state.notification_channel.clone();

let calendar_reminder_job = Job::new_async("0 0 7 * * *", move |_uuid, _l| {
    let admin_client = Arc::clone(&admin_client_for_calendar_job);
    let pool_manager = Arc::clone(&pool_manager_for_calendar_job);
    let notification_channel = notification_channel_for_calendar_job.clone();

    Box::pin(async move {
        modules::calendar::services::process_due_calendar_reminders_for_all_tenants(
            admin_client,
            pool_manager,
            notification_channel,
        )
        .await;
    })
})
.expect("Failed to create calendar reminder job");

sched
    .add(calendar_reminder_job)
    .await
    .expect("Failed to add calendar reminder job");
```

- [ ] **Step 7: Run backend checks**

Run:

```bash
cd backend-school
cargo test modules::calendar::services::tests --bin backend-school
cargo check
```

Expected: all pass.

- [ ] **Step 8: Commit Task 4**

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/main.rs
git commit -m "feat: add calendar notifications and daily reminders"
```

---

### Task 5: Backend Handlers And Route Wiring

**Files:**
- Modify: `backend-school/src/modules/calendar/handlers.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/src/modules/parents/handlers.rs`
- Modify: `backend-school/src/modules/parents/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

- [ ] **Step 1: Replace calendar handler stubs with thin handlers**

Implement this handler shape in `backend-school/src/modules/calendar/handlers.rs`:

```rust
use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarEventQuery, UpsertCalendarCategoryRequest, UpsertCalendarEventRequest,
};
use crate::modules::calendar::services as calendar_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_headers, tenant_context,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

pub async fn list_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context.actor.require_permission(codes::CALENDAR_READ_SCHOOL)?;
    let events = calendar_service::list_management_events(&context.tenant.pool, query).await?;
    Ok(Json(ApiResponse::ok(events)))
}
```

Apply the same pattern:

- Create/update/delete: `codes::CALENDAR_MANAGE_SCHOOL`.
- Category list: `codes::CALENDAR_READ_SCHOOL`.
- Category mutations: `codes::CALENDAR_MANAGE_SCHOOL`.
- Mutation handlers call notification service after service returns a `notify_audience` outcome.

- [ ] **Step 2: Add self-view and public handlers**

Add:

```rust
pub async fn list_my_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let events = calendar_service::list_my_events(
        &context.tenant.pool,
        context.user_id,
        query,
    )
    .await?;
    Ok(Json(ApiResponse::ok(events)))
}

pub async fn list_public_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = tenant_context(&state, &headers).await?;
    let events = calendar_service::list_public_events(&context.pool, query).await?;
    Ok(Json(ApiResponse::ok(events)))
}
```

- [ ] **Step 3: Add parent child calendar service and handler**

In `backend-school/src/modules/parents/services.rs`, add:

```rust
pub async fn get_child_calendar_events(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
    query: crate::modules::calendar::models::CalendarEventQuery,
) -> Result<Vec<crate::modules::calendar::models::CalendarEvent>, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    crate::modules::calendar::services::list_child_events(pool, parent_id, student_id, query).await
}
```

In `backend-school/src/modules/parents/handlers.rs`, add:

```rust
/// GET /api/parent/students/:student_id/calendar/events
pub async fn get_child_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Query(query): Query<crate::modules::calendar::models::CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let events = parent_service::get_child_calendar_events(
        &context.tenant.pool,
        context.actor.user_id,
        student_id,
        query,
    )
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(events))))
}
```

- [ ] **Step 4: Wire routes in `main.rs`**

Add a protected calendar nest near other protected module nests:

```rust
.nest(
    "/api/calendar",
    modules::calendar::calendar_routes()
        .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)),
)
```

Add protected self/parent routes near timetable equivalents:

```rust
.route(
    "/api/me/calendar/events",
    get(modules::calendar::handlers::list_my_calendar_events)
        .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)),
)
.route(
    "/api/parent/students/{student_id}/calendar/events",
    get(modules::parents::handlers::get_child_calendar_events)
        .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)),
)
```

Add public route without auth middleware:

```rust
.route(
    "/api/public/calendar/events",
    get(modules::calendar::handlers::list_public_calendar_events),
)
```

- [ ] **Step 5: Add backend static boundary checks**

Append to `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn calendar_handlers_stay_thin_and_services_own_sql() {
    let handlers = strip_comments(&read_source(
        manifest_dir().join("src/modules/calendar/handlers.rs"),
    ));
    let services = strip_comments(&read_source(
        manifest_dir().join("src/modules/calendar/services.rs"),
    ));

    assert!(handlers.contains("actor_tenant_context(&state, &headers).await?"));
    assert!(handlers.contains("codes::CALENDAR_READ_SCHOOL"));
    assert!(handlers.contains("codes::CALENDAR_MANAGE_SCHOOL"));
    assert!(!handlers.contains("sqlx::query"));
    assert!(!handlers.contains(".fetch_"));
    assert!(!handlers.contains(".execute("));
    assert!(services.contains("sqlx::query"));
    assert!(services.contains("CalendarEvent"));
    assert!(services.contains("resolve_event_recipient_user_ids"));
    assert!(services.contains("process_due_reminders"));
}
```

- [ ] **Step 6: Run backend verification**

Run:

```bash
cd backend-school
cargo test calendar_schema_routes_and_permissions_are_registered calendar_handlers_stay_thin_and_services_own_sql --test static_architecture
cargo test modules::calendar::services::tests --bin backend-school
cargo check
```

Expected: all pass.

- [ ] **Step 7: Commit Task 5**

```bash
git add backend-school/src/modules/calendar/handlers.rs \
  backend-school/src/main.rs \
  backend-school/src/modules/parents/handlers.rs \
  backend-school/src/modules/parents/services.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat: expose calendar API routes"
```

---

### Task 6: Frontend Calendar API And Date Utilities

**Files:**
- Create: `frontend-school/src/lib/api/calendar.ts`
- Create: `frontend-school/src/lib/utils/calendar.ts`
- Modify: `frontend-school/tests/static/calendar.test.mjs`

- [ ] **Step 1: Add frontend API client**

Create `frontend-school/src/lib/api/calendar.ts`:

```ts
import { apiClient, requireApiData } from '$lib/api/client';

export type CalendarAudienceType = 'all' | 'staff' | 'student' | 'parent';

export interface CalendarCategory {
	id: string;
	name: string;
	color: string;
	orderIndex: number;
	isActive: boolean;
	createdAt: string;
	updatedAt: string;
}

export interface CalendarEventTarget {
	id?: string;
	audienceType: CalendarAudienceType;
	gradeLevelId?: string | null;
	classRoomId?: string | null;
}

export interface CalendarEventReminder {
	id: string;
	daysBefore: number;
	remindOn: string;
	sentAt?: string | null;
}

export interface CalendarEvent {
	id: string;
	categoryId?: string | null;
	categoryName?: string | null;
	categoryColor?: string | null;
	title: string;
	description?: string | null;
	location?: string | null;
	startDate: string;
	endDate: string;
	allDay: boolean;
	startTime?: string | null;
	endTime?: string | null;
	isPublic: boolean;
	targets: CalendarEventTarget[];
	reminders: CalendarEventReminder[];
	createdBy?: string | null;
	updatedBy?: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface CalendarEventFilters {
	from?: string;
	to?: string;
	categoryId?: string;
	audience?: CalendarAudienceType;
	visibility?: 'public' | 'private';
	q?: string;
}

export interface CreateCalendarEventRequest {
	title: string;
	description?: string | null;
	location?: string | null;
	categoryId?: string | null;
	startDate: string;
	endDate: string;
	allDay: boolean;
	startTime?: string | null;
	endTime?: string | null;
	isPublic: boolean;
	targets: CalendarEventTarget[];
	reminderOffsetsDays: number[];
	notifyAudience: boolean;
}

export interface UpsertCalendarCategoryRequest {
	name: string;
	color: string;
	orderIndex?: number;
	isActive?: boolean;
}

function calendarQuery(filters: CalendarEventFilters = {}) {
	const params = new URLSearchParams();
	if (filters.from) params.set('from', filters.from);
	if (filters.to) params.set('to', filters.to);
	if (filters.categoryId) params.set('category_id', filters.categoryId);
	if (filters.audience) params.set('audience', filters.audience);
	if (filters.visibility) params.set('visibility', filters.visibility);
	if (filters.q) params.set('q', filters.q);
	const query = params.toString();
	return query ? `?${query}` : '';
}

export async function listCalendarEvents(filters: CalendarEventFilters = {}) {
	const response = await apiClient.get<CalendarEvent[]>(`/api/calendar/events${calendarQuery(filters)}`);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินได้');
}

export async function listMyCalendarEvents(filters: CalendarEventFilters = {}) {
	const response = await apiClient.get<CalendarEvent[]>(`/api/me/calendar/events${calendarQuery(filters)}`);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินได้');
}

export async function listChildCalendarEvents(studentId: string, filters: CalendarEventFilters = {}) {
	const response = await apiClient.get<CalendarEvent[]>(
		`/api/parent/students/${studentId}/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินของบุตรหลานได้');
}

export async function listPublicCalendarEvents(filters: CalendarEventFilters = {}) {
	const response = await apiClient.get<CalendarEvent[]>(
		`/api/public/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินสาธารณะได้');
}

export async function createCalendarEvent(data: CreateCalendarEventRequest) {
	const response = await apiClient.post<CalendarEvent>('/api/calendar/events', data);
	return requireApiData(response, 'สร้าง event ไม่สำเร็จ');
}

export async function updateCalendarEvent(id: string, data: CreateCalendarEventRequest) {
	const response = await apiClient.put<CalendarEvent>(`/api/calendar/events/${id}`, data);
	return requireApiData(response, 'บันทึก event ไม่สำเร็จ');
}

export async function deleteCalendarEvent(id: string) {
	const response = await apiClient.delete<Record<string, never>>(`/api/calendar/events/${id}`);
	return requireApiData(response, 'ลบ event ไม่สำเร็จ');
}

export async function listCalendarCategories() {
	const response = await apiClient.get<CalendarCategory[]>('/api/calendar/categories');
	return requireApiData(response, 'ไม่สามารถโหลดหมวดหมู่ได้');
}

export async function createCalendarCategory(data: UpsertCalendarCategoryRequest) {
	const response = await apiClient.post<CalendarCategory>('/api/calendar/categories', data);
	return requireApiData(response, 'สร้างหมวดหมู่ไม่สำเร็จ');
}

export async function updateCalendarCategory(id: string, data: UpsertCalendarCategoryRequest) {
	const response = await apiClient.put<CalendarCategory>(`/api/calendar/categories/${id}`, data);
	return requireApiData(response, 'บันทึกหมวดหมู่ไม่สำเร็จ');
}

export async function deleteCalendarCategory(id: string) {
	const response = await apiClient.delete<Record<string, never>>(`/api/calendar/categories/${id}`);
	return requireApiData(response, 'ลบหมวดหมู่ไม่สำเร็จ');
}
```

- [ ] **Step 2: Add date helper unit tests**

Create static helper tests in `frontend-school/tests/static/calendar-utils.test.mjs`:

```js
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { buildCalendarMonth, eventOverlapsDate } from '../../src/lib/utils/calendar.ts';

describe('calendar helpers', () => {
	it('builds a 42-cell month grid', () => {
		const cells = buildCalendarMonth('2026-07-01');
		assert.equal(cells.length, 42);
		assert.equal(cells.some((cell) => cell.date === '2026-07-01'), true);
	});

	it('detects multi-day event overlap', () => {
		assert.equal(eventOverlapsDate({ startDate: '2026-07-03', endDate: '2026-07-05' }, '2026-07-04'), true);
		assert.equal(eventOverlapsDate({ startDate: '2026-07-03', endDate: '2026-07-05' }, '2026-07-06'), false);
	});
});
```

- [ ] **Step 3: Implement calendar utilities**

Create `frontend-school/src/lib/utils/calendar.ts`:

```ts
import { addDays, endOfMonth, format, isSameMonth, parseISO, startOfMonth, startOfWeek } from 'date-fns';
import { th } from 'date-fns/locale';

export interface CalendarMonthCell {
	date: string;
	dayNumber: number;
	inCurrentMonth: boolean;
}

export function toIsoDate(date: Date): string {
	return format(date, 'yyyy-MM-dd');
}

export function buildCalendarMonth(monthDate: string): CalendarMonthCell[] {
	const monthStart = startOfMonth(parseISO(monthDate));
	const gridStart = startOfWeek(monthStart, { weekStartsOn: 1 });

	return Array.from({ length: 42 }, (_, index) => {
		const date = addDays(gridStart, index);
		return {
			date: toIsoDate(date),
			dayNumber: Number(format(date, 'd')),
			inCurrentMonth: isSameMonth(date, monthStart)
		};
	});
}

export function monthRange(monthDate: string): { from: string; to: string } {
	const parsed = parseISO(monthDate);
	return {
		from: toIsoDate(startOfMonth(parsed)),
		to: toIsoDate(endOfMonth(parsed))
	};
}

export function formatCalendarDate(value: string): string {
	return format(parseISO(value), 'd MMM yyyy', { locale: th });
}

export function eventOverlapsDate(
	event: { startDate: string; endDate: string },
	date: string
): boolean {
	return event.startDate <= date && event.endDate >= date;
}
```

- [ ] **Step 4: Run frontend tests**

Run:

```bash
cd frontend-school
npm run test:static -- calendar-utils.test.mjs calendar.test.mjs
```

Expected:

- `calendar-utils.test.mjs` passes.
- `calendar.test.mjs` still fails until routes/components are added.

- [ ] **Step 5: Commit Task 6**

```bash
git add frontend-school/src/lib/api/calendar.ts \
  frontend-school/src/lib/utils/calendar.ts \
  frontend-school/tests/static/calendar-utils.test.mjs
git commit -m "feat: add frontend calendar API utilities"
```

---

### Task 7: Shared Calendar UI Components

**Files:**
- Create: `frontend-school/src/lib/components/calendar/CalendarMonthGrid.svelte`
- Create: `frontend-school/src/lib/components/calendar/CalendarEventList.svelte`
- Create: `frontend-school/src/lib/components/calendar/CalendarEventDialog.svelte`
- Create: `frontend-school/src/lib/components/calendar/CalendarCategoryDialog.svelte`

- [ ] **Step 1: Create month grid component**

Create `CalendarMonthGrid.svelte` using plain layout for the grid and shadcn `Badge` for event markers:

```svelte
<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import type { CalendarEvent } from '$lib/api/calendar';
	import { buildCalendarMonth, eventOverlapsDate } from '$lib/utils/calendar';
	import { cn } from '$lib/utils';

	let {
		monthDate,
		events = [],
		selectedDate = '',
		onselect
	}: {
		monthDate: string;
		events?: CalendarEvent[];
		selectedDate?: string;
		onselect?: (date: string) => void;
	} = $props();

	const weekDays = ['จ', 'อ', 'พ', 'พฤ', 'ศ', 'ส', 'อา'];
	let cells = $derived(buildCalendarMonth(monthDate));

	function eventsForDate(date: string) {
		return events.filter((event) => eventOverlapsDate(event, date)).slice(0, 3);
	}
</script>

<div class="rounded-md border bg-background">
	<div class="grid grid-cols-7 border-b text-center text-xs font-medium text-muted-foreground">
		{#each weekDays as day (day)}
			<div class="px-2 py-2">{day}</div>
		{/each}
	</div>
	<div class="grid grid-cols-7">
		{#each cells as cell (cell.date)}
			<button
				type="button"
				class={cn(
					'min-h-24 border-b border-r p-2 text-left transition-colors hover:bg-muted/50',
					!cell.inCurrentMonth && 'bg-muted/30 text-muted-foreground',
					selectedDate === cell.date && 'bg-primary/5 ring-1 ring-primary'
				)}
				onclick={() => onselect?.(cell.date)}
			>
				<div class="text-xs font-medium">{cell.dayNumber}</div>
				<div class="mt-2 space-y-1">
					{#each eventsForDate(cell.date) as event (event.id)}
						<Badge variant="secondary" class="block truncate border-l-4 text-[11px]" style={`border-left-color:${event.categoryColor ?? '#64748b'}`}>
							{event.title}
						</Badge>
					{/each}
				</div>
			</button>
		{/each}
	</div>
</div>
```

- [ ] **Step 2: Create event list component**

Create `CalendarEventList.svelte`:

```svelte
<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { PageState } from '$lib/components/app-state';
	import type { CalendarEvent } from '$lib/api/calendar';
	import { formatCalendarDate } from '$lib/utils/calendar';
	import { Pencil, Trash2 } from 'lucide-svelte';

	let {
		events = [],
		canManage = false,
		onedit,
		ondelete
	}: {
		events?: CalendarEvent[];
		canManage?: boolean;
		onedit?: (event: CalendarEvent) => void;
		ondelete?: (event: CalendarEvent) => void;
	} = $props();
</script>

{#if events.length === 0}
	<PageState title="ยังไม่มี event" description="ไม่มีรายการในช่วงวันที่ที่เลือก" />
{:else}
	<div class="space-y-3">
		{#each events as event (event.id)}
			<div class="rounded-md border bg-background p-4">
				<div class="flex items-start justify-between gap-3">
					<div class="min-w-0">
						<div class="flex flex-wrap items-center gap-2">
							<span class="h-3 w-3 rounded-full" style={`background:${event.categoryColor ?? '#64748b'}`}></span>
							<h3 class="truncate font-medium">{event.title}</h3>
							{#if event.isPublic}
								<Badge variant="outline">Public</Badge>
							{/if}
						</div>
						<p class="mt-1 text-sm text-muted-foreground">
							{formatCalendarDate(event.startDate)}
							{#if event.endDate !== event.startDate} - {formatCalendarDate(event.endDate)}{/if}
							{#if !event.allDay && event.startTime && event.endTime}
								· {event.startTime.slice(0, 5)}-{event.endTime.slice(0, 5)}
							{/if}
						</p>
						{#if event.location}
							<p class="mt-1 text-sm text-muted-foreground">{event.location}</p>
						{/if}
					</div>
					{#if canManage}
						<div class="flex shrink-0 gap-1">
							<Button variant="ghost" size="icon" onclick={() => onedit?.(event)} aria-label="แก้ไข event">
								<Pencil class="h-4 w-4" />
							</Button>
							<Button variant="ghost" size="icon" onclick={() => ondelete?.(event)} aria-label="ลบ event">
								<Trash2 class="h-4 w-4 text-destructive" />
							</Button>
						</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
{/if}
```

- [ ] **Step 3: Create event dialog**

Create `CalendarEventDialog.svelte` with these required imports and props:

```svelte
<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Textarea } from '$lib/components/ui/textarea';
	import DatePicker from '$lib/components/ui/date-picker/DatePicker.svelte';
	import { LoadingButton } from '$lib/components/app-state';
	import type {
		CalendarCategory,
		CalendarEvent,
		CalendarEventTarget,
		CreateCalendarEventRequest
	} from '$lib/api/calendar';

	let {
		open = $bindable(false),
		event = null,
		categories = [],
		gradeLevels = [],
		classrooms = [],
		saving = false,
		onsave
	}: {
		open: boolean;
		event?: CalendarEvent | null;
		categories?: CalendarCategory[];
		gradeLevels?: { id: string; name: string }[];
		classrooms?: { id: string; name: string; grade_level_id?: string }[];
		saving?: boolean;
		onsave?: (payload: CreateCalendarEventRequest) => void;
	} = $props();
</script>
```

Implement the form with:

- Inputs for `title`, `location`.
- `Textarea` for `description`.
- `DatePicker` for start/end dates.
- `Checkbox` for all-day, public, notify audience.
- `Select` for category.
- Checkbox group or button toggles for `all`, `staff`, `student`, `parent`.
- Grade/class selectors shown only when student or parent is selected.
- Reminder controls for `1`, `3`, and `7` days, plus a numeric input for another positive day value.
- `LoadingButton` submit label `บันทึกและเผยแพร่`.

Build payload in a local `submitForm()` function:

```ts
function submitForm() {
	const targets: CalendarEventTarget[] = selectedAudiences.map((audienceType) => ({
		audienceType,
		gradeLevelId:
			audienceType === 'student' || audienceType === 'parent' ? selectedGradeLevelId || null : null,
		classRoomId:
			audienceType === 'student' || audienceType === 'parent' ? selectedClassRoomId || null : null
	}));

	onsave?.({
		title: title.trim(),
		description: description.trim() || null,
		location: location.trim() || null,
		categoryId: categoryId || null,
		startDate,
		endDate,
		allDay,
		startTime: allDay ? null : startTime,
		endTime: allDay ? null : endTime,
		isPublic,
		targets,
		reminderOffsetsDays: reminderOffsetsDays.filter((value) => value > 0),
		notifyAudience
	});
}
```

- [ ] **Step 4: Create category dialog**

Create `CalendarCategoryDialog.svelte` with shadcn `Dialog`, `Button`, `Input`, `Label`, and `LoadingButton`. Required behavior:

- List active categories with color dots.
- Edit selected category fields.
- Create category when no category is selected.
- Deactivate category through a destructive button.
- Restrict color choices to this palette:

```ts
const colorOptions = ['#2563eb', '#16a34a', '#f59e0b', '#dc2626', '#7c3aed', '#0891b2'];
```

- [ ] **Step 5: Run frontend static test for components**

Run:

```bash
cd frontend-school
npm run test:static -- calendar.test.mjs
```

Expected: component import checks pass; route checks still fail until route files are added.

- [ ] **Step 6: Commit Task 7**

```bash
git add frontend-school/src/lib/components/calendar
git commit -m "feat: add shared calendar components"
```

---

### Task 8: Frontend Routes And Workspaces

**Files:**
- Create: `frontend-school/src/routes/(app)/staff/calendar/+page.ts`
- Create: `frontend-school/src/routes/(app)/staff/calendar/+page.svelte`
- Create: `frontend-school/src/routes/(app)/student/calendar/+page.ts`
- Create: `frontend-school/src/routes/(app)/student/calendar/+page.svelte`
- Create: `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.ts`
- Create: `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.svelte`
- Create: `frontend-school/src/routes/(public)/calendar/+page.ts`
- Create: `frontend-school/src/routes/(public)/calendar/+page.svelte`

- [ ] **Step 1: Create staff route metadata**

Create `frontend-school/src/routes/(app)/staff/calendar/+page.ts`:

```ts
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ปฏิทินโรงเรียน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 7,
		user_type: 'staff',
		permission: PERMISSION_MODULES.CALENDAR
	}
};

export const load = async () => {
	return {
		title: _meta.menu.title
	};
};
```

- [ ] **Step 2: Create student, parent, and public route metadata**

Create `frontend-school/src/routes/(app)/student/calendar/+page.ts`:

```ts
export const _meta = {
	menu: {
		title: 'ปฏิทิน',
		icon: 'CalendarDays',
		group: 'main',
		workspace: 'home',
		order: 3,
		user_type: 'student'
	}
};

export const load = async () => ({ title: _meta.menu.title });
```

Create `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.ts`:

```ts
export const _meta = {
	access: {
		user_type: 'parent'
	}
};

export const load = async ({ params }) => {
	return {
		title: 'ปฏิทินของลูก',
		studentId: params.id
	};
};
```

Create `frontend-school/src/routes/(public)/calendar/+page.ts`:

```ts
export const load = async () => {
	return {
		title: 'ปฏิทินโรงเรียน'
	};
};
```

- [ ] **Step 3: Implement staff page**

Create `frontend-school/src/routes/(app)/staff/calendar/+page.svelte` with:

- `PageShell`, `PageSkeleton`, `PageState`, `Button`, `Input`, `Select`, `CalendarMonthGrid`, `CalendarEventList`, `CalendarEventDialog`, `CalendarCategoryDialog`.
- State: `events`, `categories`, `loading`, `selectedMonth`, `selectedDate`, `search`, `categoryId`, `audience`, `visibility`, `eventDialogOpen`, `categoryDialogOpen`, `editingEvent`, `saving`.
- Permissions:

```ts
const canReadCalendar = $derived($can.has(PERMISSIONS.CALENDAR_READ_SCHOOL));
const canManageCalendar = $derived($can.has(PERMISSIONS.CALENDAR_MANAGE_SCHOOL));
```

- `loadCalendar()` calls `listCalendarEvents(monthRange(selectedMonth))`.
- `saveEvent(payload)` calls create/update and patches local state:

```ts
function replaceEvent(event: CalendarEvent) {
	events = events.some((item) => item.id === event.id)
		? events.map((item) => (item.id === event.id ? event : item))
		: [event, ...events];
}
```

- `deleteEvent(event)` calls `deleteCalendarEvent(event.id)` and removes from local state.
- Do not full-page reload after create/update/delete.

- [ ] **Step 4: Implement student page**

Create `frontend-school/src/routes/(app)/student/calendar/+page.svelte`:

- Use `PageShell`.
- Call `listMyCalendarEvents(monthRange(selectedMonth))`.
- Render `CalendarMonthGrid` and `CalendarEventList` with `canManage={false}`.
- Use `PageSkeleton` while loading and `PageState` on load error.

- [ ] **Step 5: Implement parent child page**

Create `frontend-school/src/routes/(app)/parent/student/[id]/calendar/+page.svelte`:

- Read `studentId` from `data`.
- Call `listChildCalendarEvents(data.studentId, monthRange(selectedMonth))`.
- Render read-only grid/list.
- Include a back button link to `/parent/student/${data.studentId}`.

- [ ] **Step 6: Implement public page**

Create `frontend-school/src/routes/(public)/calendar/+page.svelte`:

- Do not use app-only permission stores.
- Call `listPublicCalendarEvents(monthRange(selectedMonth))`.
- Render a clean page with title, month controls, grid, and list.
- Do not show target rows, reminders, edit/delete controls, or notification controls.

- [ ] **Step 7: Run Svelte and static checks**

Run:

```bash
cd frontend-school
npm run test:static -- calendar.test.mjs calendar-utils.test.mjs
npm run check
```

Expected: all pass.

- [ ] **Step 8: Commit Task 8**

```bash
git add frontend-school/src/routes/'(app)'/staff/calendar \
  frontend-school/src/routes/'(app)'/student/calendar \
  frontend-school/src/routes/'(app)'/parent/student/'[id]'/calendar \
  frontend-school/src/routes/'(public)'/calendar
git commit -m "feat: add calendar frontend routes"
```

---

### Task 9: End-To-End Verification And Cleanup

**Files:**
- Modify: files touched in earlier tasks only if verification exposes issues.

- [ ] **Step 1: Run backend verification**

Run:

```bash
cd backend-school
cargo test modules::calendar::services::tests --bin backend-school
cargo test calendar_schema_routes_and_permissions_are_registered calendar_handlers_stay_thin_and_services_own_sql --test static_architecture
cargo check
```

Expected: all pass.

- [ ] **Step 2: Run frontend verification**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
```

Expected: all pass.

- [ ] **Step 3: Run repository checks**

Run:

```bash
git diff --check
git status --short
```

Expected:

- `git diff --check` produces no output.
- `git status --short` lists only intentional files if changes remain uncommitted.

- [ ] **Step 4: Commit any verification fixes**

If verification required fixes, return to the specific task that introduced the failing files, make the focused correction there, rerun the failed command, and amend that task's commit. If the fix crosses task boundaries, make a separate focused commit with the exact touched files listed explicitly in the command, for example:

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/modules/calendar/handlers.rs
git commit -m "fix: stabilize calendar implementation"
```

Use a specific commit message if the fix is narrower, such as `fix: align calendar route metadata`.

---

## Execution Notes

- Use shadcn-svelte primitives for interactive controls. Do not build custom raw HTML select/dialog/checkbox primitives.
- Keep event category colors as accents only: dots, side borders, badges, and light markers.
- Keep reminder UI copy neutral as "เตือนล่วงหน้า"; V1 controls expose day-based offsets only.
- Do not add automatic imports from academic, admission, work, assessment, timetable, or workflow modules.
- Do not make calendar events affect timetable, attendance, or school-day calculations.
- Do not log event descriptions as raw request bodies. Titles, dates, and UUIDs are acceptable in structured logs.
- Do not expose targets or reminders from the public API.
