-- A bounded Results-owned operation for explicitly authorized offering deletion.
-- Application authorization precedes this operation on the same transaction.
-- No session setting can authorize an immutable-row mutation.
CREATE TABLE academic_result_removal_scope (
    transaction_id xid8 NOT NULL,
    backend_pid integer NOT NULL,
    relation_name text NOT NULL,
    operation text NOT NULL CHECK(operation IN ('DELETE','UPDATE')),
    before_hash text NOT NULL,
    after_hash text,
    PRIMARY KEY(transaction_id,backend_pid,relation_name,operation,before_hash)
);
REVOKE ALL ON academic_result_removal_scope FROM PUBLIC;
DO $$ DECLARE runtime_role text; BEGIN
    IF current_user ~ '^so_[0-9a-fA-F]{32}_owner$' THEN
        runtime_role:=regexp_replace(current_user,'_owner$','_runtime');
        IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname=runtime_role) THEN
            EXECUTE format('REVOKE ALL ON academic_result_removal_scope FROM %I',runtime_role);
        END IF;
    END IF;
END $$;

CREATE FUNCTION academic_result_uuid(value text) RETURNS uuid LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    IF value IS NULL THEN RETURN NULL; END IF;
    IF NOT pg_input_is_valid(value,'uuid') THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    RETURN value::uuid;
END $$;

CREATE FUNCTION academic_guard_result_removal() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    IF EXISTS(SELECT 1 FROM academic_result_removal_scope s
        WHERE s.transaction_id=pg_current_xact_id() AND s.backend_pid=pg_backend_pid()
          AND s.relation_name=TG_TABLE_NAME AND s.operation=TG_OP
          AND s.before_hash=encode(sha256(convert_to(to_jsonb(OLD)::text,'UTF8')),'hex')
          AND s.after_hash IS NOT DISTINCT FROM CASE WHEN TG_OP='UPDATE' THEN encode(sha256(convert_to(to_jsonb(NEW)::text,'UTF8')),'hex') END)
    THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
    RAISE EXCEPTION 'ACADEMIC_OFFICIAL_RESULT_IMMUTABLE' USING ERRCODE='check_violation';
END $$;

CREATE FUNCTION academic_result_fraction_mean(values_json jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE v jsonb; n numeric:=0; d numeric:=1; count_rows bigint:=0; divisor numeric;
BEGIN
    FOR v IN SELECT value FROM jsonb_array_elements(values_json) LOOP
        IF (v->>'denominator')::numeric<=0 THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        n:=n*(v->>'denominator')::numeric+(v->>'numerator')::numeric*d;
        d:=d*(v->>'denominator')::numeric; divisor:=gcd(n,d); n:=n/divisor; d:=d/divisor;
        count_rows:=count_rows+1;
    END LOOP;
    IF count_rows=0 THEN RETURN NULL; END IF;
    d:=d*count_rows; divisor:=gcd(n,d); n:=n/divisor; d:=d/divisor;
    RETURN jsonb_build_object('numerator',n::numeric(1000,0)::text,'denominator',d::numeric(1000,0)::text,
        'decimal',trim(trailing '.' FROM trim(trailing '0' FROM trunc(n/d,12)::text)));
END $$;

-- This helper has no public execution privilege. Only the fixed command below
-- creates an exact before/after allowance; it never accepts arbitrary SQL.
CREATE FUNCTION academic_result_removal_allow(relation_name text,before_row jsonb,after_row jsonb DEFAULT NULL)
RETURNS void LANGUAGE sql SECURITY DEFINER SET search_path FROM CURRENT AS $$
    INSERT INTO academic_result_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),relation_name,
        CASE WHEN after_row IS NULL THEN 'DELETE' ELSE 'UPDATE' END,encode(sha256(convert_to(before_row::text,'UTF8')),'hex'),
        CASE WHEN after_row IS NOT NULL THEN encode(sha256(convert_to(after_row::text,'UTF8')),'hex') END)
    ON CONFLICT DO NOTHING;
$$;
REVOKE ALL ON FUNCTION academic_result_removal_allow(text,jsonb,jsonb) FROM PUBLIC;

DO $$ DECLARE r record; BEGIN
    FOR r IN SELECT * FROM (VALUES
        ('subject_term_evaluation_locks','subject_term_evaluation_locks_immutable'),
        ('subject_term_student_evaluations','subject_term_student_evaluations_immutable'),
        ('academic_course_result_locks','academic_course_result_locks_immutable'),
        ('academic_course_results','academic_course_results_immutable'),
        ('academic_activity_result_locks','academic_activity_result_locks_immutable'),
        ('academic_activity_results','academic_activity_results_immutable'),
        ('academic_result_corrections','academic_result_corrections_immutable'),
        ('academic_term_aggregate_revisions','academic_term_aggregate_revisions_immutable'),
        ('academic_annual_result_revisions','academic_annual_result_revisions_immutable'),
        ('academic_annual_result_term_sources','academic_annual_result_term_sources_immutable')
    ) owned(relation_name,trigger_name) LOOP
        EXECUTE format('DROP TRIGGER %I ON %I',r.trigger_name,r.relation_name);
        EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION academic_guard_result_removal()',r.trigger_name,r.relation_name);
    END LOOP;
