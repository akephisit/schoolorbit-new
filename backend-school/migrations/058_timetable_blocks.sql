-- Canonical timetable blocks, synchronized activity reservations, and structural targets.
--
-- This is a hard cutover. The migration builds the complete target model, reconciles
-- every timetable and supervision reference, installs immutable/conflict guards, and
-- removes the entry/batch runtime only after all bounded checks pass.

CREATE TEMP TABLE academic_058_preflight_counts ON COMMIT DROP AS
SELECT (SELECT COUNT(*) FROM academic_timetable_entries) AS entry_count,
       (SELECT COUNT(*)
          FROM academic_timetable_entries
         WHERE entry_type IN ('COURSE', 'ACTIVITY')) AS delivery_entry_count,
       (SELECT COUNT(*)
          FROM timetable_entry_instructors instructor
          JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
         WHERE entry.entry_type IN ('COURSE', 'ACTIVITY')) AS delivery_instructor_count,
       (SELECT COUNT(*)
          FROM supervision_observations
         WHERE timetable_entry_id IS NOT NULL) AS observation_reference_count;

DO $$
DECLARE
    invalid_entry UUID;
    invalid_observation UUID;
BEGIN
    SELECT entry.id
      INTO invalid_entry
      FROM academic_timetable_entries entry
     WHERE entry.entry_type IN ('COURSE', 'ACTIVITY')
       AND (entry.learning_group_id IS NULL OR entry.learning_offering_id IS NULL)
     ORDER BY entry.id
     LIMIT 1;

    IF invalid_entry IS NOT NULL THEN
        RAISE EXCEPTION 'TIMETABLE_BLOCK_PREFLIGHT_DELIVERY_CONTEXT:%', invalid_entry;
    END IF;

    SELECT observation.id
      INTO invalid_observation
      FROM supervision_observations observation
      JOIN academic_timetable_entries entry ON entry.id = observation.timetable_entry_id
     WHERE observation.timetable_entry_id IS NOT NULL
       AND entry.entry_type NOT IN ('COURSE', 'ACTIVITY')
     ORDER BY observation.id
     LIMIT 1;

    IF invalid_observation IS NOT NULL THEN
        RAISE EXCEPTION 'TIMETABLE_BLOCK_PREFLIGHT_SUPERVISION_TARGET:%', invalid_observation;
    END IF;
END;
$$;

