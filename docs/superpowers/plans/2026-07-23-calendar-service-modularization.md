# Calendar Service Modularization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `calendar/services.rs` into private lifecycle, visibility, notification, and reminder modules without changing calendar APIs or delivery semantics.

**Architecture:** Keep `calendar::services` as the public facade for handlers, parent services, and the background worker. Characterize PostgreSQL behavior first, then move cohesive dependency closures behind private child modules.

**Tech Stack:** Rust 2021, Axum 0.8, sqlx 0.8/PostgreSQL, Tokio broadcast channels/tests, Cargo tooling.

**Approved design:** `docs/superpowers/specs/2026-07-23-calendar-service-modularization-design.md`

## Global Constraints

- Preserve routes, DTOs, permissions, errors, notification text/links, reminder timing, worker behavior, and visibility.
- Preserve SQL, bind order, transactions, advisory locks, target/tag/reminder write order, and soft deletes.
- Do not add migrations, indexes, queues, caches, retries, frontend changes, or new infrastructure.
- Handlers, `parents::services`, and `main.rs` continue using `calendar::services`.
- Child modules remain private and no `mod.rs` is created.
- Use `TEST_DATABASE_URL` for database-backed tests.
- Each task ends with focused tests and a commit.

## Target File Map

**Create:**

- `backend-school/src/modules/calendar/services_tests.rs`
- `backend-school/src/modules/calendar/services/shared.rs`
- `backend-school/src/modules/calendar/services/categories_and_tags.rs`
- `backend-school/src/modules/calendar/services/events.rs`
- `backend-school/src/modules/calendar/services/visibility.rs`
- `backend-school/src/modules/calendar/services/notifications.rs`
- `backend-school/src/modules/calendar/services/reminders.rs`

**Modify:**

- `backend-school/src/modules/calendar.rs`
- `backend-school/src/modules/calendar/services.rs`
- `backend-school/tests/static_architecture.rs`

## Public Compatibility Surface

```rust
mod categories_and_tags;
mod events;
mod notifications;
mod reminders;
mod shared;
mod visibility;

pub use categories_and_tags::{
    create_category, create_tag, hard_delete_category, hard_delete_tag,
    list_categories, list_tags, update_category, update_tag,
};
pub use events::{
    create_event, soft_delete_event, update_event, CalendarEventMutationOutcome,
};
pub use notifications::{
    resolve_event_recipient_user_ids, send_event_notification,
    CalendarNotificationKind, CalendarNotificationSendOutcome,
};
pub use reminders::{
    process_due_calendar_reminders_for_all_tenants, process_due_reminders,
};
pub use shared::{
    dedupe_user_ids, reminder_dates, validate_event_date_time, validate_targets,
};
pub use visibility::{
    list_child_events, list_management_events, list_my_events, list_public_events,
};
```

---

### Task 1: Add Calendar Database Characterization Tests

**Files:**

- Create: `backend-school/src/modules/calendar/services_tests.rs`
- Modify: `backend-school/src/modules/calendar.rs`

**Interfaces:**

- Consumes: current calendar facade, notification broadcast channel, and isolated-schema helpers.
- Produces: behavioral tests for lifecycle, visibility, recipients, and reminders.

- [ ] **Step 1: Register the missing test module**

Add:

```rust
#[cfg(test)]
mod services_tests;
```

Run `cargo test modules::calendar::services_tests --no-run` and confirm failure because the file
does not exist.

- [ ] **Step 2: Add the deterministic fixture**

Create:

```rust
struct CalendarFixture {
    staff_user_id: Uuid,
    student_user_id: Uuid,
    second_student_user_id: Uuid,
    parent_user_id: Uuid,
    grade_level_id: Uuid,
    classroom_id: Uuid,
}
```

Use synthetic users, one grade/classroom, active enrollments, and a parent-child link. Provide
`migrated_pool()` and `notification_channel()` helpers without production hooks.

- [ ] **Step 3: Characterize categories, tags, and event writes**

Add `event_lifecycle_preserves_targets_tags_reminders_and_soft_delete` using public create, update,
management-list, and delete calls. Add
`duplicate_category_and_tag_names_keep_existing_conflict_messages` with case-insensitive
duplicates. Assert target/tag/reminder rows, ordering, update replacement, soft-delete visibility,
and exact conflict messages.