END $$;

CREATE FUNCTION academic_result_credit_totals(courses jsonb,passing numeric) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE row jsonb; credits numeric; grade numeric; attempted numeric:=0; graded numeric:=0;
    earned numeric:=0; points numeric:=0; missing bigint:=0; exceptional bigint:=0; complete boolean;
BEGIN
    IF jsonb_typeof(courses) IS DISTINCT FROM 'array' OR passing<=0 OR passing>4 THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    IF EXISTS(SELECT 1 FROM jsonb_array_elements(courses) c GROUP BY c->>'subjectId' HAVING count(*)>1) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    FOR row IN SELECT value FROM jsonb_array_elements(courses) LOOP
        IF NOT(row ?& ARRAY['subjectId','learningOfferingId','resultId','effectiveVersion','credits','outcome','numericGrade']) THEN
            RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        credits:=(row->>'credits')::numeric;
        IF credits IS NULL OR credits<0 OR credits<>round(credits,2) OR credits::text IN('NaN','Infinity','-Infinity') THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        attempted:=attempted+credits;
        IF row->>'outcome'='numeric' AND row->>'resultId' IS NOT NULL AND (row->>'effectiveVersion')::bigint>0 THEN
            grade:=(row->>'numericGrade')::numeric;
            IF grade IS NULL OR grade<0 OR grade>4 OR mod(grade*2,1)<>0 THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            graded:=graded+credits; points:=points+credits*grade;
            IF grade>=passing THEN earned:=earned+credits; END IF;
        ELSIF row->>'outcome' IN('incomplete','insufficient_attendance') AND row->>'numericGrade' IS NULL
            AND row->>'resultId' IS NOT NULL AND (row->>'effectiveVersion')::bigint>0 THEN exceptional:=exceptional+1;
        ELSIF row->>'outcome' IS NULL AND row->>'numericGrade' IS NULL AND row->>'resultId' IS NULL AND row->>'effectiveVersion' IS NULL THEN missing:=missing+1;
        ELSE RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    END LOOP;
    complete:=jsonb_array_length(courses)>0 AND missing=0;
    RETURN jsonb_build_object('attemptedCredits',to_char(attempted,'FM999999999999990.00'),
        'gradedCredits',to_char(graded,'FM999999999999990.00'),'earnedCredits',to_char(earned,'FM999999999999990.00'),
        'unresolvedCredits',to_char(attempted-graded,'FM999999999999990.00'),
        'weightedGradePoints',to_char(points,'FM999999999999990.0000'),
        'provisionalGpa',CASE WHEN graded>0 THEN to_char(round(points/graded,2),'FM990.00') END,
        'missingResultCount',missing,'exceptionalResultCount',exceptional,'coverageComplete',complete,
        'allOutcomesNumeric',complete AND exceptional=0);
END $$;