CREATE TABLE academic_timetable_blocks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    timetable_version_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    bell_schedule_id UUID NOT NULL,
    bell_schedule_period_id UUID NOT NULL,
    day_of_week TEXT NOT NULL CHECK (
        day_of_week IN ('MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN')
    ),
    block_kind TEXT NOT NULL CHECK (block_kind IN ('COURSE', 'ACTIVITY', 'STRUCTURAL')),
    scheduling_mode TEXT CHECK (scheduling_mode IN ('independent', 'synchronized')),
    learning_offering_id UUID,
    structural_kind TEXT CHECK (
        structural_kind IN (
            'BREAK', 'HOMEROOM', 'FLAG_CEREMONY',
            'TEACHER_MEETING', 'ACADEMIC', 'OTHER'
        )
    ),
    title TEXT,
    note TEXT,
    series_id UUID,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    is_active BOOLEAN NOT NULL DEFAULT true,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_blocks_shape_check CHECK (
        (block_kind = 'COURSE'
         AND scheduling_mode = 'independent'
         AND learning_offering_id IS NOT NULL
         AND structural_kind IS NULL)
        OR
        (block_kind = 'ACTIVITY'
         AND scheduling_mode IS NOT NULL
         AND learning_offering_id IS NOT NULL
         AND structural_kind IS NULL)
        OR
        (block_kind = 'STRUCTURAL'
         AND scheduling_mode IS NULL
         AND learning_offering_id IS NULL
         AND structural_kind IS NOT NULL)
    ),
    CONSTRAINT academic_timetable_blocks_version_context_fkey
        FOREIGN KEY (timetable_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_versions(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_blocks_term_schedule_fkey
        FOREIGN KEY (academic_term_id, bell_schedule_id)
        REFERENCES academic_terms(id, bell_schedule_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_blocks_period_schedule_fkey
        FOREIGN KEY (bell_schedule_period_id, bell_schedule_id)
        REFERENCES bell_schedule_periods(id, bell_schedule_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_blocks_offering_context_fkey
        FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_blocks_id_context_key
        UNIQUE (id, academic_term_id, academic_year_id),
    CONSTRAINT academic_timetable_blocks_id_offering_context_key
        UNIQUE (id, learning_offering_id, academic_term_id, academic_year_id)
);

CREATE TABLE academic_timetable_block_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_id UUID NOT NULL,
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    room_id UUID REFERENCES rooms(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    is_active BOOLEAN NOT NULL DEFAULT true,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_block_groups_block_context_fkey
        FOREIGN KEY (block_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_blocks(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE CASCADE,
    CONSTRAINT academic_timetable_block_groups_group_context_fkey
        FOREIGN KEY (learning_group_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_block_groups_id_context_key
        UNIQUE (id, academic_term_id, academic_year_id),
    CONSTRAINT academic_timetable_block_groups_id_block_key UNIQUE (id, block_id),
    CONSTRAINT academic_timetable_block_groups_block_group_key
        UNIQUE (block_id, learning_group_id)
);

CREATE TABLE academic_timetable_block_group_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_group_id UUID NOT NULL REFERENCES academic_timetable_block_groups(id)
        ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK (role IN ('primary', 'secondary', 'assistant')),
    display_order INTEGER NOT NULL CHECK (display_order > 0),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_block_group_instructors_unique_teacher
        UNIQUE (block_group_id, instructor_id),
    CONSTRAINT academic_timetable_block_group_instructors_unique_order
        UNIQUE (block_group_id, display_order)
);

CREATE TABLE academic_timetable_block_homerooms (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_id UUID NOT NULL,
    homeroom_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('reservation', 'structural')),
    room_id UUID REFERENCES rooms(id) ON DELETE RESTRICT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    is_active BOOLEAN NOT NULL DEFAULT true,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_block_homerooms_block_context_fkey
        FOREIGN KEY (block_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_blocks(id, academic_term_id, academic_year_id)
        ON DELETE CASCADE,
    CONSTRAINT academic_timetable_block_homerooms_homeroom_context_fkey
        FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_block_homerooms_target_key
        UNIQUE (block_id, homeroom_id)
);

CREATE TABLE academic_timetable_block_teachers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_id UUID NOT NULL,
    teacher_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    is_active BOOLEAN NOT NULL DEFAULT true,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_block_teachers_block_context_fkey
        FOREIGN KEY (block_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_blocks(id, academic_term_id, academic_year_id)
        ON DELETE CASCADE,
    CONSTRAINT academic_timetable_block_teachers_target_key UNIQUE (block_id, teacher_id)
);

CREATE TABLE academic_timetable_block_group_sync (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_id UUID NOT NULL,
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('LINKED', 'WAITING_FOR_DATA', 'CONFLICT', 'OUTSIDE_SCOPE', 'EXCLUDED')
    ),
    linked_block_group_id UUID,
    conflict_code TEXT,
    conflict_message TEXT,
    attempted_group_row_version BIGINT,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    updated_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_timetable_block_group_sync_block_context_fkey
        FOREIGN KEY (block_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_blocks(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE CASCADE,
    CONSTRAINT academic_timetable_block_group_sync_group_context_fkey
        FOREIGN KEY (learning_group_id, learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(
            id, learning_offering_id, academic_term_id, academic_year_id
        ) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_block_group_sync_linked_group_fkey
        FOREIGN KEY (linked_block_group_id, block_id)
        REFERENCES academic_timetable_block_groups(id, block_id) ON DELETE RESTRICT,
    CONSTRAINT academic_timetable_block_group_sync_shape_check CHECK (
        (status = 'LINKED'
         AND linked_block_group_id IS NOT NULL
         AND conflict_code IS NULL
         AND conflict_message IS NULL)
        OR
        (status IN ('WAITING_FOR_DATA', 'OUTSIDE_SCOPE', 'EXCLUDED')
         AND linked_block_group_id IS NULL)
        OR
        (status = 'CONFLICT'
         AND linked_block_group_id IS NULL
         AND conflict_code IS NOT NULL
         AND btrim(conflict_code) <> '')
    ),
    CONSTRAINT academic_timetable_block_group_sync_target_key
        UNIQUE (block_id, learning_group_id)
);

CREATE INDEX academic_timetable_blocks_version_slot_idx
    ON academic_timetable_blocks(
        timetable_version_id, day_of_week, bell_schedule_period_id
    ) WHERE is_active;
CREATE INDEX academic_timetable_blocks_version_offering_idx
    ON academic_timetable_blocks(timetable_version_id, learning_offering_id)
    WHERE is_active;
CREATE INDEX academic_timetable_blocks_series_idx
    ON academic_timetable_blocks(timetable_version_id, series_id)
    WHERE is_active AND series_id IS NOT NULL;
CREATE INDEX academic_timetable_block_groups_group_idx
    ON academic_timetable_block_groups(learning_group_id, block_id) WHERE is_active;
CREATE INDEX academic_timetable_block_groups_room_idx
    ON academic_timetable_block_groups(room_id, block_id)
    WHERE is_active AND room_id IS NOT NULL;
CREATE INDEX academic_timetable_block_group_instructors_teacher_idx
    ON academic_timetable_block_group_instructors(instructor_id, block_group_id);
CREATE INDEX academic_timetable_block_homerooms_homeroom_idx
    ON academic_timetable_block_homerooms(homeroom_id, block_id) WHERE is_active;
CREATE INDEX academic_timetable_block_homerooms_room_idx
    ON academic_timetable_block_homerooms(room_id, block_id)
    WHERE is_active AND room_id IS NOT NULL;
CREATE INDEX academic_timetable_block_teachers_teacher_idx
    ON academic_timetable_block_teachers(teacher_id, block_id) WHERE is_active;
CREATE INDEX academic_timetable_block_group_sync_status_idx
    ON academic_timetable_block_group_sync(block_id, status);

CREATE TEMP TABLE academic_058_entry_block_map (
    entry_id UUID PRIMARY KEY,
    block_id UUID NOT NULL
) ON COMMIT DROP;

INSERT INTO academic_058_entry_block_map (entry_id, block_id)
SELECT entry.id,
       CASE
           WHEN entry.entry_type = 'ACTIVITY'
                AND detail.scheduling_mode = 'synchronized'
           THEN uuid_generate_v5(
               'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
               'timetable-block:sync:' || entry.timetable_version_id::text || ':'
                   || entry.learning_offering_id::text || ':' || entry.day_of_week || ':'
                   || entry.bell_schedule_period_id::text
           )
           WHEN entry.entry_type IN ('BREAK', 'HOMEROOM', 'ACADEMIC')
                AND entry.batch_id IS NOT NULL
           THEN uuid_generate_v5(
               'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
               'timetable-block:structural:' || entry.timetable_version_id::text || ':'
                   || entry.batch_id::text || ':' || entry.day_of_week || ':'
                   || entry.bell_schedule_period_id::text
           )
           ELSE uuid_generate_v5(
               'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
               'timetable-block:entry:' || entry.id::text
           )
       END
FROM academic_timetable_entries entry
LEFT JOIN activity_offering_details detail
  ON detail.learning_offering_id = entry.learning_offering_id;

DO $$
DECLARE
    ambiguous_block UUID;
BEGIN
    SELECT map.block_id
      INTO ambiguous_block
      FROM academic_058_entry_block_map map
      JOIN academic_timetable_entries entry ON entry.id = map.entry_id
     GROUP BY map.block_id
    HAVING COUNT(DISTINCT entry.timetable_version_id) <> 1
        OR COUNT(DISTINCT entry.academic_term_id) <> 1
        OR COUNT(DISTINCT entry.academic_year_id) <> 1
        OR COUNT(DISTINCT entry.bell_schedule_id) <> 1
        OR COUNT(DISTINCT entry.bell_schedule_period_id) <> 1
        OR COUNT(DISTINCT entry.day_of_week) <> 1
        OR COUNT(DISTINCT entry.entry_type) <> 1
        OR COUNT(DISTINCT COALESCE(entry.learning_offering_id::text, '')) > 1
        OR COUNT(DISTINCT COALESCE(entry.title, '')) > 1
        OR COUNT(DISTINCT COALESCE(entry.note, '')) > 1
     ORDER BY map.block_id
     LIMIT 1;

    IF ambiguous_block IS NOT NULL THEN
        RAISE EXCEPTION 'TIMETABLE_BLOCK_PREFLIGHT_AMBIGUOUS_BLOCK:%', ambiguous_block;
    END IF;
END;
$$;

INSERT INTO academic_timetable_blocks (
    id, timetable_version_id, academic_term_id, academic_year_id,
    bell_schedule_id, bell_schedule_period_id, day_of_week,
    block_kind, scheduling_mode, learning_offering_id, structural_kind,
    title, note, series_id, row_version, is_active, migration_provenance,
    created_by, updated_by, created_at, updated_at
)
SELECT map.block_id,
       min(entry.timetable_version_id::text)::uuid,
       min(entry.academic_term_id::text)::uuid,
       min(entry.academic_year_id::text)::uuid,
       min(entry.bell_schedule_id::text)::uuid,
       min(entry.bell_schedule_period_id::text)::uuid,
       min(entry.day_of_week),
       CASE
           WHEN min(entry.entry_type) = 'COURSE' THEN 'COURSE'
           WHEN min(entry.entry_type) = 'ACTIVITY' THEN 'ACTIVITY'
           ELSE 'STRUCTURAL'
       END,
       CASE
           WHEN min(entry.entry_type) = 'COURSE' THEN 'independent'
           WHEN min(entry.entry_type) = 'ACTIVITY' THEN min(detail.scheduling_mode)
           ELSE NULL
       END,
       min(entry.learning_offering_id::text)::uuid,
       CASE
           WHEN min(entry.entry_type) = 'BREAK' THEN 'BREAK'
           WHEN min(entry.entry_type) = 'HOMEROOM' THEN 'HOMEROOM'
           WHEN min(entry.entry_type) = 'ACADEMIC' THEN 'ACADEMIC'
           ELSE NULL
       END,
       NULLIF(min(COALESCE(entry.title, '')), ''),
       NULLIF(min(COALESCE(entry.note, '')), ''),
       CASE
           WHEN COUNT(DISTINCT entry.batch_id) = 1
           THEN min(entry.batch_id::text)::uuid
           ELSE NULL
       END,
       greatest(max(entry.row_version), 1),
       bool_or(entry.is_active),
       jsonb_build_object(
           'migration', 58,
           'sourceEntryIds', jsonb_agg(entry.id ORDER BY entry.id)
       ),
       (array_agg(entry.created_by ORDER BY entry.created_at, entry.id)
           FILTER (WHERE entry.created_by IS NOT NULL))[1],
       (array_agg(entry.updated_by ORDER BY entry.updated_at DESC, entry.id)
           FILTER (WHERE entry.updated_by IS NOT NULL))[1],
       min(entry.created_at),
       max(entry.updated_at)
FROM academic_058_entry_block_map map
JOIN academic_timetable_entries entry ON entry.id = map.entry_id
LEFT JOIN activity_offering_details detail
  ON detail.learning_offering_id = entry.learning_offering_id
GROUP BY map.block_id;

INSERT INTO academic_timetable_block_groups (
    id, block_id, learning_group_id, learning_offering_id,
    academic_term_id, academic_year_id, room_id,
    row_version, is_active, migration_provenance,
    created_by, updated_by, created_at, updated_at
)
SELECT entry.id,
       map.block_id,
       entry.learning_group_id,
       entry.learning_offering_id,
       entry.academic_term_id,
       entry.academic_year_id,
       entry.room_id,
       entry.row_version,
       entry.is_active,
       entry.migration_provenance || jsonb_build_object(
           'timetableBlockCutover', jsonb_build_object('migration', 58)
       ),
       entry.created_by,
       entry.updated_by,
       entry.created_at,
       entry.updated_at
FROM academic_timetable_entries entry
JOIN academic_058_entry_block_map map ON map.entry_id = entry.id
WHERE entry.entry_type IN ('COURSE', 'ACTIVITY');

INSERT INTO academic_timetable_block_group_instructors (
    id, block_group_id, instructor_id, role, display_order, created_at, updated_at
)
SELECT instructor.id,
       instructor.entry_id,
       instructor.instructor_id,
       instructor.role,
       row_number() OVER (
           PARTITION BY instructor.entry_id
           ORDER BY CASE instructor.role WHEN 'primary' THEN 0 ELSE 1 END,
                    instructor.created_at,
                    instructor.id
       )::integer,
       instructor.created_at,
       instructor.created_at
FROM timetable_entry_instructors instructor
JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
WHERE entry.entry_type IN ('COURSE', 'ACTIVITY');

WITH synchronized_homerooms AS (
    SELECT DISTINCT map.block_id,
           coverage.homeroom_id,
           entry.academic_term_id,
           entry.academic_year_id,
           entry.created_by,
           entry.updated_by,
           min(entry.created_at) OVER (PARTITION BY map.block_id, coverage.homeroom_id) AS created_at,
           max(entry.updated_at) OVER (PARTITION BY map.block_id, coverage.homeroom_id) AS updated_at
    FROM academic_timetable_entries entry
    JOIN academic_058_entry_block_map map ON map.entry_id = entry.id
    JOIN activity_offering_details detail
      ON detail.learning_offering_id = entry.learning_offering_id
     AND detail.scheduling_mode = 'synchronized'
    JOIN learning_group_homerooms coverage
      ON coverage.learning_group_id = entry.learning_group_id
    WHERE entry.entry_type = 'ACTIVITY'
), structural_homerooms AS (
    SELECT map.block_id,
           entry.homeroom_id,
           entry.academic_term_id,
           entry.academic_year_id,
           entry.room_id,
           entry.created_by,
           entry.updated_by,
           entry.created_at,
           entry.updated_at
    FROM academic_timetable_entries entry
    JOIN academic_058_entry_block_map map ON map.entry_id = entry.id
    WHERE entry.entry_type IN ('BREAK', 'HOMEROOM', 'ACADEMIC')
      AND entry.homeroom_id IS NOT NULL
)
INSERT INTO academic_timetable_block_homerooms (
    id, block_id, homeroom_id, academic_term_id, academic_year_id,
    target_kind, room_id, migration_provenance,
    created_by, updated_by, created_at, updated_at
)
SELECT uuid_generate_v5(
           'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
           'timetable-block-homeroom:' || source.block_id::text || ':'
               || source.homeroom_id::text
       ),
       source.block_id,
       source.homeroom_id,
       min(source.academic_term_id::text)::uuid,
       min(source.academic_year_id::text)::uuid,
       CASE WHEN bool_or(source.is_reservation) THEN 'reservation' ELSE 'structural' END,
       min(source.room_id::text)::uuid,
       jsonb_build_object('migration', 58),
       (array_agg(source.created_by ORDER BY source.created_at)
           FILTER (WHERE source.created_by IS NOT NULL))[1],
       (array_agg(source.updated_by ORDER BY source.updated_at DESC)
           FILTER (WHERE source.updated_by IS NOT NULL))[1],
       min(source.created_at),
       max(source.updated_at)
FROM (
    SELECT block_id, homeroom_id, academic_term_id, academic_year_id,
           NULL::uuid AS room_id, true AS is_reservation,
           created_by, updated_by, created_at, updated_at
    FROM synchronized_homerooms
    UNION ALL
    SELECT block_id, homeroom_id, academic_term_id, academic_year_id,
           room_id, false, created_by, updated_by, created_at, updated_at
    FROM structural_homerooms
) source
GROUP BY source.block_id, source.homeroom_id;

INSERT INTO academic_timetable_block_teachers (
    id, block_id, teacher_id, academic_term_id, academic_year_id,
    migration_provenance, created_by, updated_by, created_at, updated_at
)
SELECT uuid_generate_v5(
           'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
           'timetable-block-teacher:' || map.block_id::text || ':'
               || instructor.instructor_id::text
       ),
       map.block_id,
       instructor.instructor_id,
       min(entry.academic_term_id::text)::uuid,
       min(entry.academic_year_id::text)::uuid,
       jsonb_build_object('migration', 58),
       (array_agg(entry.created_by ORDER BY entry.created_at)
           FILTER (WHERE entry.created_by IS NOT NULL))[1],
       (array_agg(entry.updated_by ORDER BY entry.updated_at DESC)
           FILTER (WHERE entry.updated_by IS NOT NULL))[1],
       min(entry.created_at),
       max(entry.updated_at)
FROM timetable_entry_instructors instructor
JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
JOIN academic_058_entry_block_map map ON map.entry_id = entry.id
WHERE entry.entry_type IN ('BREAK', 'HOMEROOM', 'ACADEMIC')
GROUP BY map.block_id, instructor.instructor_id;

INSERT INTO academic_timetable_block_group_sync (
    id, block_id, learning_group_id, learning_offering_id,
    academic_term_id, academic_year_id, status, linked_block_group_id,
    attempted_group_row_version, created_by, updated_by, created_at, updated_at
)
SELECT uuid_generate_v5(
           'f291607b-fef7-56f8-a679-ad9d37e3bc75'::uuid,
           'timetable-block-sync:' || group_target.block_id::text || ':'
               || group_target.learning_group_id::text
       ),
       group_target.block_id,
       group_target.learning_group_id,
       group_target.learning_offering_id,
       group_target.academic_term_id,
       group_target.academic_year_id,
       'LINKED',
       group_target.id,
       learning_group.row_version,
       group_target.created_by,
       group_target.updated_by,
       group_target.created_at,
       group_target.updated_at
FROM academic_timetable_block_groups group_target
JOIN academic_timetable_blocks block ON block.id = group_target.block_id
JOIN learning_groups learning_group ON learning_group.id = group_target.learning_group_id
WHERE block.block_kind = 'ACTIVITY'
  AND block.scheduling_mode = 'synchronized';

ALTER TABLE supervision_observations
    DROP CONSTRAINT IF EXISTS supervision_observations_timetable_context_fkey,
    DROP CONSTRAINT IF EXISTS supervision_observations_timetable_entry_id_fkey;

ALTER TABLE supervision_observations
    RENAME COLUMN timetable_entry_id TO timetable_block_group_id;

ALTER TABLE supervision_observations
    ADD CONSTRAINT supervision_observations_timetable_block_group_context_fkey
        FOREIGN KEY (timetable_block_group_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_block_groups(id, academic_term_id, academic_year_id)
        ON DELETE RESTRICT;

DO $$
DECLARE
    before_counts RECORD;
    after_counts RECORD;
    orphan_count BIGINT;
BEGIN
    SELECT * INTO before_counts FROM academic_058_preflight_counts;
    SELECT (SELECT COUNT(*) FROM academic_timetable_block_groups) AS delivery_entry_count,
           (SELECT COUNT(*) FROM academic_timetable_block_group_instructors)
               AS delivery_instructor_count,
           (SELECT COUNT(*) FROM supervision_observations
             WHERE timetable_block_group_id IS NOT NULL) AS observation_reference_count
      INTO after_counts;

    IF after_counts.delivery_entry_count <> before_counts.delivery_entry_count
       OR after_counts.delivery_instructor_count <> before_counts.delivery_instructor_count
       OR after_counts.observation_reference_count <> before_counts.observation_reference_count
    THEN
        RAISE EXCEPTION 'TIMETABLE_BLOCK_RECONCILIATION_COUNT_MISMATCH';
    END IF;

    SELECT COUNT(*)
      INTO orphan_count
      FROM supervision_observations observation
      LEFT JOIN academic_timetable_block_groups block_group
        ON block_group.id = observation.timetable_block_group_id
     WHERE observation.timetable_block_group_id IS NOT NULL
       AND block_group.id IS NULL;

    IF orphan_count <> 0 THEN
        RAISE EXCEPTION 'TIMETABLE_BLOCK_RECONCILIATION_SUPERVISION_ORPHAN';
    END IF;
END;
$$;

DROP TABLE timetable_entry_instructors;
DROP TABLE academic_timetable_entries;

DROP FUNCTION IF EXISTS academic_validate_timetable_slot_conflicts();
DROP FUNCTION IF EXISTS check_entry_move_no_instructor_conflict();
DROP FUNCTION IF EXISTS check_instructor_no_double_book();
DROP FUNCTION IF EXISTS academic_protect_timetable_entry_instructor();

CREATE OR REPLACE FUNCTION assert_timetable_block_mutable(target_block_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM academic_timetable_blocks block
        JOIN academic_timetable_versions version ON version.id = block.timetable_version_id
        WHERE block.id = target_block_id
          AND version.status = 'published'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION academic_protect_timetable_block_child()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    old_block_id UUID;
    new_block_id UUID;
BEGIN
    IF TG_TABLE_NAME = 'academic_timetable_block_group_instructors' THEN
        IF TG_OP <> 'INSERT' THEN
            SELECT block_id INTO old_block_id
            FROM academic_timetable_block_groups
            WHERE id = OLD.block_group_id;
        END IF;
        IF TG_OP <> 'DELETE' THEN
            SELECT block_id INTO new_block_id
            FROM academic_timetable_block_groups
            WHERE id = NEW.block_group_id;
        END IF;
    ELSE
        old_block_id := CASE WHEN TG_OP = 'INSERT' THEN NULL ELSE OLD.block_id END;
        new_block_id := CASE WHEN TG_OP = 'DELETE' THEN NULL ELSE NEW.block_id END;
    END IF;

    IF old_block_id IS NOT NULL THEN
        PERFORM assert_timetable_block_mutable(old_block_id);
    END IF;
    IF new_block_id IS NOT NULL AND new_block_id IS DISTINCT FROM old_block_id THEN
        PERFORM assert_timetable_block_mutable(new_block_id);
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE OR REPLACE FUNCTION assert_timetable_block_conflict_free(target_block_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_block academic_timetable_blocks%ROWTYPE;
    slot_lock_key BIGINT;
BEGIN
    SELECT * INTO target_block
    FROM academic_timetable_blocks
    WHERE id = target_block_id;

    IF target_block.id IS NULL OR NOT target_block.is_active THEN
        RETURN;
    END IF;

    slot_lock_key := hashtextextended(
        target_block.timetable_version_id::text || ':' || target_block.day_of_week || ':'
            || target_block.bell_schedule_period_id::text,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_block_groups candidate
        JOIN academic_timetable_block_groups occupied
          ON occupied.learning_group_id = candidate.learning_group_id
         AND occupied.block_id <> candidate.block_id
         AND occupied.is_active
        JOIN academic_timetable_blocks occupied_block ON occupied_block.id = occupied.block_id
        WHERE candidate.block_id = target_block_id
          AND candidate.is_active
          AND occupied_block.is_active
          AND occupied_block.timetable_version_id = target_block.timetable_version_id
          AND occupied_block.day_of_week = target_block.day_of_week
          AND occupied_block.bell_schedule_period_id = target_block.bell_schedule_period_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_GROUP_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_homerooms AS (
            SELECT homeroom_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT coverage.homeroom_id
            FROM academic_timetable_block_groups block_group
            JOIN learning_group_homerooms coverage
              ON coverage.learning_group_id = block_group.learning_group_id
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_homerooms AS (
            SELECT homeroom.homeroom_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT coverage.homeroom_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            JOIN learning_group_homerooms coverage
              ON coverage.learning_group_id = block_group.learning_group_id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_homerooms candidate
        JOIN occupied_homerooms occupied USING (homeroom_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_teachers AS (
            SELECT teacher_id
            FROM academic_timetable_block_teachers
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT instructor.instructor_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_teachers AS (
            SELECT teacher.teacher_id, teacher.block_id
            FROM academic_timetable_block_teachers teacher
            JOIN academic_timetable_blocks block ON block.id = teacher.block_id
            WHERE teacher.block_id <> target_block_id
              AND teacher.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT instructor.instructor_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_teachers candidate
        JOIN occupied_teachers occupied USING (teacher_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_rooms AS (
            SELECT room_id
            FROM academic_timetable_block_groups
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
            UNION
            SELECT room_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
        ), occupied_rooms AS (
            SELECT block_group.room_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block_group.room_id IS NOT NULL
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT homeroom.room_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND homeroom.room_id IS NOT NULL
              AND block.timetable_version_id = target_block.timetable_version_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_rooms candidate
        JOIN occupied_rooms occupied USING (room_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_ROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION academic_validate_timetable_block_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_block_id UUID;
BEGIN
    affected_block_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    IF TG_OP <> 'DELETE' THEN
        PERFORM assert_timetable_block_conflict_free(affected_block_id);
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE OR REPLACE FUNCTION academic_validate_timetable_block_child_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    affected_block_id UUID;
BEGIN
    IF TG_TABLE_NAME = 'academic_timetable_block_group_instructors' THEN
        SELECT block_id INTO affected_block_id
        FROM academic_timetable_block_groups
        WHERE id = CASE WHEN TG_OP = 'DELETE' THEN OLD.block_group_id ELSE NEW.block_group_id END;
    ELSE
        affected_block_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.block_id ELSE NEW.block_id END;
    END IF;

    IF TG_OP <> 'DELETE' AND affected_block_id IS NOT NULL THEN
        PERFORM assert_timetable_block_conflict_free(affected_block_id);
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER academic_timetable_blocks_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_blocks
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_version_child();

CREATE TRIGGER academic_timetable_blocks_conflict_guard
AFTER INSERT OR UPDATE OF timetable_version_id, day_of_week,
    bell_schedule_period_id, is_active
ON academic_timetable_blocks
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_block_change();

CREATE TRIGGER academic_timetable_block_groups_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_block_groups
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_block_child();
CREATE TRIGGER academic_timetable_block_groups_conflict_guard
AFTER INSERT OR UPDATE OF block_id, learning_group_id, room_id, is_active
ON academic_timetable_block_groups
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_block_child_change();

CREATE TRIGGER academic_timetable_block_group_instructors_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_block_group_instructors
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_block_child();
CREATE TRIGGER academic_timetable_block_group_instructors_conflict_guard
AFTER INSERT OR UPDATE OF block_group_id, instructor_id
ON academic_timetable_block_group_instructors
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_block_child_change();

CREATE TRIGGER academic_timetable_block_homerooms_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_block_homerooms
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_block_child();
CREATE TRIGGER academic_timetable_block_homerooms_conflict_guard
AFTER INSERT OR UPDATE OF block_id, homeroom_id, room_id, is_active
ON academic_timetable_block_homerooms
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_block_child_change();

CREATE TRIGGER academic_timetable_block_teachers_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_block_teachers
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_block_child();
CREATE TRIGGER academic_timetable_block_teachers_conflict_guard
AFTER INSERT OR UPDATE OF block_id, teacher_id, is_active
ON academic_timetable_block_teachers
FOR EACH ROW EXECUTE FUNCTION academic_validate_timetable_block_child_change();

CREATE TRIGGER academic_timetable_block_group_sync_version_immutable
BEFORE INSERT OR UPDATE OR DELETE ON academic_timetable_block_group_sync
FOR EACH ROW EXECUTE FUNCTION academic_protect_timetable_block_child();

-- Timetable scheduling owns an explicit permission boundary. Existing Delivery grants are
-- copied once so this cutover does not silently remove access, while future grants can diverge.
INSERT INTO permissions (code, name, module, action, scope, description, is_active)
VALUES
    ('academic_timetable.read.assigned', 'ดูตารางสอนที่รับผิดชอบ', 'academic_timetable', 'read', 'assigned', 'Read assigned timetable resources', true),
    ('academic_timetable.read.organization_unit', 'ดูตารางสอนของหน่วยงาน', 'academic_timetable', 'read', 'organization_unit', 'Read timetable resources owned by the exact organization unit', true),
    ('academic_timetable.read.organization_tree', 'ดูตารางสอนในสายงาน', 'academic_timetable', 'read', 'organization_tree', 'Read timetable resources in the organization tree', true),
    ('academic_timetable.read.school', 'ดูตารางสอนทั้งโรงเรียน', 'academic_timetable', 'read', 'school', 'Read all school timetable resources', true),
    ('academic_timetable.manage.assigned', 'จัดตารางสอนที่รับผิดชอบ', 'academic_timetable', 'manage', 'assigned', 'Manage assigned timetable resources', true),
    ('academic_timetable.manage.organization_unit', 'จัดตารางสอนของหน่วยงาน', 'academic_timetable', 'manage', 'organization_unit', 'Manage timetable resources owned by the exact organization unit', true),
    ('academic_timetable.manage.organization_tree', 'จัดตารางสอนในสายงาน', 'academic_timetable', 'manage', 'organization_tree', 'Manage timetable resources in the organization tree', true),
    ('academic_timetable.manage.school', 'จัดตารางสอนทั้งโรงเรียน', 'academic_timetable', 'manage', 'school', 'Manage all school timetable resources', true),
    ('academic_timetable.publish.school', 'เผยแพร่ตารางสอน', 'academic_timetable', 'publish', 'school', 'Publish a school timetable version', true)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    is_active = true,
    updated_at = now();

CREATE TEMP TABLE academic_058_permission_map (
    source_code TEXT NOT NULL,
    target_code TEXT NOT NULL,
    PRIMARY KEY (source_code, target_code)
) ON COMMIT DROP;

INSERT INTO academic_058_permission_map (source_code, target_code)
VALUES
    ('learning_offering.read.assigned', 'academic_timetable.read.assigned'),
    ('learning_offering.read.organization_unit', 'academic_timetable.read.organization_unit'),
    ('learning_offering.read.organization_tree', 'academic_timetable.read.organization_tree'),
    ('learning_offering.read.school', 'academic_timetable.read.school'),
    ('learning_offering.manage.assigned', 'academic_timetable.manage.assigned'),
    ('learning_offering.manage.organization_unit', 'academic_timetable.manage.organization_unit'),
    ('learning_offering.manage.organization_tree', 'academic_timetable.manage.organization_tree'),
    ('learning_offering.manage.school', 'academic_timetable.manage.school'),
    ('learning_offering.manage.school', 'academic_timetable.publish.school');

INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT source_grant.role_id, target.id, source_grant.created_at
FROM role_permissions source_grant
JOIN permissions source ON source.id = source_grant.permission_id
JOIN academic_058_permission_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_grants (
    organization_unit_id, permission_id, created_at, created_by, position_code
)
SELECT source_grant.organization_unit_id, target.id, source_grant.created_at,
       source_grant.created_by, source_grant.position_code
FROM organization_permission_grants source_grant
JOIN permissions source ON source.id = source_grant.permission_id
JOIN academic_058_permission_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_delegations (
    id, from_user_id, to_user_id, permission_id, organization_unit_id,
    reason, started_at, expires_at, revoked_at, created_at
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'timetable-permission-delegation:' || source_grant.id::text || ':' || target.code
       ),
       source_grant.from_user_id,
       source_grant.to_user_id,
       target.id,
       source_grant.organization_unit_id,
       source_grant.reason,
       source_grant.started_at,
       source_grant.expires_at,
       source_grant.revoked_at,
       source_grant.created_at
FROM organization_permission_delegations source_grant
JOIN permissions source ON source.id = source_grant.permission_id
JOIN academic_058_permission_map mapping ON mapping.source_code = source.code
JOIN permissions target ON target.code = mapping.target_code
ON CONFLICT DO NOTHING;

COMMENT ON TABLE academic_timetable_blocks IS
    'Canonical one-period timetable event; child targets participate in the same simultaneous block.';
COMMENT ON TABLE academic_timetable_block_group_sync IS
    'Per synchronized activity group linkage state. EXCLUDED remains sticky until explicitly restored.';
