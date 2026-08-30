-- Establish effective learning-group teacher episodes and make exact timetable
-- entry instructors authoritative. This migration is intentionally forward-only.

CREATE TEMP TABLE academic_054_preflight_counts ON COMMIT DROP AS
SELECT (SELECT count(*) FROM learning_group_teachers) AS teacher_assignment_count,
       (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
       (SELECT count(*) FROM timetable_entry_instructors) AS instructor_count,
       (SELECT count(*)
          FROM timetable_entry_instructors instructor
          JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
         WHERE entry.learning_group_id IS NULL) AS structural_instructor_count,
       (SELECT count(*)
          FROM academic_timetable_entries entry
          JOIN learning_group_teachers teacher
            ON teacher.learning_group_id = entry.learning_group_id
         WHERE entry.learning_group_id IS NOT NULL) AS expected_group_instructor_count,
       (SELECT count(*) FROM academic_timetable_versions) AS version_count,
       (SELECT count(*) FROM academic_timetable_version_targets) AS target_count,
       (SELECT count(*) FROM academic_term_change_sets) AS change_set_count;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries entry
        WHERE entry.learning_group_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1
              FROM learning_group_teachers teacher
              WHERE teacher.learning_group_id = entry.learning_group_id
          )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_ENTRY_INSTRUCTORS_UNMAPPABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers teacher
        JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
        JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
        JOIN users user_account ON user_account.id = teacher.teacher_id
        WHERE teacher.academic_term_id <> learning_group.academic_term_id
           OR teacher.academic_year_id <> learning_group.academic_year_id
           OR learning_group.academic_term_id <> offering.academic_term_id
           OR learning_group.academic_year_id <> offering.academic_year_id
           OR offering.starts_on IS NULL
           OR user_account.user_type <> 'staff'
           OR user_account.status <> 'active'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_TEACHER_ASSIGNMENT_INVALID'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH current_entry_teacher AS (
            SELECT entry.id AS entry_id,
                   entry.timetable_version_id,
                   entry.day_of_week,
                   entry.bell_schedule_period_id,
                   teacher.teacher_id
            FROM academic_timetable_entries entry
            JOIN learning_group_teachers teacher
              ON teacher.learning_group_id = entry.learning_group_id
            WHERE entry.learning_group_id IS NOT NULL
              AND entry.is_active
            UNION ALL
            SELECT entry.id,
                   entry.timetable_version_id,
                   entry.day_of_week,
                   entry.bell_schedule_period_id,
                   instructor.instructor_id
            FROM academic_timetable_entries entry
            JOIN timetable_entry_instructors instructor ON instructor.entry_id = entry.id
            WHERE entry.learning_group_id IS NULL
              AND entry.is_active
        )
        SELECT 1
        FROM current_entry_teacher left_entry
        JOIN current_entry_teacher right_entry
          ON right_entry.timetable_version_id = left_entry.timetable_version_id
         AND right_entry.day_of_week = left_entry.day_of_week
         AND right_entry.bell_schedule_period_id = left_entry.bell_schedule_period_id
         AND right_entry.teacher_id = left_entry.teacher_id
         AND right_entry.entry_id > left_entry.entry_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_CURRENT_TEACHER_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries left_entry
        JOIN academic_timetable_entries right_entry
          ON right_entry.timetable_version_id = left_entry.timetable_version_id
         AND right_entry.day_of_week = left_entry.day_of_week
         AND right_entry.bell_schedule_period_id = left_entry.bell_schedule_period_id
         AND right_entry.learning_group_id = left_entry.learning_group_id
         AND right_entry.id > left_entry.id
         AND right_entry.is_active
        WHERE left_entry.is_active
          AND left_entry.learning_group_id IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_EXISTING_GROUP_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH entry_homeroom AS (
            SELECT entry.id AS entry_id,
                   entry.timetable_version_id,
                   entry.day_of_week,
                   entry.bell_schedule_period_id,
                   coverage.homeroom_id
            FROM academic_timetable_entries entry
            JOIN learning_group_homerooms coverage
              ON coverage.learning_group_id = entry.learning_group_id
            WHERE entry.is_active
            UNION
            SELECT entry.id,
                   entry.timetable_version_id,
                   entry.day_of_week,
                   entry.bell_schedule_period_id,
                   entry.homeroom_id
            FROM academic_timetable_entries entry
            WHERE entry.is_active
              AND entry.homeroom_id IS NOT NULL
        )
        SELECT 1
        FROM entry_homeroom left_entry
        JOIN entry_homeroom right_entry
          ON right_entry.timetable_version_id = left_entry.timetable_version_id
         AND right_entry.day_of_week = left_entry.day_of_week
         AND right_entry.bell_schedule_period_id = left_entry.bell_schedule_period_id
         AND right_entry.homeroom_id = left_entry.homeroom_id
         AND right_entry.entry_id > left_entry.entry_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_EXISTING_HOMEROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries left_entry
        JOIN academic_timetable_entries right_entry
          ON right_entry.timetable_version_id = left_entry.timetable_version_id
         AND right_entry.day_of_week = left_entry.day_of_week
         AND right_entry.bell_schedule_period_id = left_entry.bell_schedule_period_id
         AND right_entry.room_id = left_entry.room_id
         AND right_entry.id > left_entry.id
         AND right_entry.is_active
        WHERE left_entry.is_active
          AND left_entry.room_id IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_EXISTING_ROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

