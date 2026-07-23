\set ON_ERROR_STOP on
\set target_semester '00000000-0000-0000-0000-000000000003'
\pset format unaligned
\pset tuples_only on
\pset fieldsep '|'

BEGIN;

SET LOCAL statement_timeout = '120s';
SET LOCAL lock_timeout = '5s';
SELECT set_config('schoolorbit.target_semester', :'target_semester', true);

CREATE TEMP TABLE timetable_plan_fixture (
    id uuid PRIMARY KEY,
    academic_semester_id uuid NOT NULL,
    day_of_week varchar(3) NOT NULL,
    period_id uuid NOT NULL,
    is_active boolean NOT NULL
) ON COMMIT DROP;

INSERT INTO timetable_plan_fixture (
    id,
    academic_semester_id,
    day_of_week,
    period_id,
    is_active
)
SELECT
    md5('timetable-entry-' || row_number)::uuid,
    (
        '00000000-0000-0000-0000-'
        || lpad(((((row_number - 1) % 5) + 1))::text, 12, '0')
    )::uuid,
    (ARRAY['MON', 'TUE', 'WED', 'THU', 'FRI'])[
        (((row_number - 1) / 5) % 5) + 1
    ],
    (
        '10000000-0000-0000-0000-'
        || lpad(((((row_number - 1) / 25) % 10) + 1)::text, 12, '0')
    )::uuid,
    ((row_number - 1) % 25) >= 5
FROM generate_series(1, 100000) AS fixture(row_number);

-- These are the two relevant existing access paths from 001_baseline.sql.
CREATE INDEX timetable_plan_fixture_semester_idx
    ON timetable_plan_fixture (academic_semester_id);
CREATE INDEX timetable_plan_fixture_day_period_idx
    ON timetable_plan_fixture (day_of_week, period_id);

ANALYZE timetable_plan_fixture;

CREATE TEMP TABLE timetable_plan_results (
    phase text PRIMARY KEY,
    plan_json jsonb NOT NULL,
    total_index_bytes bigint NOT NULL,
    candidate_index_bytes bigint NOT NULL
) ON COMMIT DROP;

DO $benchmark$
DECLARE
    warm_plan jsonb;
    measured_plan jsonb;
BEGIN
    -- Warm each access path once before recording so the candidate does not win
    -- merely because the baseline query populated shared buffers.
    EXECUTE $query$
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        SELECT id, academic_semester_id, day_of_week, period_id
        FROM timetable_plan_fixture
        WHERE academic_semester_id =
                  current_setting('schoolorbit.target_semester')::uuid
          AND is_active = true
        ORDER BY day_of_week, period_id
    $query$ INTO warm_plan;

    EXECUTE $query$
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        SELECT id, academic_semester_id, day_of_week, period_id
        FROM timetable_plan_fixture
        WHERE academic_semester_id =
                  current_setting('schoolorbit.target_semester')::uuid
          AND is_active = true
        ORDER BY day_of_week, period_id
    $query$ INTO measured_plan;

    INSERT INTO timetable_plan_results (
        phase,
        plan_json,
        total_index_bytes,
        candidate_index_bytes
    )
    VALUES (
        'before',
        measured_plan,
        pg_indexes_size('timetable_plan_fixture'),
        0
    );
END
$benchmark$;

CREATE INDEX timetable_plan_fixture_active_semester_slot_idx
    ON timetable_plan_fixture (academic_semester_id, day_of_week, period_id)
    WHERE is_active = true;

ANALYZE timetable_plan_fixture;

DO $benchmark$
DECLARE
    warm_plan jsonb;
    measured_plan jsonb;
BEGIN
    EXECUTE $query$
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        SELECT id, academic_semester_id, day_of_week, period_id
        FROM timetable_plan_fixture
        WHERE academic_semester_id =
                  current_setting('schoolorbit.target_semester')::uuid
          AND is_active = true
        ORDER BY day_of_week, period_id
    $query$ INTO warm_plan;

    EXECUTE $query$
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        SELECT id, academic_semester_id, day_of_week, period_id
        FROM timetable_plan_fixture
        WHERE academic_semester_id =
                  current_setting('schoolorbit.target_semester')::uuid
          AND is_active = true
        ORDER BY day_of_week, period_id
    $query$ INTO measured_plan;

    INSERT INTO timetable_plan_results (
        phase,
        plan_json,
        total_index_bytes,
        candidate_index_bytes
    )
    VALUES (
        'after',
        measured_plan,
        pg_indexes_size('timetable_plan_fixture'),
        pg_relation_size('timetable_plan_fixture_active_semester_slot_idx')
    );
END
$benchmark$;

SELECT
    'FIXTURE',
    count(*),
    count(*) FILTER (WHERE is_active),
    count(*) FILTER (
        WHERE academic_semester_id = :'target_semester'::uuid
          AND is_active
    )
FROM timetable_plan_fixture;

WITH RECURSIVE plan_nodes AS (
    SELECT
        result.phase,
        result.plan_json,
        result.total_index_bytes,
        result.candidate_index_bytes,
        result.plan_json -> 0 -> 'Plan' AS node
    FROM timetable_plan_results result

    UNION ALL

    SELECT
        parent.phase,
        parent.plan_json,
        parent.total_index_bytes,
        parent.candidate_index_bytes,
        child.node
    FROM plan_nodes parent
    CROSS JOIN LATERAL jsonb_array_elements(
        COALESCE(parent.node -> 'Plans', '[]'::jsonb)
    ) AS child(node)
),
summaries AS (
    SELECT
        phase,
        max((plan_json -> 0 ->> 'Execution Time')::numeric) AS execution_ms,
        max((plan_json -> 0 -> 'Plan' ->> 'Actual Rows')::numeric)::bigint AS actual_rows,
        max((plan_json -> 0 -> 'Plan' ->> 'Shared Hit Blocks')::bigint) AS shared_hits,
        max((plan_json -> 0 -> 'Plan' ->> 'Shared Read Blocks')::bigint) AS shared_reads,
        max((plan_json -> 0 -> 'Plan' ->> 'Local Hit Blocks')::bigint) AS local_hits,
        max((plan_json -> 0 -> 'Plan' ->> 'Local Read Blocks')::bigint) AS local_reads,
        max(total_index_bytes) AS total_index_bytes,
        max(candidate_index_bytes) AS candidate_index_bytes,
        string_agg(DISTINCT node ->> 'Node Type', ',' ORDER BY node ->> 'Node Type')
            AS node_types,
        string_agg(DISTINCT node ->> 'Index Name', ',' ORDER BY node ->> 'Index Name')
            FILTER (WHERE node ? 'Index Name') AS index_names
    FROM plan_nodes
    GROUP BY phase
)
SELECT
    'RESULT',
    phase,
    actual_rows,
    execution_ms,
    shared_hits,
    shared_reads,
    local_hits,
    local_reads,
    total_index_bytes,
    candidate_index_bytes,
    node_types,
    COALESCE(index_names, '')
FROM summaries
ORDER BY CASE phase WHEN 'before' THEN 1 ELSE 2 END;

SELECT 'PLAN_JSON', phase, plan_json::text
FROM timetable_plan_results
ORDER BY CASE phase WHEN 'before' THEN 1 ELSE 2 END;

ROLLBACK;
