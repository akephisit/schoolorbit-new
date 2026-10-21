-- Canonical custom plan order. Existing rows keep legacy zero ties without backfill.
LOCK TABLE curriculum_editions, curriculum_levels, study_programs,
    curriculum_term_slots, curriculum_course_requirements, curriculum_activity_requirements,
    curriculum_publications, curriculum_publication_levels, curriculum_publication_programs,
    curriculum_publication_slots, curriculum_publication_courses, curriculum_publication_activities,
    curriculum_requirement_sources, homerooms, student_academic_years, admission_tracks,
    course_offering_details, activity_offering_details IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE curriculum_134_reconciliation (
    table_name text PRIMARY KEY, row_count bigint NOT NULL,
    identity_checksum text NOT NULL, content_checksum text NOT NULL
) ON COMMIT DROP;

-- Reconcile each live and historical graph independently: an open amendment may
-- intentionally differ from its current publication. Only the new order field is omitted.
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
        IF missing_identity<>0 THEN RAISE EXCEPTION 'CURRICULUM_134_IDENTITY_UNSUPPORTED'; END IF;
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text, COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5((CASE WHEN $1 IN ('study_programs','curriculum_publication_programs','published_study_programs') THEN to_jsonb(r)-'display_order' ELSE to_jsonb(r) END)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, selected_table) INTO evidence USING selected_table;
        INSERT INTO curriculum_134_reconciliation VALUES
            (selected_table,evidence.row_count,evidence.identity_checksum,evidence.content_checksum);
    END LOOP;
END $$;

ALTER TABLE study_programs ADD COLUMN display_order integer NOT NULL DEFAULT 0 CHECK(display_order>=0);
ALTER TABLE curriculum_publication_programs ADD COLUMN display_order integer NOT NULL DEFAULT 0 CHECK(display_order>=0);

CREATE OR REPLACE FUNCTION capture_curriculum_publication(selected_edition uuid,actor uuid,note text,baseline boolean DEFAULT false) RETURNS uuid LANGUAGE plpgsql AS $$
DECLARE edition curriculum_editions; publication uuid:=gen_random_uuid();
BEGIN
 SELECT * INTO STRICT edition FROM curriculum_editions WHERE id=selected_edition FOR UPDATE;
 IF NOT baseline AND edition.draft_id IS NULL THEN RAISE EXCEPTION 'CURRICULUM_DRAFT_REQUIRED'; END IF;
 IF baseline AND (edition.publication_count<>0 OR edition.status<>'published') THEN RAISE EXCEPTION 'CURRICULUM_INVALID_BASELINE'; END IF;
 INSERT INTO curriculum_publications(id,edition_id,publication_no,previous_publication_id,name,revision_year,description,published_by,published_at,change_note,is_baseline,level_count,program_count,slot_count,course_count,activity_count)
 SELECT publication,edition.id,edition.publication_count+1,edition.current_publication_id,edition.name,edition.revision_year,edition.description,actor,
 CASE WHEN baseline THEN edition.published_at ELSE now() END,note,baseline,
 (SELECT count(*) FROM curriculum_levels WHERE edition_id=edition.id),
 (SELECT count(*) FROM study_programs WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_term_slots WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_course_requirements WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id)),
 (SELECT count(*) FROM curriculum_activity_requirements WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id));
 INSERT INTO curriculum_publication_levels SELECT r.*,publication FROM curriculum_levels r WHERE edition_id=edition.id;
 INSERT INTO curriculum_publication_programs(id,curriculum_level_id,code,name_th,name_en,status,row_version,created_at,updated_at,migration_provenance,display_order,publication_id) SELECT r.id,r.curriculum_level_id,r.code,r.name_th,r.name_en,r.status,r.row_version,r.created_at,r.updated_at,r.migration_provenance,r.display_order,publication FROM study_programs r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_slots SELECT r.*,publication FROM curriculum_term_slots r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_courses SELECT r.*,publication FROM curriculum_course_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 INSERT INTO curriculum_publication_activities SELECT r.*,publication FROM curriculum_activity_requirements r WHERE curriculum_level_id IN(SELECT id FROM curriculum_levels WHERE edition_id=edition.id);
 UPDATE curriculum_editions SET current_publication_id=publication,publication_count=publication_count+1,draft_id=NULL,status='published',
  published_at=CASE WHEN baseline THEN edition.published_at ELSE now() END,
  row_version=row_version+CASE WHEN baseline THEN 0 ELSE 1 END,
  updated_at=CASE WHEN baseline THEN edition.updated_at ELSE now() END WHERE id=edition.id;
 RETURN publication;
END $$;

-- Append the new view column without dropping the view or its existing grants.
CREATE OR REPLACE VIEW published_study_programs AS
    SELECT r.* FROM curriculum_publication_programs r
    JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;

DO $$
DECLARE previous record; actual record; workspace_columns text[]; snapshot_columns text[];
BEGIN
    FOR previous IN SELECT * FROM curriculum_134_reconciliation ORDER BY table_name LOOP
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5((CASE WHEN $1 IN ('study_programs','curriculum_publication_programs','published_study_programs') THEN to_jsonb(r)-'display_order' ELSE to_jsonb(r) END)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, previous.table_name) INTO actual USING previous.table_name;
        IF previous.row_count IS DISTINCT FROM actual.row_count
            OR previous.identity_checksum IS DISTINCT FROM actual.identity_checksum
            OR previous.content_checksum IS DISTINCT FROM actual.content_checksum THEN
            RAISE EXCEPTION 'CURRICULUM_134_PRESERVATION_FAILED';
        END IF;
    END LOOP;
    IF EXISTS(SELECT 1 FROM study_programs WHERE display_order<>0)
       OR EXISTS(SELECT 1 FROM curriculum_publication_programs WHERE display_order<>0) THEN
        RAISE EXCEPTION 'CURRICULUM_134_LEGACY_ORDER_CHANGED';
    END IF;
    SELECT array_agg(attname::text ORDER BY attnum) INTO workspace_columns
    FROM pg_attribute WHERE attrelid='study_programs'::regclass AND attnum>0 AND NOT attisdropped;
    SELECT array_agg(attname::text ORDER BY attnum) INTO snapshot_columns
    FROM pg_attribute WHERE attrelid='curriculum_publication_programs'::regclass
        AND attnum>0 AND NOT attisdropped AND attname<>'publication_id';
    IF workspace_columns IS DISTINCT FROM snapshot_columns THEN
        RAISE EXCEPTION 'CURRICULUM_134_CAPTURE_COLUMN_MISMATCH';
    END IF;
END $$;

INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'curriculum.program_order_added.reconciled','curriculum',NULL,
    jsonb_build_object('migration',134,'passed',true,'tables',
        (SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,
            'identityChecksum',identity_checksum,'contentChecksum',content_checksum))
         FROM curriculum_134_reconciliation));