- [ ] **Step 4: Characterize audience visibility and recipients**

Add `management_self_child_and_public_views_enforce_the_current_audiences` with all-user,
staff-only, classroom, and public events. Add
`recipient_resolution_deduplicates_overlapping_all_grade_class_and_user_targets` where one user
matches every target kind. Assert each viewer sees only allowed events and each recipient UUID
appears once.

- [ ] **Step 5: Characterize reminders and failure behavior**

Add `successful_reminder_marks_sent_once_and_second_run_is_idempotent`, receive the tenant
notification from the broadcast channel, and assert `sent_at` changes once while the second
processor run returns zero. Add `reminder_with_no_successful_notification_stays_pending` with an
event whose targets resolve to no active recipient and assert `sent_at` remains null.

- [ ] **Step 6: Run and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::calendar::services_tests -- --test-threads=1
cargo test modules::calendar::services::tests
```

```bash
git add backend-school/src/modules/calendar.rs backend-school/src/modules/calendar/services_tests.rs
git commit -m "test(calendar): characterize service workflows"
```

---

### Task 2: Extract Shared Rules, Categories, and Tags

**Files:**

- Create: `backend-school/src/modules/calendar/services/shared.rs`
- Create: `backend-school/src/modules/calendar/services/categories_and_tags.rs`
- Modify: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Produces: shared validation/date helpers and category/tag lifecycle.

- [ ] **Step 1: Add a failing private-module guard**

Require `mod shared;` and `mod categories_and_tags;`, reject public child modules, and require both
files. Run the test and confirm failure.

- [ ] **Step 2: Extract shared rules**

Move unchanged:

```text
validate_event_date_time, reminder_schedule, reminder_dates, validate_targets,
dedupe_user_ids, dedupe_uuid_ids, normalized_event_range, tenant_today,
current_month_range
```

Move their focused date, target, and deduplication tests.

- [ ] **Step 3: Extract categories and tags**

Move:

```text
DUPLICATE_CATEGORY_MESSAGE, DUPLICATE_TAG_MESSAGE,
CATEGORY_NOT_FOUND_MESSAGE, TAG_NOT_FOUND_MESSAGE,
list_categories, create_category, update_category, hard_delete_category,
list_tags, create_tag, update_tag, hard_delete_tag,
map_category_write_error, normalized_tag_name, map_tag_write_error,
is_duplicate_active_category_name, is_unique_violation_for_constraint
```

Preserve constraint names and messages exactly.

- [ ] **Step 4: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test duplicate_category_and_tag_names_keep_existing_conflict_messages -- --test-threads=1
cargo test modules::calendar::services::shared::tests
cargo test --test static_architecture calendar_service_uses_private_child_modules
cargo check --all-targets
```

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/modules/calendar/services/shared.rs backend-school/src/modules/calendar/services/categories_and_tags.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(calendar): extract shared metadata workflows"
```

---

### Task 3: Extract Event Lifecycle and Visibility

**Files:**

- Create: `backend-school/src/modules/calendar/services/events.rs`
- Create: `backend-school/src/modules/calendar/services/visibility.rs`
- Modify: `backend-school/src/modules/calendar/services.rs`

**Interfaces:**

- Consumes: shared validation/date helpers.
- Produces: event mutations, hydration, and four visibility views.

- [ ] **Step 1: Run lifecycle and visibility characterization**

Run the two relevant tests from Task 1 and confirm pass.

- [ ] **Step 2: Extract event writes**

Move:

```text
CalendarEventMutationOutcome, create_event, update_event, soft_delete_event,
replace_event_targets, replace_event_tags, replace_pending_event_reminders,
EVENT_NOT_FOUND_MESSAGE, INVALID_TAGS_MESSAGE
```

Keep the transaction around the event plus targets/tags/reminders unchanged.

- [ ] **Step 3: Extract visibility and hydration**

Move:

```text
EVENT_SELECT_WITH_CATEGORY, CalendarEventTargetRow, CalendarEventReminderRow,
CalendarEventTagRow, list_targets_for_events, list_reminders_for_events,
list_tags_for_events, hydrate_events, get_event_for_response,
list_management_events, list_my_events, list_child_events, list_public_events,
push_* event filters, target_visible_to_*, retain_* targets,
self_calendar_user_type_access, active_user_type, calendar_search_pattern
```

Expose `get_event_for_response` and the minimum hydration types as `pub(super)` for events/reminders.

- [ ] **Step 4: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::calendar::services_tests -- --test-threads=1
cargo check --all-targets
```

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/modules/calendar/services/events.rs backend-school/src/modules/calendar/services/visibility.rs
git commit -m "refactor(calendar): extract event lifecycle and visibility"
```

---

### Task 4: Extract Notifications

**Files:**

- Create: `backend-school/src/modules/calendar/services/notifications.rs`
- Modify: `backend-school/src/modules/calendar/services.rs`

**Interfaces:**

- Consumes: shared deduplication and hydrated event DTO.
- Produces: recipient resolution and notification delivery outcomes used by events/reminders.

- [ ] **Step 1: Run recipient characterization**

Run `recipient_resolution_deduplicates_overlapping_all_grade_class_and_user_targets`.

- [ ] **Step 2: Move the exact notification closure**

Move:

```text
NotificationRecipientRow, CalendarNotificationKind,
CalendarNotificationSendOutcome, resolve_event_recipient_user_ids,
send_event_notification, calendar_notification_text,
calendar_notification_link_for_user_type
```

Preserve link routing, title/body wording, recipient counts, and failure aggregation.

- [ ] **Step 3: Verify and commit**

```bash
cd backend-school
cargo fmt --all -- --check
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test recipient_resolution_deduplicates_overlapping_all_grade_class_and_user_targets -- --test-threads=1
cargo test modules::calendar::services::notifications::tests
cargo check --all-targets
```

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/modules/calendar/services/notifications.rs
git commit -m "refactor(calendar): extract notification workflows"
```

