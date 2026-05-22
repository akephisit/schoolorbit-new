-- Document JSONB column schemas via COMMENT ON COLUMN.
-- Verified actual key usage via backend-school/examples/inspect_db.rs (2026-05-22).

-- ============================================
-- Columns with active structured data (schema enforced by application code)
-- ============================================

COMMENT ON COLUMN admission_rounds.report_config IS
$$Report layout config for admission round result PDF.
Keys (verified used in production):
  - institution: TEXT — header institution name shown on report
  - reportMode: TEXT — 'per_track' | 'global' (selection ranking mode)
  - zone: TEXT — geographic zone label for grouping
Source of truth: frontend admission round detail page form.$$;

COMMENT ON COLUMN timetable_scheduling_jobs.config IS
$$Auto-scheduler run configuration.
Keys (verified used in production):
  - allow_partial: BOOLEAN — accept partial solutions if no full solution found
  - force_overwrite: BOOLEAN — replace existing entries instead of preserving
  - min_quality_score: NUMERIC — reject solutions below this score (0.0-1.0)
  - respect_preferences: BOOLEAN — honor instructor_preferences table
  - timeout_seconds: INTEGER — max compute time before aborting
  - weight_consecutive: NUMERIC — penalty multiplier for non-consecutive periods
  - weight_daily_load: NUMERIC — penalty multiplier for uneven daily distribution
  - weight_distribution: NUMERIC — penalty for clustering same subject
  - weight_instructor_preference: NUMERIC — bonus for matching preferences
  - weight_time_of_day: NUMERIC — bonus for matching time-of-day prefs
Source of truth: scheduler service config (services/scheduler/types.rs).$$;

-- ============================================
-- Metadata columns — intentionally free-form (currently empty across all rows)
-- ============================================
-- These columns exist as schema escape hatches. No fixed schema is enforced.
-- If a key becomes load-bearing for queries or business logic, normalize it
-- into a proper column rather than relying on JSONB.

COMMENT ON COLUMN users.metadata IS
'Free-form per-user metadata. No fixed schema. Normalize keys into columns if they become load-bearing.';

COMMENT ON COLUMN staff_info.metadata IS
'Free-form per-staff metadata. No fixed schema.';

COMMENT ON COLUMN student_info.metadata IS
'Free-form per-student metadata. No fixed schema.';

COMMENT ON COLUMN parent_info.metadata IS
'Free-form per-parent metadata. No fixed schema.';

COMMENT ON COLUMN classroom_courses.settings IS
'Free-form per-course settings. No fixed schema.';

COMMENT ON COLUMN admission_applications.metadata IS
'Free-form per-application metadata. No fixed schema.';

COMMENT ON COLUMN consent_records.metadata IS
'Free-form per-consent metadata (consent_text version, UI context, etc.). No fixed schema.';

COMMENT ON COLUMN menu_items.metadata IS
'Free-form per-menu-item metadata. No fixed schema.';

-- ============================================
-- Audit columns — capture arbitrary state snapshots
-- ============================================

COMMENT ON COLUMN audit_logs.metadata IS
'Arbitrary context for the audit event (request_id, ip, user_agent, ...). Not a fixed schema; varies per action type.';

COMMENT ON COLUMN audit_logs.old_values IS
'Snapshot of affected row BEFORE the change. Shape mirrors the target table; NULL for create events.';

COMMENT ON COLUMN audit_logs.new_values IS
'Snapshot of affected row AFTER the change. Shape mirrors the target table; NULL for delete events.';
