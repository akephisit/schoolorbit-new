-- Release 2 database guards for effective-from academic operational changes.
-- Migration 052 owns the foundational tables; this migration adds only the
-- request binding and roster interval invariants needed by the runtime workflow.

CREATE TEMP TABLE academic_053_preflight_counts ON COMMIT DROP AS
SELECT (SELECT count(*) FROM learning_offerings) AS offering_count,
       (SELECT count(*) FROM academic_timetable_versions) AS version_count,
       (SELECT count(*) FROM academic_timetable_version_targets) AS target_count,
       (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
       (SELECT count(*) FROM learning_group_students) AS membership_count;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM learning_group_students left_membership
        JOIN learning_group_students right_membership
          ON right_membership.learning_group_id = left_membership.learning_group_id
         AND right_membership.student_id = left_membership.student_id
         AND right_membership.id > left_membership.id
         AND daterange(
                 right_membership.joined_at,
                 right_membership.left_at,
                 '[]'
             ) && daterange(
                 left_membership.joined_at,
                 left_membership.left_at,
                 '[]'
             )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_053_EXISTING_ROSTER_INTERVAL_OVERLAP'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

ALTER TABLE academic_term_change_sets
    ADD COLUMN creation_request_hash TEXT,
    ADD COLUMN publication_idempotency_key UUID,
    ADD COLUMN publication_request_hash TEXT,
    ADD COLUMN acknowledged_warning_codes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[];

UPDATE academic_term_change_sets
SET creation_request_hash = repeat('0', 64);

ALTER TABLE academic_term_change_sets
    ALTER COLUMN creation_request_hash SET NOT NULL,
    DROP CONSTRAINT academic_term_change_sets_status_metadata_check,
    ADD CONSTRAINT academic_term_change_sets_status_metadata_check CHECK (
        creation_request_hash ~ '^[0-9a-f]{64}$'
        AND (
            (status = 'draft'
             AND published_by IS NULL
             AND published_at IS NULL
             AND cancelled_by IS NULL
             AND cancelled_at IS NULL
             AND publication_idempotency_key IS NULL
             AND publication_request_hash IS NULL
             AND cardinality(acknowledged_warning_codes) = 0)
            OR
            (status = 'published'
             AND published_by IS NOT NULL
             AND published_at IS NOT NULL
             AND cancelled_by IS NULL
             AND cancelled_at IS NULL
             AND publication_idempotency_key IS NOT NULL
             AND publication_request_hash ~ '^[0-9a-f]{64}$')
            OR
            (status = 'cancelled'
             AND published_by IS NULL
             AND published_at IS NULL
             AND cancelled_by IS NOT NULL
             AND cancelled_at IS NOT NULL
             AND publication_idempotency_key IS NULL
             AND publication_request_hash IS NULL
             AND cardinality(acknowledged_warning_codes) = 0)
        )
    );

CREATE UNIQUE INDEX academic_term_change_sets_publication_idempotency_key
    ON academic_term_change_sets(publication_idempotency_key)
    WHERE publication_idempotency_key IS NOT NULL;

CREATE OR REPLACE FUNCTION academic_validate_roster_membership_interval()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    new_lock_key BIGINT;
    old_lock_key BIGINT;
BEGIN
    new_lock_key := hashtextextended(
        NEW.learning_group_id::TEXT || ':' || NEW.student_id::TEXT,
        0
    );

    IF TG_OP = 'UPDATE' THEN
        old_lock_key := hashtextextended(
            OLD.learning_group_id::TEXT || ':' || OLD.student_id::TEXT,
            0
        );
        PERFORM pg_advisory_xact_lock(LEAST(new_lock_key, old_lock_key));
        IF new_lock_key <> old_lock_key THEN
            PERFORM pg_advisory_xact_lock(GREATEST(new_lock_key, old_lock_key));
        END IF;
    ELSE
        PERFORM pg_advisory_xact_lock(new_lock_key);
    END IF;

    PERFORM 1
    FROM learning_group_students membership
    WHERE membership.learning_group_id = NEW.learning_group_id
      AND membership.student_id = NEW.student_id
      AND membership.id <> NEW.id
    ORDER BY membership.id
    FOR UPDATE;

    IF NEW.left_at IS NULL OR NEW.left_at >= NEW.joined_at THEN
        IF EXISTS (
            SELECT 1
            FROM learning_group_students membership
            WHERE membership.learning_group_id = NEW.learning_group_id
              AND membership.student_id = NEW.student_id
              AND membership.id <> NEW.id
              AND daterange(
                      membership.joined_at,
                      membership.left_at,
                      '[]'
                  ) && daterange(NEW.joined_at, NEW.left_at, '[]')
        ) THEN
            RAISE EXCEPTION 'ACADEMIC_ROSTER_MEMBERSHIP_INTERVAL_OVERLAP'
                USING ERRCODE = 'check_violation';
        END IF;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER learning_group_students_interval_guard
BEFORE INSERT OR UPDATE OF learning_group_id, student_id, joined_at, left_at
ON learning_group_students
FOR EACH ROW EXECUTE FUNCTION academic_validate_roster_membership_interval();

DO $$
DECLARE
    before_counts RECORD;
    after_counts RECORD;
    enabled_trigger_count INTEGER;
BEGIN
    SELECT * INTO before_counts FROM academic_053_preflight_counts;
    SELECT (SELECT count(*) FROM learning_offerings) AS offering_count,
           (SELECT count(*) FROM academic_timetable_versions) AS version_count,
           (SELECT count(*) FROM academic_timetable_version_targets) AS target_count,
           (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
           (SELECT count(*) FROM learning_group_students) AS membership_count
      INTO after_counts;

    IF before_counts.offering_count <> after_counts.offering_count
       OR before_counts.version_count <> after_counts.version_count
       OR before_counts.target_count <> after_counts.target_count
       OR before_counts.entry_count <> after_counts.entry_count
       OR before_counts.membership_count <> after_counts.membership_count
    THEN
        RAISE EXCEPTION 'ACADEMIC_053_ROW_PRESERVATION_FAILED';
    END IF;

    SELECT count(*)
      INTO enabled_trigger_count
    FROM pg_trigger
    WHERE tgname = 'learning_group_students_interval_guard'
      AND tgrelid = 'learning_group_students'::regclass
      AND tgenabled = 'O';

    IF enabled_trigger_count <> 1 THEN
        RAISE EXCEPTION 'ACADEMIC_053_ROSTER_INTERVAL_GUARD_MISSING';
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_indexes
        WHERE schemaname = current_schema()
          AND tablename = 'academic_term_change_sets'
          AND indexname = 'academic_term_change_sets_publication_idempotency_key'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_053_PUBLICATION_IDEMPOTENCY_INDEX_MISSING';
    END IF;
END;
$$;