---

### Task 5: Extract Reminders and Seal the Facade

**Files:**

- Create: `backend-school/src/modules/calendar/services/reminders.rs`
- Modify: `backend-school/src/modules/calendar/services.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**

- Consumes: notification delivery and event hydration.
- Produces: per-tenant and cross-tenant reminder entry points plus final facade.

- [ ] **Step 1: Run reminder characterization**

Run both reminder tests from Task 1 and confirm pass.

- [ ] **Step 2: Move the complete reminder closure**

Move:

```text
SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL,
REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL,
MARK_CALENDAR_REMINDER_SENT_SQL,
TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL,
RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL,
DueCalendarEventReminderRow, process_due_reminders,
fetch_due_reminder_candidates, process_due_reminder_candidate,
process_advisory_locked_reminder, mark_calendar_reminder_sent,
release_calendar_reminder_advisory_lock, calendar_reminder_advisory_lock_keys,
process_due_calendar_reminders_for_all_tenants and SQL getter helpers
```

Preserve session-level advisory lock keys/order and successful-delivery marking semantics.

- [ ] **Step 3: Add a failing facade-surface guard**

Require every declaration/re-export listed above, reject database/query/transaction tokens, and
cap nonblank facade lines at 75. Confirm failure before reducing the facade.
Update every existing calendar architecture test that reads implementation text from
`calendar/services.rs` to read the owning child file or an explicit concatenation of all child
files. Keep handler, parent-service, and `main.rs` caller checks unchanged.

- [ ] **Step 4: Replace facade, verify, and commit**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test modules::calendar -- --test-threads=1
cargo test --test static_architecture calendar -- --test-threads=1
```

```bash
git add backend-school/src/modules/calendar/services.rs backend-school/src/modules/calendar/services/reminders.rs backend-school/tests/static_architecture.rs
git commit -m "refactor(calendar): finish modular service facade"
```

---

### Task 6: Full Phase Verification

**Files:**

- Modify only for proven regressions.

- [ ] **Step 1: Run complete backend gate**

```bash
cd backend-school
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
TEST_DATABASE_URL="$TEST_DATABASE_URL" cargo test --bin backend-school -- --test-threads=1
cargo test --test static_architecture -- --test-threads=1
```

- [ ] **Step 2: Audit callers and scope**

```bash
rg -n 'calendar::services' backend-school/src/main.rs backend-school/src/modules/parents backend-school/src/modules/calendar
rg -n 'sqlx::|\.fetch_|\.execute\(|\.begin\(' backend-school/src/modules/calendar/services.rs
git diff --check
git status --short
```

Expected: main/parents/handlers still use the facade, no SQL remains in it, no frontend/migration
files changed, and the worktree is clean.
