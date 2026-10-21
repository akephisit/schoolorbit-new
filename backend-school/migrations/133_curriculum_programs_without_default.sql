-- Matched-release contraction: maintenance must remain active until all consumers
-- use the default-free contract. The migration runner owns the atomic transaction.
LOCK TABLE curriculum_editions, curriculum_levels, study_programs,
    curriculum_term_slots, curriculum_course_requirements, curriculum_activity_requirements,
    curriculum_publications, curriculum_publication_levels, curriculum_publication_programs,
    curriculum_publication_slots, curriculum_publication_courses, curriculum_publication_activities,
    curriculum_requirement_sources, homerooms, student_academic_years, admission_tracks,
    course_offering_details, activity_offering_details IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE curriculum_133_reconciliation (
    table_name text PRIMARY KEY, row_count bigint NOT NULL,
    identity_checksum text NOT NULL, content_checksum text NOT NULL
) ON COMMIT DROP;

-- Reconcile each live and historical graph independently: an open amendment may
-- intentionally differ from its current publication. Only the retired flag is omitted.
DO $$
DECLARE selected_table text; evidence record; missing_identity bigint;
BEGIN
    FOREACH selected_table IN ARRAY ARRAY[
        'curriculum_editions','curriculum_levels','study_programs','curriculum_term_slots',
        'curriculum_course_requirements','curriculum_activity_requirements',
        'curriculum_publications','curriculum_publication_levels','curriculum_publication_programs',
        'curriculum_publication_slots','curriculum_publication_courses','curriculum_publication_activities',
        'curriculum_requirement_sources','homerooms','student_academic_years','admission_tracks',
        'course_offering_details','activity_offering_details','published_study_programs'
    ] LOOP
        EXECUTE format('SELECT count(*) FROM %I r WHERE COALESCE(to_jsonb(r)->>''id'',to_jsonb(r)->>''learning_offering_id'') IS NULL', selected_table) INTO missing_identity;
        IF missing_identity<>0 THEN RAISE EXCEPTION 'CURRICULUM_133_IDENTITY_UNSUPPORTED'; END IF;
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text, COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5((CASE WHEN $1 IN ('study_programs','curriculum_publication_programs','published_study_programs') THEN to_jsonb(r)-'is_default' ELSE to_jsonb(r) END)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, selected_table) INTO evidence USING selected_table;
        INSERT INTO curriculum_133_reconciliation VALUES
            (selected_table,evidence.row_count,evidence.identity_checksum,evidence.content_checksum);
    END LOOP;
END $$;

-- Both positional capture sources retain exactly the same surviving column order.
-- RESTRICT refuses any unexpected dependency instead of deleting it implicitly.
DROP VIEW published_study_programs RESTRICT;
DROP INDEX study_programs_one_default_per_version RESTRICT;
ALTER TABLE study_programs DROP COLUMN is_default RESTRICT;
ALTER TABLE curriculum_publication_programs DROP COLUMN is_default RESTRICT;
CREATE VIEW published_study_programs AS
    SELECT r.* FROM curriculum_publication_programs r
    JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;

DO $$
DECLARE previous record; actual record; workspace_columns text[]; snapshot_columns text[];
BEGIN
    FOR previous IN SELECT * FROM curriculum_133_reconciliation ORDER BY table_name LOOP
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5(to_jsonb(r)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, previous.table_name) INTO actual;
        IF previous.row_count IS DISTINCT FROM actual.row_count
            OR previous.identity_checksum IS DISTINCT FROM actual.identity_checksum
            OR previous.content_checksum IS DISTINCT FROM actual.content_checksum THEN
            RAISE EXCEPTION 'CURRICULUM_133_PRESERVATION_FAILED';
        END IF;
    END LOOP;
    SELECT array_agg(attname::text ORDER BY attnum) INTO workspace_columns
    FROM pg_attribute WHERE attrelid='study_programs'::regclass AND attnum>0 AND NOT attisdropped;
    SELECT array_agg(attname::text ORDER BY attnum) INTO snapshot_columns
    FROM pg_attribute WHERE attrelid='curriculum_publication_programs'::regclass
        AND attnum>0 AND NOT attisdropped AND attname<>'publication_id';
    IF workspace_columns IS DISTINCT FROM snapshot_columns THEN
        RAISE EXCEPTION 'CURRICULUM_133_CAPTURE_COLUMN_MISMATCH';
    END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'curriculum.program_default_removed.reconciled','curriculum',NULL,
    jsonb_build_object('migration',133,'passed',true,'tables',
        (SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,
            'identityChecksum',identity_checksum,'contentChecksum',content_checksum))
         FROM curriculum_133_reconciliation));
