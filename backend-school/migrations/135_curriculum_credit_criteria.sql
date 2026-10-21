-- Level-owned basic-credit settings; legacy levels remain explicitly unconfigured.
LOCK TABLE curriculum_editions, curriculum_levels, study_programs,
    curriculum_term_slots, curriculum_course_requirements, curriculum_activity_requirements,
    curriculum_publications, curriculum_publication_levels, curriculum_publication_programs,
    curriculum_publication_slots, curriculum_publication_courses, curriculum_publication_activities,
    curriculum_requirement_sources, homerooms, student_academic_years, admission_tracks,
    course_offering_details, activity_offering_details IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE curriculum_135_reconciliation (
    table_name text PRIMARY KEY, row_count bigint NOT NULL,
    identity_checksum text NOT NULL, content_checksum text NOT NULL
) ON COMMIT DROP;

-- Reconcile each live and historical graph independently: an open amendment may
-- intentionally differ from its current publication. Only the new criteria fields are omitted.
DO $$
DECLARE selected_table text; evidence record; missing_identity bigint;
BEGIN
    FOREACH selected_table IN ARRAY ARRAY[
        'curriculum_editions','curriculum_levels','study_programs','curriculum_term_slots',
        'curriculum_course_requirements','curriculum_activity_requirements',
        'curriculum_publications','curriculum_publication_levels','curriculum_publication_programs',
        'curriculum_publication_slots','curriculum_publication_courses','curriculum_publication_activities',
        'curriculum_requirement_sources','homerooms','student_academic_years','admission_tracks',
        'course_offering_details','activity_offering_details','published_study_programs','published_curriculum_levels'
    ] LOOP
        EXECUTE format('SELECT count(*) FROM %I r WHERE COALESCE(to_jsonb(r)->>''id'',to_jsonb(r)->>''learning_offering_id'') IS NULL', selected_table) INTO missing_identity;
        IF missing_identity<>0 THEN RAISE EXCEPTION 'CURRICULUM_135_IDENTITY_UNSUPPORTED'; END IF;
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text, COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5((CASE WHEN $1 IN ('curriculum_levels','curriculum_publication_levels','published_curriculum_levels') THEN to_jsonb(r)-'credit_criteria'-'credit_criteria_row_version' ELSE to_jsonb(r) END)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, selected_table) INTO evidence USING selected_table;
        INSERT INTO curriculum_135_reconciliation VALUES
            (selected_table,evidence.row_count,evidence.identity_checksum,evidence.content_checksum);
    END LOOP;
END $$;