CREATE FUNCTION academic_result_annual_totals(sources jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE s jsonb; attempted numeric:=0; graded numeric:=0; earned numeric:=0; points numeric:=0;
    missing bigint:=0; exceptional bigint:=0; complete boolean:=jsonb_array_length(sources)>0; numeric_only boolean:=true;
BEGIN
    FOR s IN SELECT value FROM jsonb_array_elements(sources) LOOP
        attempted:=attempted+(s->>'attemptedCredits')::numeric; graded:=graded+(s->>'gradedCredits')::numeric;
        earned:=earned+(s->>'earnedCredits')::numeric; points:=points+(s->>'weightedGradePoints')::numeric;
        missing:=missing+(s->>'missingResultCount')::bigint; exceptional:=exceptional+(s->>'exceptionalResultCount')::bigint;
        complete:=complete AND (s->>'coverageComplete')::boolean; numeric_only:=numeric_only AND (s->>'allOutcomesNumeric')::boolean;
    END LOOP;
    RETURN jsonb_build_object('attemptedCredits',to_char(attempted,'FM999999999999990.00'),
        'gradedCredits',to_char(graded,'FM999999999999990.00'),'earnedCredits',to_char(earned,'FM999999999999990.00'),
        'unresolvedCredits',to_char(attempted-graded,'FM999999999999990.00'),'weightedGradePoints',to_char(points,'FM999999999999990.0000'),
        'provisionalGpa',CASE WHEN graded>0 THEN to_char(round(points/graded,2),'FM990.00') END,
        'missingResultCount',missing,'exceptionalResultCount',exceptional,'coverageComplete',complete AND missing=0,
        'allOutcomesNumeric',complete AND numeric_only AND missing=0 AND exceptional=0);
END $$;

CREATE FUNCTION academic_result_without_offering(document jsonb,offering uuid,removed_evaluations uuid[],removed_groups uuid[],removed_results uuid[],whole_offering boolean)
RETURNS jsonb LANGUAGE plpgsql STABLE SET search_path FROM CURRENT AS $$
DECLARE courses jsonb; activities jsonb; totals jsonb; activity_totals jsonb; domain jsonb; domains jsonb:='[]';
    subject jsonb; subjects jsonb; criteria jsonb; catalog jsonb; average jsonb; level smallint;
    blockers jsonb:='[]'; holds jsonb:='[]'; policy jsonb; missing jsonb; expected jsonb; learner jsonb;
    complete boolean; n bigint; passed bigint; failed bigint; missing_count bigint;
BEGIN
    IF jsonb_typeof(document->'results'->'courses') IS DISTINCT FROM 'array'
        OR jsonb_typeof(document->'results'->'activities') IS DISTINCT FROM 'array'
        OR jsonb_typeof(document->'learnerEvaluations'->'domains') IS DISTINCT FROM 'array'
        OR jsonb_typeof(document->'policy') IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    policy:=document->'policy';
    IF NOT(policy ?& ARRAY['passingGrade','minimumLearnerLevel','allowReviewedHolds'])
        OR policy->>'passingGrade' IS NULL OR NOT pg_input_is_valid(policy->>'passingGrade','numeric')
        OR policy->>'minimumLearnerLevel' IS NULL OR NOT pg_input_is_valid(policy->>'minimumLearnerLevel','smallint')
        OR jsonb_typeof(policy->'allowReviewedHolds') IS DISTINCT FROM 'boolean'
        OR document->'learnerEvaluations'->>'policyVersionId' IS NULL
        OR NOT pg_input_is_valid(document->'learnerEvaluations'->>'policyVersionId','uuid') THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    IF EXISTS(SELECT 1 FROM jsonb_array_elements(document->'results'->'courses') c
        WHERE jsonb_typeof(c) IS DISTINCT FROM 'object' OR c->>'learningOfferingId' IS NULL
          OR NOT pg_input_is_valid(c->>'learningOfferingId','uuid')
          OR c->>'subjectId' IS NULL OR NOT pg_input_is_valid(c->>'subjectId','uuid')
          OR (c->>'resultId' IS NOT NULL AND NOT pg_input_is_valid(c->>'resultId','uuid'))
          OR NOT(c ?& ARRAY['subjectId','learningOfferingId','resultId','effectiveVersion','credits','outcome','numericGrade']))
       OR EXISTS(SELECT 1 FROM jsonb_array_elements(document->'results'->'activities') a
        WHERE jsonb_typeof(a) IS DISTINCT FROM 'object' OR a->>'learningOfferingId' IS NULL
          OR NOT pg_input_is_valid(a->>'learningOfferingId','uuid') OR a->>'learningGroupId' IS NULL
          OR NOT pg_input_is_valid(a->>'learningGroupId','uuid')
          OR NOT(a ?& ARRAY['learningGroupId','learningOfferingId','resultId','effectiveVersion','outcome'])
          OR NOT COALESCE((a->>'outcome' IN('pass','fail') AND a->>'resultId' IS NOT NULL AND pg_input_is_valid(a->>'resultId','uuid')
                    AND pg_input_is_valid(a->>'effectiveVersion','bigint') AND (a->>'effectiveVersion')::bigint>0)
             OR (a->>'outcome' IS NULL AND a->>'resultId' IS NULL AND a->>'effectiveVersion' IS NULL),false))
       THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    -- Validate every course before filtering so a malformed unrelated identity
    -- cannot turn SQL UNKNOWN into silently discarded official evidence.
    PERFORM academic_result_credit_totals(document->'results'->'courses',(policy->>'passingGrade')::numeric);
    IF NOT whole_offering AND EXISTS(SELECT 1 FROM jsonb_array_elements(document->'results'->'courses') c
        WHERE academic_result_uuid(c->>'learningOfferingId')=offering AND c->>'resultId' IS NULL) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GROUP_RESULTS_UNMAPPABLE'; END IF;
    SELECT COALESCE(jsonb_agg(c ORDER BY ordinal),'[]') INTO courses
        FROM jsonb_array_elements(document->'results'->'courses') WITH ORDINALITY t(c,ordinal)
        WHERE NOT((whole_offering AND academic_result_uuid(c->>'learningOfferingId')=offering) OR COALESCE(academic_result_uuid(c->>'resultId')=ANY(removed_results),false));
    SELECT COALESCE(jsonb_agg(c ORDER BY ordinal),'[]') INTO activities
        FROM jsonb_array_elements(document->'results'->'activities') WITH ORDINALITY t(c,ordinal)
        WHERE NOT((whole_offering AND academic_result_uuid(c->>'learningOfferingId')=offering) OR academic_result_uuid(c->>'learningGroupId')=ANY(removed_groups));
    totals:=academic_result_credit_totals(courses,(policy->>'passingGrade')::numeric);
    SELECT count(*),count(*) FILTER(WHERE a->>'outcome'='pass'),count(*) FILTER(WHERE a->>'outcome'='fail'),
        count(*) FILTER(WHERE a->>'resultId' IS NULL) INTO n,passed,failed,missing_count FROM jsonb_array_elements(activities) a;
    activity_totals:=jsonb_build_object('expectedGroupCount',n,'passedGroupCount',passed,'failedGroupCount',failed,
        'missingResultCount',missing_count,'coverageComplete',n>0 AND missing_count=0,'allPassed',n>0 AND missing_count=0 AND failed=0);
    SELECT COALESCE(jsonb_agg(c->'subjectId' ORDER BY c->>'subjectId'),'[]') INTO expected FROM jsonb_array_elements(courses) c;
    FOR domain IN SELECT value FROM jsonb_array_elements(document->'learnerEvaluations'->'domains') LOOP
        IF jsonb_typeof(domain->'subjects') IS DISTINCT FROM 'array' OR jsonb_typeof(domain->'missingSubjects') IS DISTINCT FROM 'array' THEN
            RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        subjects:='[]';
        FOR subject IN SELECT value FROM jsonb_array_elements(domain->'subjects') LOOP
            IF jsonb_typeof(subject) IS DISTINCT FROM 'object' OR subject->>'subjectId' IS NULL
                OR NOT pg_input_is_valid(subject->>'subjectId','uuid') OR jsonb_typeof(subject->'criteria') IS DISTINCT FROM 'array'
                OR EXISTS(SELECT 1 FROM jsonb_array_elements(subject->'criteria') c WHERE jsonb_typeof(c) IS DISTINCT FROM 'object'
                    OR c->>'id' IS NULL OR NOT pg_input_is_valid(c->>'id','uuid') OR c->>'qualityLevel' IS NULL
                    OR NOT pg_input_is_valid(c->>'qualityLevel','smallint')
                    OR CASE WHEN pg_input_is_valid(c->>'qualityLevel','smallint') THEN (c->>'qualityLevel')::smallint NOT BETWEEN 0 AND 3 ELSE true END
                    OR (c->>'schoolCriterionId' IS NOT NULL AND NOT pg_input_is_valid(c->>'schoolCriterionId','uuid'))) THEN
                RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            IF NOT(expected @> jsonb_build_array(subject->'subjectId')) THEN CONTINUE; END IF;
            SELECT COALESCE(jsonb_agg(c ORDER BY ordinal),'[]') INTO criteria
                FROM jsonb_array_elements(subject->'criteria') WITH ORDINALITY t(c,ordinal)
                WHERE NOT(academic_result_uuid(c->>'id')=ANY(removed_evaluations));
            IF jsonb_array_length(criteria)=0 THEN CONTINUE; END IF;
            SELECT academic_result_fraction_mean(jsonb_agg(jsonb_build_object('numerator',c->>'qualityLevel','denominator','1')))
                INTO average FROM jsonb_array_elements(criteria) c;
            subjects:=subjects||jsonb_build_array(subject||jsonb_build_object('criteria',criteria,'average',average));
        END LOOP;
        SELECT academic_result_fraction_mean(COALESCE(jsonb_agg(s->'average'),'[]')) INTO average FROM jsonb_array_elements(subjects) s;
        SELECT quality_level INTO level FROM academic_learner_evaluation_policy_bands
            WHERE policy_version_id=(document->'learnerEvaluations'->>'policyVersionId')::uuid
              AND average IS NOT NULL AND (average->>'numerator')::numeric>=lower_bound*(average->>'denominator')::numeric
            ORDER BY lower_bound DESC LIMIT 1;
        SELECT COALESCE(jsonb_agg(item ORDER BY item->>'schoolCriterionId'),'[]') INTO catalog FROM (
            SELECT jsonb_build_object('schoolCriterionId',c->'schoolCriterionId','average',
                academic_result_fraction_mean(jsonb_agg(jsonb_build_object('numerator',c->>'qualityLevel','denominator','1')))) item
            FROM jsonb_array_elements(subjects) s CROSS JOIN LATERAL jsonb_array_elements(s->'criteria') c
            WHERE c->>'schoolCriterionId' IS NOT NULL GROUP BY c->'schoolCriterionId') grouped;
        SELECT COALESCE(jsonb_agg(m ORDER BY m->>'subjectId'),'[]') INTO missing FROM jsonb_array_elements(domain->'missingSubjects') m
            WHERE expected @> jsonb_build_array(m->'subjectId');
        -- A removed criterion is no longer an obligation. Remaining expected
        -- subjects must still have a real summary before reporting completeness.
        complete:=jsonb_array_length(expected)>0 AND missing='[]' AND jsonb_array_length(subjects)=jsonb_array_length(expected);
        domains:=domains||jsonb_build_array(domain||jsonb_build_object('subjects',subjects,'catalogCriteria',catalog,
            'average',average,'qualityLevel',level,'missingSubjects',missing,'complete',complete));
    END LOOP;
    learner:=document->'learnerEvaluations'||jsonb_build_object('domains',domains);
    IF courses='[]' THEN blockers:=blockers||'"no_course_coverage"'::jsonb; END IF;
    IF (totals->>'missingResultCount')::bigint>0 THEN blockers:=blockers||'"missing_course_results"'::jsonb; END IF;
    IF missing_count>0 THEN blockers:=blockers||'"missing_activity_results"'::jsonb; END IF;
    IF jsonb_array_length(domains)<>2 OR EXISTS(SELECT 1 FROM jsonb_array_elements(domains) d
        WHERE NOT COALESCE((d->>'complete')::boolean,false) OR d->>'qualityLevel' IS NULL) THEN
        blockers:=blockers||'"missing_learner_evaluations"'::jsonb; END IF;
    IF (totals->>'exceptionalResultCount')::bigint>0 THEN holds:=holds||'"exceptional_course_outcomes"'::jsonb; END IF;
    IF failed>0 THEN holds:=holds||'"failed_activities"'::jsonb; END IF;
    IF EXISTS(SELECT 1 FROM jsonb_array_elements(domains) d WHERE (d->>'qualityLevel')::smallint<(policy->>'minimumLearnerLevel')::smallint) THEN
        holds:=holds||'"failed_learner_evaluations"'::jsonb; END IF;
    IF holds<>'[]' AND NOT(policy->>'allowReviewedHolds')::boolean THEN blockers:=blockers||'"reviewed_hold_not_allowed"'::jsonb; END IF;
    document:=jsonb_set(document,'{results}',document->'results'||jsonb_build_object('courses',courses,'activities',activities,
        'totals',totals,'activityTotals',activity_totals,'sourceChecksum',encode(sha256(convert_to((courses||activities)::text,'UTF8')),'hex')));
    document:=document||jsonb_build_object('learnerEvaluations',learner,'blockers',blockers,'holdFindings',holds,'canLock',blockers='[]');
    document:=document||jsonb_build_object('sourceChecksum',encode(sha256(convert_to(document::text,'UTF8')),'hex'));
    IF whole_offering AND position(offering::text IN document::text)>0 THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
    RETURN document;
END $$;

CREATE FUNCTION academic_remove_scoped_results(selected_offering uuid,selected_groups uuid[],whole_offering boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE context record; owned_table text; source record; replacement jsonb; changed jsonb; groups jsonb;
    removed_evaluations uuid[]; removed_results uuid[]; before_row jsonb; after_row jsonb; terms jsonb; term jsonb; total_sources jsonb;
    totals jsonb; gpa numeric; checksum text;
BEGIN
    SELECT academic_year_id,academic_term_id INTO context FROM learning_offerings WHERE id=selected_offering;
    IF NOT FOUND THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_OFFERING_NOT_FOUND'; END IF;
    PERFORM 1 FROM academic_terms WHERE id=context.academic_term_id AND academic_year_id=context.academic_year_id FOR UPDATE;
    PERFORM 1 FROM learning_offerings WHERE id=selected_offering FOR UPDATE;
    PERFORM 1 FROM learning_groups WHERE learning_offering_id=selected_offering ORDER BY id FOR UPDATE;
    IF cardinality(selected_groups) IS NULL OR EXISTS(SELECT 1 FROM unnest(selected_groups) id
        WHERE NOT EXISTS(SELECT 1 FROM learning_groups g WHERE g.id=id AND g.learning_offering_id=selected_offering)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GROUP_RESULTS_UNMAPPABLE'; END IF;
    SELECT COALESCE(array_agg(id),'{}') INTO removed_evaluations FROM subject_term_student_evaluations
        WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups);
    SELECT COALESCE(array_agg(id),'{}') INTO removed_results FROM academic_course_results
        WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups);
    FOR source IN SELECT c.* FROM academic_result_corrections c
        WHERE c.course_result_id IN(SELECT id FROM academic_course_results WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups))
           OR c.activity_result_id IN(SELECT id FROM academic_activity_results WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups))
           OR c.subject_student_evaluation_id=ANY(removed_evaluations) FOR UPDATE LOOP
        PERFORM academic_result_removal_allow('academic_result_corrections',to_jsonb(source));
        DELETE FROM academic_result_corrections WHERE id=source.id;
    END LOOP;
    FOREACH owned_table IN ARRAY ARRAY['academic_course_results','academic_activity_results','subject_term_student_evaluations','academic_activity_result_locks'] LOOP
        FOR before_row IN EXECUTE format('SELECT to_jsonb(r) FROM %I r WHERE learning_offering_id=$1 AND learning_group_id=ANY($2) FOR UPDATE',owned_table) USING selected_offering,selected_groups LOOP
            PERFORM academic_result_removal_allow(owned_table,before_row);
            EXECUTE format('DELETE FROM %I WHERE id=$1',owned_table) USING academic_result_uuid(before_row->>'id');
        END LOOP;
    END LOOP;
    FOREACH owned_table IN ARRAY ARRAY['academic_course_result_locks','subject_term_evaluation_locks'] LOOP
        FOR before_row IN EXECUTE format('SELECT to_jsonb(r) FROM %I r WHERE academic_term_id=$1 AND position($2 IN source_snapshot::text)>0 FOR UPDATE',owned_table)
            USING context.academic_term_id,selected_offering::text LOOP
            IF jsonb_typeof(before_row->'source_snapshot'->'groups') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            IF EXISTS(SELECT 1 FROM jsonb_array_elements(before_row->'source_snapshot'->'groups') g
                WHERE jsonb_typeof(g) IS DISTINCT FROM 'object' OR g->>'learningOfferingId' IS NULL
                  OR NOT pg_input_is_valid(g->>'learningOfferingId','uuid') OR g->>'learningGroupId' IS NULL
                  OR NOT pg_input_is_valid(g->>'learningGroupId','uuid')
                  OR NOT EXISTS(SELECT 1 FROM learning_groups owned WHERE owned.id=academic_result_uuid(g->>'learningGroupId')
                      AND owned.learning_offering_id=academic_result_uuid(g->>'learningOfferingId')
                      AND owned.academic_term_id=context.academic_term_id AND owned.academic_year_id=context.academic_year_id)) THEN
                RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            IF NOT EXISTS(SELECT 1 FROM jsonb_array_elements(before_row->'source_snapshot'->'groups') g
                WHERE academic_result_uuid(g->>'learningGroupId')=ANY(selected_groups)) THEN CONTINUE; END IF;
            SELECT COALESCE(jsonb_agg(g ORDER BY ordinal),'[]') INTO groups
                FROM jsonb_array_elements(before_row->'source_snapshot'->'groups') WITH ORDINALITY t(g,ordinal)
                WHERE NOT(academic_result_uuid(g->>'learningGroupId')=ANY(selected_groups));
            IF groups='[]' THEN
                PERFORM academic_result_removal_allow(owned_table,before_row);
                EXECUTE format('DELETE FROM %I WHERE id=$1',owned_table) USING academic_result_uuid(before_row->>'id');
            ELSE
                replacement:=jsonb_set(before_row->'source_snapshot','{groups}',groups);
                IF whole_offering AND position(selected_offering::text IN replacement::text)>0 THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
                checksum:=encode(sha256(convert_to(replacement::text,'UTF8')),'hex');
                after_row:=before_row||jsonb_build_object('source_snapshot',replacement,'source_checksum',checksum,
                    'roster_checksum',checksum,'row_version',(before_row->>'row_version')::bigint+1);
                PERFORM academic_result_removal_allow(owned_table,before_row,after_row);
                EXECUTE format('UPDATE %I SET source_snapshot=$2,source_checksum=$3,roster_checksum=$3,row_version=row_version+1 WHERE id=$1',owned_table)
                    USING academic_result_uuid(before_row->>'id'),replacement,checksum;
            END IF;
        END LOOP;
    END LOOP;
    FOR source IN SELECT r.* FROM academic_term_aggregate_revisions r
        WHERE academic_term_id=context.academic_term_id AND ((whole_offering AND position(selected_offering::text IN snapshot::text)>0)
            OR (NOT whole_offering AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=ANY(selected_groups)
                AND m.student_academic_year_id=r.student_academic_year_id)
                AND (jsonb_path_exists(snapshot,'$.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))
                  OR jsonb_path_exists(snapshot,'$.terms[*].revision.snapshot.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))))
            OR EXISTS(SELECT 1 FROM unnest(removed_results) e WHERE position(e::text IN snapshot::text)>0)
            OR EXISTS(SELECT 1 FROM unnest(selected_groups) e WHERE position(e::text IN snapshot::text)>0)
            OR EXISTS(SELECT 1 FROM unnest(removed_evaluations) e WHERE position(e::text IN snapshot::text)>0)) FOR UPDATE LOOP
        replacement:=academic_result_without_offering(source.snapshot,selected_offering,removed_evaluations,selected_groups,removed_results,whole_offering);
        gpa:=CASE WHEN source.hold_reason IS NULL AND (replacement->>'canLock')::boolean AND replacement->'holdFindings'='[]'::jsonb
            THEN (replacement->'results'->'totals'->>'provisionalGpa')::numeric END;
        checksum:=replacement->>'sourceChecksum';
        after_row:=to_jsonb(source)||jsonb_build_object('snapshot',replacement,'source_checksum',checksum,'official_gpa',gpa);
        PERFORM academic_result_removal_allow('academic_term_aggregate_revisions',to_jsonb(source),after_row);
        UPDATE academic_term_aggregate_revisions SET snapshot=replacement,source_checksum=checksum,official_gpa=gpa WHERE id=source.id;
    END LOOP;
    FOR source IN SELECT r.* FROM academic_annual_result_revisions r
        WHERE academic_year_id=context.academic_year_id AND ((whole_offering AND position(selected_offering::text IN snapshot::text)>0)
            OR (NOT whole_offering AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=ANY(selected_groups)
                AND m.student_academic_year_id=r.student_academic_year_id)
                AND (jsonb_path_exists(snapshot,'$.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))
                  OR jsonb_path_exists(snapshot,'$.terms[*].revision.snapshot.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))))
            OR EXISTS(SELECT 1 FROM unnest(removed_results) e WHERE position(e::text IN snapshot::text)>0)
            OR EXISTS(SELECT 1 FROM unnest(selected_groups) e WHERE position(e::text IN snapshot::text)>0)
            OR EXISTS(SELECT 1 FROM unnest(removed_evaluations) e WHERE position(e::text IN snapshot::text)>0)) FOR UPDATE LOOP
        IF jsonb_typeof(source.snapshot->'terms') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        terms:='[]'; total_sources:='[]';
        FOR term IN SELECT value FROM jsonb_array_elements(source.snapshot->'terms') LOOP
            IF jsonb_typeof(term) IS DISTINCT FROM 'object' OR NOT(term ?& ARRAY['academicTermId','termName','sequence','isCurrent','revision'])
                OR term->>'academicTermId' IS NULL OR NOT pg_input_is_valid(term->>'academicTermId','uuid')
                OR jsonb_typeof(term->'revision') NOT IN('object','null') THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            IF jsonb_typeof(term->'revision')='object' AND (NOT(term->'revision' ?& ARRAY['id','snapshot','officialGpa','isCurrent'])
                OR term->'revision'->>'id' IS NULL OR NOT pg_input_is_valid(term->'revision'->>'id','uuid')) THEN
                RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
            IF term->'revision'<>'null'::jsonb AND academic_result_uuid(term->>'academicTermId')=context.academic_term_id THEN
                SELECT to_jsonb(r) INTO changed FROM academic_term_aggregate_revisions r WHERE id=(term->'revision'->>'id')::uuid;
                IF changed IS NULL THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
                term:=jsonb_set(term,'{revision,snapshot}',changed->'snapshot');
                term:=jsonb_set(term,'{revision,officialGpa}',COALESCE(to_jsonb(changed->>'official_gpa'),'null'));
                term:=jsonb_set(term,'{revision,isCurrent}','false'); term:=jsonb_set(term,'{isCurrent}','false');
            END IF;
            terms:=terms||jsonb_build_array(term);
            IF term->'revision'<>'null'::jsonb THEN total_sources:=total_sources||jsonb_build_array(term->'revision'->'snapshot'->'results'->'totals'); END IF;
        END LOOP;
        totals:=academic_result_annual_totals(total_sources);
        totals:=totals||jsonb_build_object('coverageComplete',false,'allOutcomesNumeric',false);
        replacement:=source.snapshot||jsonb_build_object('terms',terms,'totals',totals,'canLock',false);
        checksum:=encode(sha256(convert_to(replacement::text,'UTF8')),'hex'); replacement:=replacement||jsonb_build_object('sourceChecksum',checksum);
        IF whole_offering AND position(selected_offering::text IN replacement::text)>0 THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE'; END IF;
        after_row:=to_jsonb(source)||jsonb_build_object('snapshot',replacement,'source_checksum',checksum,'official_gpa',NULL);
        PERFORM academic_result_removal_allow('academic_annual_result_revisions',to_jsonb(source),after_row);
        UPDATE academic_annual_result_revisions SET snapshot=replacement,source_checksum=checksum,official_gpa=NULL WHERE id=source.id;
    END LOOP;
    DELETE FROM academic_result_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid();
END $$;
REVOKE ALL ON FUNCTION academic_guard_result_removal() FROM PUBLIC;

CREATE FUNCTION academic_current_results_deletion_impact(selected_offering uuid,selected_groups uuid[],whole_offering boolean DEFAULT false)
RETURNS jsonb LANGUAGE sql STABLE SET search_path FROM CURRENT AS $$
WITH context AS (SELECT academic_term_id,academic_year_id FROM learning_offerings WHERE id=selected_offering),
official AS (
    SELECT 'course' kind,id,to_jsonb(r) fact FROM academic_course_results r WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups)
    UNION ALL SELECT 'activity',id,to_jsonb(r) FROM academic_activity_results r WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups)
    UNION ALL SELECT 'evaluation',id,to_jsonb(r) FROM subject_term_student_evaluations r WHERE learning_offering_id=selected_offering AND learning_group_id=ANY(selected_groups)
), corrections AS(SELECT c.* FROM academic_result_corrections c WHERE c.course_result_id IN(SELECT id FROM official WHERE kind='course')
    OR c.activity_result_id IN(SELECT id FROM official WHERE kind='activity') OR c.subject_student_evaluation_id IN(SELECT id FROM official WHERE kind='evaluation')),
terms AS(SELECT r.* FROM academic_term_aggregate_revisions r CROSS JOIN context c WHERE r.academic_term_id=c.academic_term_id
    AND ((whole_offering AND position(selected_offering::text IN snapshot::text)>0)
        OR (NOT whole_offering AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=ANY(selected_groups)
            AND m.student_academic_year_id=r.student_academic_year_id)
            AND (jsonb_path_exists(snapshot,'$.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))
              OR jsonb_path_exists(snapshot,'$.terms[*].revision.snapshot.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))))
        OR EXISTS(SELECT 1 FROM official o WHERE position(o.id::text IN snapshot::text)>0)
        OR EXISTS(SELECT 1 FROM unnest(selected_groups) g WHERE position(g::text IN snapshot::text)>0))),
annuals AS(SELECT r.* FROM academic_annual_result_revisions r CROSS JOIN context c WHERE r.academic_year_id=c.academic_year_id
    AND ((whole_offering AND position(selected_offering::text IN snapshot::text)>0)
        OR (NOT whole_offering AND EXISTS(SELECT 1 FROM learning_group_students m WHERE m.learning_group_id=ANY(selected_groups)
            AND m.student_academic_year_id=r.student_academic_year_id)
            AND (jsonb_path_exists(snapshot,'$.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))
              OR jsonb_path_exists(snapshot,'$.terms[*].revision.snapshot.results.courses[*] ? (@.learningOfferingId == $off && @.resultId == null)',jsonb_build_object('off',selected_offering))))
        OR EXISTS(SELECT 1 FROM official o WHERE position(o.id::text IN snapshot::text)>0)
        OR EXISTS(SELECT 1 FROM unnest(selected_groups) g WHERE position(g::text IN snapshot::text)>0))),
facts AS(SELECT kind,id,fact FROM official UNION ALL SELECT 'correction',id,to_jsonb(c) FROM corrections c
    UNION ALL SELECT 'term',id,to_jsonb(t) FROM terms t UNION ALL SELECT 'annual',id,to_jsonb(a) FROM annuals a
    UNION ALL SELECT 'course_lock',id,to_jsonb(r) FROM academic_course_result_locks r CROSS JOIN context c
        WHERE r.academic_term_id=c.academic_term_id AND position(selected_offering::text IN r.source_snapshot::text)>0
    UNION ALL SELECT 'evaluation_lock',id,to_jsonb(r) FROM subject_term_evaluation_locks r CROSS JOIN context c
        WHERE r.academic_term_id=c.academic_term_id AND position(selected_offering::text IN r.source_snapshot::text)>0)
SELECT jsonb_build_object('officialResults',(SELECT count(*) FROM official),'resultCorrections',(SELECT count(*) FROM corrections),
    'termAggregates',(SELECT count(*) FROM terms),'annualAggregates',(SELECT count(*) FROM annuals),
    'sourceDigest',encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(fact ORDER BY kind,id)::text FROM facts),'[]'),'UTF8')),'hex'));
$$;

REVOKE ALL ON FUNCTION academic_remove_scoped_results(uuid,uuid[],boolean) FROM PUBLIC;
CREATE FUNCTION academic_remove_offering_results(selected_offering uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    PERFORM academic_remove_scoped_results(selected_offering,
        COALESCE((SELECT array_agg(id ORDER BY id) FROM learning_groups WHERE learning_offering_id=selected_offering),'{}'),true);
END $$;
CREATE FUNCTION academic_remove_group_results(selected_group uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE offering uuid;
BEGIN
    SELECT learning_offering_id INTO offering FROM learning_groups WHERE id=selected_group;
    IF offering IS NULL THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_GROUP_RESULTS_UNMAPPABLE'; END IF;
    PERFORM academic_remove_scoped_results(offering,ARRAY[selected_group],false);
END $$;
-- Bind definer resolution to the schema owned at installation, with pg_temp
-- explicitly last. This also supports the repository's isolated test schemas.
DO $$ DECLARE owned_schema text:=current_schema(); f record; BEGIN
    FOR f IN SELECT p.oid::regprocedure identity FROM pg_proc p
        WHERE p.pronamespace=owned_schema::regnamespace AND p.proname=ANY(ARRAY[
            'academic_guard_result_removal','academic_result_removal_allow','academic_remove_scoped_results',
            'academic_remove_offering_results','academic_remove_group_results','academic_result_without_offering',
            'academic_result_uuid','academic_result_credit_totals','academic_result_fraction_mean','academic_result_annual_totals','academic_current_results_deletion_impact']) LOOP
        EXECUTE format('ALTER FUNCTION %s SET search_path TO pg_catalog,%I,pg_temp',f.identity,owned_schema);
    END LOOP;
END $$;