ALTER TABLE learning_group_teachers
    ADD COLUMN starts_on DATE,
    ADD COLUMN ends_on DATE,
    ADD COLUMN started_by_change_set_id UUID,
    ADD COLUMN ended_by_change_set_id UUID,
    ADD COLUMN row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    ADD COLUMN created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Migration 052 protects published-group assignments from runtime mutation.
-- The cutover must update those legacy rows once, inside this atomic migration,
-- before restoring the same runtime guard.
DROP TRIGGER learning_group_teachers_published_immutable
ON learning_group_teachers;

UPDATE learning_group_teachers teacher
SET starts_on = offering.starts_on,
    updated_at = teacher.created_at,
    migration_provenance = teacher.migration_provenance
        || jsonb_build_object(
            'exactInstructorCutover',
            jsonb_build_object('migration', 54)
        )
FROM learning_groups learning_group
JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
WHERE learning_group.id = teacher.learning_group_id;

CREATE TRIGGER learning_group_teachers_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON learning_group_teachers
FOR EACH ROW EXECUTE FUNCTION academic_protect_published_group_teachers();

ALTER TABLE learning_group_teachers
    ALTER COLUMN starts_on SET NOT NULL,
    ADD CONSTRAINT learning_group_teachers_interval_check
        CHECK (ends_on IS NULL OR ends_on >= starts_on),
    ADD CONSTRAINT learning_group_teachers_started_change_set_fkey
        FOREIGN KEY (started_by_change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    ADD CONSTRAINT learning_group_teachers_ended_change_set_fkey
        FOREIGN KEY (ended_by_change_set_id, academic_term_id, academic_year_id)
        REFERENCES academic_term_change_sets(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    DROP CONSTRAINT learning_group_teachers_unique_key,
    ADD CONSTRAINT learning_group_teachers_episode_key
        UNIQUE (learning_group_id, teacher_id, starts_on);

CREATE OR REPLACE FUNCTION academic_validate_group_teacher_interval()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    new_lock_key BIGINT;
    old_lock_key BIGINT;
BEGIN
    new_lock_key := hashtextextended(
        NEW.learning_group_id::TEXT || ':' || NEW.teacher_id::TEXT,
        0
    );

    IF TG_OP = 'UPDATE' THEN
        old_lock_key := hashtextextended(
            OLD.learning_group_id::TEXT || ':' || OLD.teacher_id::TEXT,
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
    FROM learning_group_teachers teacher
    WHERE teacher.learning_group_id = NEW.learning_group_id
      AND teacher.teacher_id = NEW.teacher_id
      AND teacher.id <> NEW.id
    ORDER BY teacher.id
    FOR UPDATE;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers teacher
        WHERE teacher.learning_group_id = NEW.learning_group_id
          AND teacher.teacher_id = NEW.teacher_id
          AND teacher.id <> NEW.id
          AND daterange(teacher.starts_on, teacher.ends_on, '[]')
              && daterange(NEW.starts_on, NEW.ends_on, '[]')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_GROUP_TEACHER_INTERVAL_OVERLAP'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER learning_group_teachers_interval_guard
BEFORE INSERT OR UPDATE OF learning_group_id, teacher_id, starts_on, ends_on
ON learning_group_teachers
FOR EACH ROW EXECUTE FUNCTION academic_validate_group_teacher_interval();

DELETE FROM timetable_entry_instructors instructor
USING academic_timetable_entries entry
WHERE entry.id = instructor.entry_id
  AND entry.learning_group_id IS NOT NULL;

WITH ordered_teacher AS (
    SELECT entry.id AS entry_id,
           teacher.teacher_id,
           row_number() OVER (
               PARTITION BY entry.id
               ORDER BY CASE teacher.role
                   WHEN 'primary' THEN 1
                   WHEN 'secondary' THEN 2
                   ELSE 3
               END,
               teacher.starts_on,
               teacher.id
           ) AS teacher_order
    FROM academic_timetable_entries entry
    JOIN learning_group_teachers teacher
      ON teacher.learning_group_id = entry.learning_group_id
    WHERE entry.learning_group_id IS NOT NULL
)
INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
SELECT uuid_generate_v5(
           'f291607b-fef7-56f8-a679-ad9d37e3bc75'::UUID,
           'exact-instructor:' || entry_id::TEXT || ':' || teacher_id::TEXT
       ),
       entry_id,
       teacher_id,
       CASE WHEN teacher_order = 1 THEN 'primary' ELSE 'secondary' END
FROM ordered_teacher
ORDER BY entry_id, teacher_order;

DROP TRIGGER academic_timetable_entries_version_immutable
ON academic_timetable_entries;

UPDATE academic_timetable_entries
SET migration_provenance = migration_provenance
    || jsonb_build_object(
        'exactInstructorCutover',
        jsonb_build_object('migration', 54)
    )
WHERE learning_group_id IS NOT NULL;

CREATE TRIGGER academic_timetable_entries_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_entries
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_version_child();

CREATE OR REPLACE FUNCTION academic_validate_timetable_slot_conflicts()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    slot_lock_key BIGINT;
BEGIN
    IF NOT NEW.is_active THEN
        RETURN NEW;
    END IF;

    slot_lock_key := hashtextextended(
        NEW.timetable_version_id::TEXT || ':' || NEW.day_of_week || ':'
            || NEW.bell_schedule_period_id::TEXT,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    IF NEW.learning_group_id IS NOT NULL AND EXISTS (
        SELECT 1
        FROM academic_timetable_entries other_entry
        WHERE other_entry.timetable_version_id = NEW.timetable_version_id
          AND other_entry.day_of_week = NEW.day_of_week
          AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
          AND other_entry.learning_group_id = NEW.learning_group_id
          AND other_entry.id <> NEW.id
          AND other_entry.is_active
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_GROUP_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_homeroom AS (
            SELECT coverage.homeroom_id
            FROM learning_group_homerooms coverage
            WHERE NEW.learning_group_id IS NOT NULL
              AND coverage.learning_group_id = NEW.learning_group_id
            UNION
            SELECT NEW.homeroom_id WHERE NEW.homeroom_id IS NOT NULL
        ),
        occupied_homeroom AS (
            SELECT other_entry.id AS entry_id, coverage.homeroom_id
            FROM academic_timetable_entries other_entry
            JOIN learning_group_homerooms coverage
              ON coverage.learning_group_id = other_entry.learning_group_id
            WHERE other_entry.timetable_version_id = NEW.timetable_version_id
              AND other_entry.day_of_week = NEW.day_of_week
              AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
              AND other_entry.id <> NEW.id
              AND other_entry.is_active
            UNION
            SELECT other_entry.id, other_entry.homeroom_id
            FROM academic_timetable_entries other_entry
            WHERE other_entry.timetable_version_id = NEW.timetable_version_id
              AND other_entry.day_of_week = NEW.day_of_week
              AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
              AND other_entry.id <> NEW.id
              AND other_entry.is_active
              AND other_entry.homeroom_id IS NOT NULL
        )
        SELECT 1
        FROM candidate_homeroom candidate
        JOIN occupied_homeroom occupied
          ON occupied.homeroom_id = candidate.homeroom_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF NEW.room_id IS NOT NULL AND EXISTS (
        SELECT 1
        FROM academic_timetable_entries other_entry
        WHERE other_entry.timetable_version_id = NEW.timetable_version_id
          AND other_entry.day_of_week = NEW.day_of_week
          AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
          AND other_entry.room_id = NEW.room_id
          AND other_entry.id <> NEW.id
          AND other_entry.is_active
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_ROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER academic_timetable_entries_slot_conflict_guard
BEFORE INSERT OR UPDATE OF timetable_version_id, day_of_week,
    bell_schedule_period_id, learning_group_id, homeroom_id, room_id, is_active
ON academic_timetable_entries
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_slot_conflicts();

CREATE OR REPLACE FUNCTION check_entry_move_no_instructor_conflict()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    has_conflict BOOLEAN;
    slot_lock_key BIGINT;
BEGIN
    IF OLD.timetable_version_id = NEW.timetable_version_id
       AND OLD.day_of_week = NEW.day_of_week
       AND OLD.bell_schedule_period_id = NEW.bell_schedule_period_id
       AND (OLD.is_active IS NOT DISTINCT FROM NEW.is_active OR NOT NEW.is_active)
    THEN
        RETURN NEW;
    END IF;

    IF NOT NEW.is_active THEN
        RETURN NEW;
    END IF;

    slot_lock_key := hashtextextended(
        NEW.timetable_version_id::TEXT || ':' || NEW.day_of_week || ':'
            || NEW.bell_schedule_period_id::TEXT,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    SELECT EXISTS (
        SELECT 1
        FROM timetable_entry_instructors own_instructor
        JOIN timetable_entry_instructors other_instructor
          ON other_instructor.instructor_id = own_instructor.instructor_id
        JOIN academic_timetable_entries other_entry
          ON other_entry.id = other_instructor.entry_id
        WHERE own_instructor.entry_id = NEW.id
          AND other_entry.id <> NEW.id
          AND other_entry.timetable_version_id = NEW.timetable_version_id
          AND other_entry.day_of_week = NEW.day_of_week
          AND other_entry.bell_schedule_period_id = NEW.bell_schedule_period_id
          AND other_entry.is_active
    ) INTO has_conflict;

    IF has_conflict THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION check_instructor_no_double_book()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    entry_version UUID;
    entry_day VARCHAR(10);
    entry_period UUID;
    entry_active BOOLEAN;
    has_conflict BOOLEAN;
    slot_lock_key BIGINT;
BEGIN
    SELECT timetable_version_id,
           day_of_week,
           bell_schedule_period_id,
           is_active
      INTO entry_version,
           entry_day,
           entry_period,
           entry_active
    FROM academic_timetable_entries
    WHERE id = NEW.entry_id;

    IF entry_day IS NULL OR NOT entry_active THEN
        RETURN NEW;
    END IF;

    slot_lock_key := hashtextextended(
        entry_version::TEXT || ':' || entry_day || ':' || entry_period::TEXT,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    SELECT EXISTS (
        SELECT 1
        FROM timetable_entry_instructors other_instructor
        JOIN academic_timetable_entries other_entry
          ON other_entry.id = other_instructor.entry_id
        WHERE other_instructor.instructor_id = NEW.instructor_id
          AND other_entry.timetable_version_id = entry_version
          AND other_entry.day_of_week = entry_day
          AND other_entry.bell_schedule_period_id = entry_period
          AND other_entry.id <> NEW.entry_id
          AND other_entry.is_active
    ) INTO has_conflict;

    IF has_conflict THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER tei_prevent_double_book ON timetable_entry_instructors;
CREATE TRIGGER timetable_entry_instructors_exact_conflict_guard
BEFORE INSERT OR UPDATE OF entry_id, instructor_id
ON timetable_entry_instructors
FOR EACH ROW EXECUTE FUNCTION check_instructor_no_double_book();

CREATE OR REPLACE FUNCTION academic_protect_timetable_entry_instructor()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_entry_id UUID;
BEGIN
    affected_entry_id := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.entry_id
        ELSE OLD.entry_id
    END;

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries entry
        JOIN academic_timetable_versions version
          ON version.id = entry.timetable_version_id
        WHERE entry.id = affected_entry_id
          AND version.status = 'published'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    IF TG_OP = 'UPDATE'
       AND NEW.entry_id <> OLD.entry_id
       AND EXISTS (
           SELECT 1
           FROM academic_timetable_entries entry
           JOIN academic_timetable_versions version
             ON version.id = entry.timetable_version_id
           WHERE entry.id = NEW.entry_id
             AND version.status = 'published'
       )
    THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER timetable_entry_instructors_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON timetable_entry_instructors
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_entry_instructor();

DO $$
DECLARE
    before_counts RECORD;
    after_counts RECORD;
    enabled_trigger_count INTEGER;
BEGIN
    SELECT * INTO before_counts FROM academic_054_preflight_counts;
    SELECT (SELECT count(*) FROM learning_group_teachers) AS teacher_assignment_count,
           (SELECT count(*) FROM academic_timetable_entries) AS entry_count,
           (SELECT count(*) FROM timetable_entry_instructors) AS instructor_count,
           (SELECT count(*) FROM academic_timetable_versions) AS version_count,
           (SELECT count(*) FROM academic_timetable_version_targets) AS target_count,
           (SELECT count(*) FROM academic_term_change_sets) AS change_set_count
      INTO after_counts;

    IF before_counts.teacher_assignment_count <> after_counts.teacher_assignment_count
       OR before_counts.entry_count <> after_counts.entry_count
       OR before_counts.version_count <> after_counts.version_count
       OR before_counts.target_count <> after_counts.target_count
       OR before_counts.change_set_count <> after_counts.change_set_count
       OR before_counts.structural_instructor_count
            + before_counts.expected_group_instructor_count <> after_counts.instructor_count
    THEN
        RAISE EXCEPTION 'ACADEMIC_054_ROW_PRESERVATION_FAILED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM learning_group_teachers teacher
        JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
        JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
        WHERE teacher.starts_on <> offering.starts_on
           OR teacher.ends_on < teacher.starts_on
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_TEACHER_EPISODE_RECONCILIATION_FAILED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_entries entry
        WHERE entry.learning_group_id IS NOT NULL
          AND ARRAY(
                  SELECT instructor.instructor_id
                  FROM timetable_entry_instructors instructor
                  WHERE instructor.entry_id = entry.id
                  ORDER BY instructor.instructor_id
              ) IS DISTINCT FROM ARRAY(
                  SELECT teacher.teacher_id
                  FROM learning_group_teachers teacher
                  WHERE teacher.learning_group_id = entry.learning_group_id
                  ORDER BY teacher.teacher_id
              )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_054_ENTRY_INSTRUCTOR_RECONCILIATION_FAILED';
    END IF;

    SELECT count(*)
      INTO enabled_trigger_count
    FROM pg_trigger trigger_row
    JOIN pg_class target_table ON target_table.oid = trigger_row.tgrelid
    JOIN pg_namespace target_schema ON target_schema.oid = target_table.relnamespace
    WHERE target_schema.nspname = current_schema()
      AND trigger_row.tgname IN (
        'academic_timetable_entries_slot_conflict_guard',
        'learning_group_teachers_interval_guard',
        'timetable_entry_instructors_exact_conflict_guard',
        'timetable_entry_instructors_version_immutable'
    )
      AND trigger_row.tgenabled = 'O';

    IF enabled_trigger_count <> 4 THEN
        RAISE EXCEPTION 'ACADEMIC_054_REQUIRED_TRIGGER_MISSING';
    END IF;
END;
$$;
