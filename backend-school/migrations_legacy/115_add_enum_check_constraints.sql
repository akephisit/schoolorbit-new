-- Add CHECK constraints for enum-like VARCHAR columns
-- Verified safe via backend-school/examples/inspect_db.rs (2026-05-22):
-- All existing rows match the enum values declared here.

-- ============================================
-- users
-- ============================================
ALTER TABLE users
    ADD CONSTRAINT check_users_user_type
        CHECK (user_type IN ('student', 'staff', 'parent'));

ALTER TABLE users
    ADD CONSTRAINT check_users_status
        CHECK (status IN ('active', 'inactive', 'suspended', 'resigned'));

-- ============================================
-- student_class_enrollments
-- ============================================
ALTER TABLE student_class_enrollments
    ADD CONSTRAINT check_enrollments_enrollment_type
        CHECK (enrollment_type IN ('regular', 'transferred_in', 'repeated'));

ALTER TABLE student_class_enrollments
    ADD CONSTRAINT check_enrollments_status
        CHECK (status IN ('active', 'transferred', 'dropped', 'completed'));

-- ============================================
-- consent_records (PDPA compliance — strict enum required)
-- ============================================
ALTER TABLE consent_records
    ADD CONSTRAINT check_consent_status
        CHECK (consent_status IN ('pending', 'granted', 'denied', 'withdrawn'));

ALTER TABLE consent_records
    ADD CONSTRAINT check_consent_method
        CHECK (consent_method IN ('web_form', 'paper_form', 'verbal', 'implicit'));

-- ============================================
-- rooms (facility status)
-- ============================================
ALTER TABLE rooms
    ADD CONSTRAINT check_rooms_status
        CHECK (status IN ('ACTIVE', 'MAINTENANCE', 'INACTIVE'));

-- ============================================
-- activity_catalog (scheduling mode determines slot behavior)
-- ============================================
ALTER TABLE activity_catalog
    ADD CONSTRAINT check_activity_catalog_scheduling_mode
        CHECK (scheduling_mode IN ('synchronized', 'independent'));

-- ============================================
-- staff_info (employment type; NULL allowed for legacy rows)
-- ============================================
ALTER TABLE staff_info
    ADD CONSTRAINT check_staff_employment_type
        CHECK (employment_type IS NULL OR employment_type IN ('permanent', 'contract', 'temporary', 'part_time'));

-- ============================================
-- Notes:
-- - admission_applications.status NOT constrained here — state machine has
--   more values than the migration 046 comment lists (accepted/absent/enrolled
--   observed in production). Document the full set before adding CHECK.
-- - student_parents.relationship and consent_records.parent_relationship are
--   intentionally free-form (father/mother/guardian/grandparent/uncle/aunt/etc.).
-- - day_of_week already has CHECK in academic_timetable_entries (migration 016).
--   timetable_locked_slots and activity_slot_days inherit semantic via FK or
--   are populated from constrained sources.
-- ============================================