CREATE FUNCTION curriculum_credit_criteria_valid(criteria jsonb) RETURNS boolean
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE rule jsonb; grade jsonb; scopes text[] := ARRAY[]::text[]; keys text[];
BEGIN
    IF jsonb_typeof(criteria) IS DISTINCT FROM 'array' THEN RETURN false; END IF;
    IF jsonb_array_length(criteria)>2 THEN RETURN false; END IF;
    FOR rule IN SELECT value FROM jsonb_array_elements(criteria) LOOP
        IF jsonb_typeof(rule) IS DISTINCT FROM 'object' THEN RETURN false; END IF;
        SELECT array_agg(key ORDER BY key) INTO keys FROM jsonb_object_keys(rule) AS key;
        IF keys IS DISTINCT FROM ARRAY['comparison','enabled','gradeLevelIds','scope','targetCredits'] THEN RETURN false; END IF;
        IF rule->>'scope' NOT IN ('annual','level') OR rule->>'comparison' NOT IN ('equal','minimum')
           OR jsonb_typeof(rule->'scope') IS DISTINCT FROM 'string'
           OR jsonb_typeof(rule->'comparison') IS DISTINCT FROM 'string'
           OR jsonb_typeof(rule->'enabled') IS DISTINCT FROM 'boolean'
           OR jsonb_typeof(rule->'targetCredits') IS DISTINCT FROM 'string'
           OR (rule->>'targetCredits') !~ '^(0|[1-9][0-9]{0,3})([.][0-9]{1,2})?$'
           OR jsonb_typeof(rule->'gradeLevelIds') IS DISTINCT FROM 'array' THEN RETURN false; END IF;
        IF rule->>'scope'=ANY(scopes) THEN RETURN false; END IF;
        scopes := array_append(scopes,rule->>'scope');
        IF jsonb_array_length(rule->'gradeLevelIds') NOT BETWEEN 1 AND 500 THEN RETURN false; END IF;
        FOR grade IN SELECT value FROM jsonb_array_elements(rule->'gradeLevelIds') LOOP
            IF jsonb_typeof(grade) IS DISTINCT FROM 'string' OR (grade#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' THEN RETURN false; END IF;
        END LOOP;
        IF (SELECT count(DISTINCT value) FROM jsonb_array_elements(rule->'gradeLevelIds')) <> jsonb_array_length(rule->'gradeLevelIds') THEN RETURN false; END IF;
    END LOOP;
    RETURN true;
END $$;
ALTER TABLE curriculum_levels
    ADD COLUMN credit_criteria jsonb NOT NULL DEFAULT '[]'::jsonb CHECK(curriculum_credit_criteria_valid(credit_criteria)),
    ADD COLUMN credit_criteria_row_version bigint NOT NULL DEFAULT 1 CHECK(credit_criteria_row_version>0);
ALTER TABLE curriculum_publication_levels
    ADD COLUMN credit_criteria jsonb NOT NULL DEFAULT '[]'::jsonb CHECK(curriculum_credit_criteria_valid(credit_criteria)),
    ADD COLUMN credit_criteria_row_version bigint NOT NULL DEFAULT 1 CHECK(credit_criteria_row_version>0);

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
 INSERT INTO curriculum_publication_levels(id,description,created_at,updated_at,row_version,migration_provenance,edition_id,code,name_th,name_en,grade_level_ids,is_active,credit_criteria,credit_criteria_row_version,publication_id) SELECT r.id,r.description,r.created_at,r.updated_at,r.row_version,r.migration_provenance,r.edition_id,r.code,r.name_th,r.name_en,r.grade_level_ids,r.is_active,r.credit_criteria,r.credit_criteria_row_version,publication FROM curriculum_levels r WHERE edition_id=edition.id;
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

-- Append fields while preserving view identity and grants.
CREATE OR REPLACE VIEW published_curriculum_levels AS
    SELECT r.* FROM curriculum_publication_levels r JOIN curriculum_editions e ON e.current_publication_id=r.publication_id;

CREATE OR REPLACE FUNCTION curriculum_workspace_matches_publication(selected_edition uuid,selected_publication uuid)
RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT selected_publication IS NOT NULL AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at'-'credit_criteria_row_version' ORDER BY id) FROM curriculum_levels r WHERE edition_id=selected_edition)
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at'-'credit_criteria_row_version' ORDER BY id) FROM curriculum_publication_levels r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM study_programs r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_programs r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_term_slots r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_slots r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_course_requirements r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_courses r WHERE publication_id=selected_publication)
 AND
 (SELECT jsonb_agg(to_jsonb(r)-'row_version'-'updated_at' ORDER BY id) FROM curriculum_activity_requirements r WHERE curriculum_level_id IN (SELECT id FROM curriculum_levels WHERE edition_id=selected_edition))
 IS NOT DISTINCT FROM
 (SELECT jsonb_agg(to_jsonb(r)-'publication_id'-'row_version'-'updated_at' ORDER BY id) FROM curriculum_publication_activities r WHERE publication_id=selected_publication);
$$;

DO $$
DECLARE previous record; actual record; workspace_columns text[]; snapshot_columns text[];
BEGIN
    FOR previous IN SELECT * FROM curriculum_135_reconciliation ORDER BY table_name LOOP
        EXECUTE format($query$
            SELECT count(*) AS row_count,
                   md5(COALESCE(string_agg(md5(COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text || COALESCE((to_jsonb(r)->'publication_id')::text,'')),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS identity_checksum,
                   md5(COALESCE(string_agg(md5((CASE WHEN $1 IN ('curriculum_levels','curriculum_publication_levels','published_curriculum_levels') THEN to_jsonb(r)-'credit_criteria'-'credit_criteria_row_version' ELSE to_jsonb(r) END)::text),'' ORDER BY (to_jsonb(r)->'publication_id')::text,COALESCE(to_jsonb(r)->'id',to_jsonb(r)->'learning_offering_id')::text),'')) AS content_checksum
            FROM %I r
        $query$, previous.table_name) INTO actual USING previous.table_name;
        IF previous.row_count IS DISTINCT FROM actual.row_count
            OR previous.identity_checksum IS DISTINCT FROM actual.identity_checksum
            OR previous.content_checksum IS DISTINCT FROM actual.content_checksum THEN
            RAISE EXCEPTION 'CURRICULUM_135_PRESERVATION_FAILED';
        END IF;
    END LOOP;
    IF EXISTS(SELECT 1 FROM curriculum_levels WHERE credit_criteria<>'[]'::jsonb OR credit_criteria_row_version<>1)
       OR EXISTS(SELECT 1 FROM curriculum_publication_levels WHERE credit_criteria<>'[]'::jsonb OR credit_criteria_row_version<>1) THEN
        RAISE EXCEPTION 'CURRICULUM_135_LEGACY_CRITERIA_CHANGED';
    END IF;
END $$;
INSERT INTO academic_audit_events(event_code,entity_type,entity_id,payload)
SELECT 'curriculum.credit_criteria_added.reconciled','curriculum',NULL,
    jsonb_build_object('migration',135,'passed',true,'tables',
        (SELECT jsonb_object_agg(table_name,jsonb_build_object('count',row_count,
            'identityChecksum',identity_checksum,'contentChecksum',content_checksum))
         FROM curriculum_135_reconciliation));
