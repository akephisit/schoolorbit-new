-- Old dates are migration input only. All transformed intent is reconciled before
-- removing the boundary contract; unknown dependencies refuse without CASCADE.
LOCK TABLE academic_years,academic_terms,calendar_events,calendar_event_targets,
    attendance_settings,attendance_days,academic_promotion_runs,behavior_settings
    IN SHARE ROW EXCLUSIVE MODE;

DO $$ BEGIN
    IF EXISTS(
        SELECT 1 FROM pg_proc function_record JOIN pg_namespace namespace ON namespace.oid=function_record.pronamespace
        WHERE namespace.nspname=current_schema()
          AND function_record.proname NOT IN('academic_assert_term_within_year','academic_validate_offering_availability')
          AND ((function_record.prosrc ~* '\macademic_(years|terms)\M'
                AND function_record.prosrc ~* '\m(start_date|end_date|planned_end_date|closed_on)\M')
               OR (function_record.prosrc ~* '\m(start_date|end_date|planned_end_date|closed_on)\M' AND EXISTS(
                   SELECT 1 FROM pg_trigger trigger_record WHERE trigger_record.tgfoid=function_record.oid
                     AND trigger_record.tgrelid IN('academic_years'::regclass,'academic_terms'::regclass))))
    ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_UNKNOWN_DATE_DEPENDENCY'; END IF;
END $$;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM pg_constraint rule WHERE rule.conrelid IN('academic_years'::regclass,'academic_terms'::regclass)
        AND rule.contype='c' AND pg_get_constraintdef(rule.oid) ~* '\m(start_date|end_date|planned_end_date|closed_on)\M'
        AND NOT (
          (rule.conrelid='academic_years'::regclass AND rule.conname='academic_years_date_order_check'
            AND pg_get_constraintdef(rule.oid)='CHECK ((start_date <= end_date))')
          OR (rule.conrelid='academic_terms'::regclass AND rule.conname='academic_terms_date_order_check'
            AND pg_get_constraintdef(rule.oid)='CHECK ((start_date <= planned_end_date))')))
       OR EXISTS(SELECT 1 FROM pg_index index_record WHERE index_record.indrelid IN('academic_years'::regclass,'academic_terms'::regclass)
        AND pg_get_indexdef(index_record.indexrelid) ~* '\m(start_date|end_date|planned_end_date|closed_on)\M')
       OR EXISTS(SELECT 1 FROM pg_proc function_record JOIN pg_namespace namespace ON namespace.oid=function_record.pronamespace
        WHERE namespace.nspname=current_schema() AND ((function_record.proname='academic_assert_term_within_year' AND encode(sha256(convert_to(function_record.prosrc,'UTF8')),'hex')<>'72cf708c0dfd2df8916342573625724a797bc472d325c3dcef26584ccf844bfb') OR (function_record.proname='academic_validate_offering_availability' AND encode(sha256(convert_to(function_record.prosrc,'UTF8')),'hex')<>'b51ff23a6686b7a91f88530cb16fbecb77806a85ad5e0fc6f6b184905e93947a')))
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_UNKNOWN_DATE_DEPENDENCY'; END IF;
END $$;
CREATE TEMP TABLE academic147_expected_promotion_dates ON COMMIT DROP AS
SELECT run.id,year.start_date AS target_placement_starts_on FROM academic_promotion_runs run
JOIN academic_years year ON year.id=run.target_year_id;
CREATE TEMP TABLE academic147_existing_targets ON COMMIT DROP AS
SELECT * FROM calendar_event_targets;
CREATE TEMP TABLE academic147_existing_days ON COMMIT DROP AS SELECT * FROM attendance_days;
CREATE TEMP TABLE academic147_calendar_mapping ON COMMIT DROP AS
SELECT target.id,
       count(year.id)::bigint AS candidate_count,
       min(year.id::text)::uuid AS academic_year_id
FROM calendar_event_targets target
JOIN calendar_events event ON event.id=target.event_id
LEFT JOIN homerooms room ON room.id=target.homeroom_id
LEFT JOIN academic_years year
  ON (target.homeroom_id IS NOT NULL AND year.id=room.academic_year_id)
  OR (target.homeroom_id IS NULL AND event.start_date BETWEEN year.start_date AND year.end_date)
WHERE target.academic_year_id IS NULL
  AND (target.grade_level_id IS NOT NULL OR target.homeroom_id IS NOT NULL)
GROUP BY target.id;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic147_calendar_mapping WHERE candidate_count<>1) THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_CALENDAR_YEAR_UNMAPPABLE';
    END IF;
    IF EXISTS(
        SELECT 1 FROM calendar_event_targets target JOIN homerooms room ON room.id=target.homeroom_id
        WHERE target.academic_year_id IS NOT NULL AND target.academic_year_id<>room.academic_year_id
    ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_CALENDAR_ROOM_YEAR_MISMATCH'; END IF;
    IF (SELECT COALESCE(sum(GREATEST(0,COALESCE(term.closed_on,term.planned_end_date,year.end_date)-term.start_date+1)),0)
        FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id)>2500000 THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_CALENDAR_MATERIALIZATION_TOO_LARGE';
    END IF;
END $$;

-- Every old date's counting decision becomes an Attendance-owned calendar fact.
CREATE TEMP TABLE academic147_expected_days ON COMMIT DROP AS
SELECT term.id AS academic_term_id,day.date::date AS date,
       COALESCE(existing.counted,COALESCE(settings.configuration->'weekdays','[1,2,3,4,5]'::jsonb)
           @> jsonb_build_array(extract(isodow FROM day.date)::integer)) AS counted,
       COALESCE(existing.note,'') AS note
FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id
LEFT JOIN attendance_settings settings ON settings.academic_term_id=term.id
CROSS JOIN LATERAL generate_series(term.start_date::timestamp,
    COALESCE(term.closed_on,term.planned_end_date,year.end_date)::timestamp,interval '1 day') day(date)
LEFT JOIN attendance_days existing ON existing.academic_term_id=term.id AND existing.date=day.date::date;

-- Old checksums retain their accepted meaning. Recover the exact full request
-- from the original audit, then store one current canonical replay request.
CREATE FUNCTION pg_temp.academic147_compact_json(input json) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE source text:=input::text; result text:=''; quoted boolean:=false; escaped boolean:=false; character text; position integer;
BEGIN
    FOR position IN 1..length(source) LOOP
        character:=substr(source,position,1);
        IF quoted THEN
            result:=result||character;
            IF escaped THEN escaped:=false;
            ELSIF character=chr(92) THEN escaped:=true;
            ELSIF character='"' THEN quoted:=false; END IF;
        ELSIF character='"' THEN quoted:=true;result:=result||character;
        ELSIF character NOT IN(' ',chr(9),chr(10),chr(13)) THEN result:=result||character;
        END IF;
    END LOOP;
    RETURN result;
END $$;
CREATE FUNCTION pg_temp.academic147_strip_intent_dates(document jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE result jsonb; member record;
BEGIN
    IF jsonb_typeof(document)='object' THEN
        result:='{}'::jsonb;
        FOR member IN SELECT * FROM jsonb_each(document) LOOP
            IF member.key<>'targetPlacementStartsOn' THEN result:=result||jsonb_build_object(member.key,pg_temp.academic147_strip_intent_dates(member.value)); END IF;
        END LOOP;RETURN result;
    ELSIF jsonb_typeof(document)='array' THEN
        SELECT COALESCE(jsonb_agg(pg_temp.academic147_strip_intent_dates(value) ORDER BY position),'[]'::jsonb) INTO result FROM jsonb_array_elements(document) WITH ORDINALITY items(value,position);
        RETURN result;
    END IF;RETURN document;
END $$;
CREATE FUNCTION pg_temp.academic147_strip_replay_context(document jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN jsonb_typeof(document->'context')='object' THEN jsonb_set(document,'{context}',(document->'context')-ARRAY['startDate','endDate','yearStartDate','yearEndDate','termStartDate','plannedEndDate','closedOn','operationDate']) ELSE document END
$$;
CREATE TEMP TABLE academic147_term_request_sources ON COMMIT DROP AS
SELECT receipt.request_id,receipt.academic_year_id,receipt.academic_term_id,receipt.actor_user_id,receipt.action,receipt.request_checksum,receipt.outcome,
       receipt.created_at,
       CASE WHEN jsonb_typeof(audit.payload)='array' THEN audit.payload->0 ELSE audit.payload->'request' END AS original_request,
       CASE WHEN jsonb_typeof(audit.payload)='array' THEN audit.payload->1 ELSE audit.payload->'outcome' END AS original_outcome
FROM academic_term_transition_receipts receipt
JOIN academic_audit_events audit ON audit.actor_user_id=receipt.actor_user_id
 AND audit.entity_type='academic_term' AND audit.entity_id=receipt.academic_term_id
 AND audit.academic_year_id=receipt.academic_year_id AND audit.academic_term_id=receipt.academic_term_id
 AND audit.event_code='academic.term.'||receipt.action
 AND (CASE WHEN jsonb_typeof(audit.payload)='array' THEN audit.payload->0 ELSE audit.payload->'request' END)->>'requestId'=receipt.request_id::text;
DO $$ BEGIN
    IF EXISTS(SELECT receipt.request_id FROM academic_term_transition_receipts receipt LEFT JOIN academic147_term_request_sources source ON source.request_id=receipt.request_id GROUP BY receipt.request_id HAVING count(source.request_id)<>1)
       OR EXISTS(SELECT 1 FROM academic147_term_request_sources source WHERE
        jsonb_typeof(original_request)<>'object'
        OR NOT original_request ?& ARRAY['academicYearId','requestId','action','expectedYearVersion','expectedTermVersion','readinessChecksum','acknowledgedWarningCodes','closedOn','reason']
        OR original_request-ARRAY['academicYearId','requestId','action','expectedYearVersion','expectedTermVersion','readinessChecksum','acknowledgedWarningCodes','closedOn','reason']<>'{}'::jsonb
        OR original_request->>'academicYearId' IS DISTINCT FROM academic_year_id::text
        OR original_request->>'action' IS DISTINCT FROM action
        OR original_outcome IS DISTINCT FROM outcome
        OR outcome->'context'->>'academicYearId' IS DISTINCT FROM academic_year_id::text
        OR outcome->'context'->>'academicTermId' IS DISTINCT FROM academic_term_id::text
        OR request_checksum<>encode(sha256(convert_to(pg_temp.academic147_compact_json(json_build_array(actor_user_id,academic_term_id,json_build_object(
            'academicYearId',original_request->'academicYearId','requestId',original_request->'requestId','action',original_request->'action',
            'expectedYearVersion',original_request->'expectedYearVersion','expectedTermVersion',original_request->'expectedTermVersion',
            'readinessChecksum',original_request->'readinessChecksum','acknowledgedWarningCodes',original_request->'acknowledgedWarningCodes',
            'closedOn',original_request->'closedOn','reason',original_request->'reason'))),'UTF8')),'hex'))
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_TERM_RECEIPT_SOURCE_UNMAPPABLE'; END IF;
END $$;
CREATE TEMP TABLE academic147_expected_term_receipts ON COMMIT DROP AS
SELECT request_id,original_request-'closedOn' AS canonical_request,
       jsonb_set(pg_temp.academic147_strip_replay_context(outcome),'{context}',
         (pg_temp.academic147_strip_replay_context(outcome)->'context')||jsonb_build_object('operationDate',(created_at AT TIME ZONE 'Asia/Bangkok')::date)) AS outcome
FROM academic147_term_request_sources;
CREATE TEMP TABLE academic147_expected_year_receipts ON COMMIT DROP AS
SELECT receipt.request_id,CASE WHEN receipt.action='activate' THEN term.outcome ELSE pg_temp.academic147_strip_replay_context(receipt.outcome) END AS outcome
FROM academic_year_transition_receipts receipt LEFT JOIN academic147_expected_term_receipts term ON term.request_id=receipt.request_id;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_year_transition_receipts year LEFT JOIN academic_term_transition_receipts term ON term.request_id=year.request_id
        WHERE year.action='activate' AND (term.request_id IS NULL OR term.actor_user_id<>year.actor_user_id OR term.academic_year_id<>year.academic_year_id OR term.action<>'activate' OR term.outcome IS DISTINCT FROM year.outcome OR term.request_checksum<>year.request_checksum))
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_YEAR_ACTIVATION_RECEIPT_UNMAPPABLE'; END IF;
END $$;
CREATE TEMP TABLE academic147_expected_promotion_item_dates ON COMMIT DROP AS
SELECT item.id,item.run_id,
       CASE WHEN receipt.target_placement_id IS NOT NULL THEN placement.start_date
            WHEN item.status<>'executed' AND item.decision->>'targetHomeroomId' IS NOT NULL THEN year.start_date END AS target_placement_starts_on
FROM academic_promotion_run_items item JOIN academic_years year ON year.id=item.target_year_id
LEFT JOIN academic_promotion_execution_receipts receipt ON receipt.item_id=item.id AND receipt.run_id=item.run_id
LEFT JOIN homeroom_placements placement ON placement.id=receipt.target_placement_id;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_promotion_run_items item LEFT JOIN academic_promotion_execution_receipts receipt ON receipt.item_id=item.id AND receipt.run_id=item.run_id
       WHERE item.status='executed' AND receipt.item_id IS NULL)
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_PROMOTION_ITEM_INTENT_UNMAPPABLE'; END IF;
END $$;
CREATE FUNCTION pg_temp.academic147_canonical_promotion(document jsonb,owner uuid) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE result jsonb; member record; expected_date date; matching boolean;
BEGIN
    IF jsonb_typeof(document)='object' THEN
        result:='{}'::jsonb;
        FOR member IN SELECT * FROM jsonb_each(document) LOOP
            result:=result||jsonb_build_object(member.key,pg_temp.academic147_canonical_promotion(member.value,owner));
        END LOOP;
        IF document ?& ARRAY['id','sourceYearId','targetYearId','policyId','status','createdBy'] AND NOT document ? 'runId' THEN
            SELECT expected.target_placement_starts_on,run.source_year_id::text=document->>'sourceYearId' AND run.target_year_id::text=document->>'targetYearId' AND run.policy_id::text=document->>'policyId'
              INTO expected_date,matching FROM academic147_expected_promotion_dates expected JOIN academic_promotion_runs run ON run.id=expected.id
              WHERE expected.id=owner AND owner::text=document->>'id';
            IF matching IS DISTINCT FROM true THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_PROMOTION_RUN_SNAPSHOT_UNMAPPABLE'; END IF;
            result:=result||jsonb_build_object('targetPlacementStartsOn',expected_date);
        ELSIF document ?& ARRAY['id','runId','studentAcademicYearId','recommendation','decision'] THEN
            SELECT CASE WHEN document->'decision' IS NOT DISTINCT FROM item.decision AND document->>'status'<>'calculated' THEN expected.target_placement_starts_on END,
                   item.run_id=owner AND item.student_academic_year_id::text=document->>'studentAcademicYearId'
              INTO expected_date,matching FROM academic147_expected_promotion_item_dates expected JOIN academic_promotion_run_items item ON item.id=expected.id
              WHERE expected.id::text=document->>'id' AND document->>'runId'=owner::text;
            IF matching IS DISTINCT FROM true THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_PROMOTION_ITEM_SNAPSHOT_UNMAPPABLE'; END IF;
            result:=result||jsonb_build_object('targetPlacementStartsOn',expected_date);
        END IF;RETURN result;
    ELSIF jsonb_typeof(document)='array' THEN
        SELECT COALESCE(jsonb_agg(pg_temp.academic147_canonical_promotion(value,owner) ORDER BY position),'[]'::jsonb) INTO result FROM jsonb_array_elements(document) WITH ORDINALITY items(value,position);
        RETURN result;
    END IF;RETURN document;
END $$;
CREATE TEMP TABLE academic147_expected_promotion_commands ON COMMIT DROP AS
SELECT request_id,pg_temp.academic147_canonical_promotion(outcome,run_id) AS outcome FROM academic_promotion_run_commands;
CREATE TEMP TABLE academic147_expected_promotion_approvals ON COMMIT DROP AS
SELECT id,pg_temp.academic147_canonical_promotion(intent,run_id) AS intent FROM academic_promotion_run_approvals;

-- Created availability has an actual date; retained-only results have no new
-- availability intent. Prove the created subset with IDs, counts and the exact
-- transaction-default timestamp, never a best-effort date/timestamp heuristic.
CREATE TEMP TABLE academic147_expected_apply_dates ON COMMIT DROP AS
SELECT ledger.idempotency_key,
       CASE WHEN ledger.created_offering_count>0 THEN created.starts_on END AS starts_on,
       ledger.created_offering_count,created.created_count,created.date_count,
       (SELECT count(*) FROM learning_offerings offering WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at) AS all_created_count,
       (SELECT count(*) FROM unnest(ledger.offering_ids) output(id) LEFT JOIN learning_offerings offering ON offering.id=output.id AND offering.academic_term_id=ledger.academic_term_id WHERE offering.id IS NULL) AS invalid_output_count,
       cardinality(ledger.offering_ids)=(SELECT count(DISTINCT output.id) FROM unnest(ledger.offering_ids) output(id)) AS unique_outputs
FROM learning_delivery_apply_runs ledger
CROSS JOIN LATERAL(
    SELECT count(*) AS created_count,count(DISTINCT offering.starts_on) AS date_count,min(offering.starts_on) AS starts_on
    FROM unnest(ledger.offering_ids) output(id) JOIN learning_offerings offering ON offering.id=output.id
    WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at
) created;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic147_expected_apply_dates WHERE created_count<>created_offering_count OR all_created_count<>created_count OR invalid_output_count<>0 OR NOT unique_outputs OR (created_count>0 AND date_count<>1))
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_CURRICULUM_CREATED_INTENT_UNMAPPABLE'; END IF;
END $$;
CREATE TEMP TABLE academic147_preparation_delivery_outputs ON COMMIT DROP AS
SELECT ledger.id,ledger.target_academic_term_id,ledger.created_at,
       (module->>'createdCount')::bigint AS created_count,
       ARRAY(SELECT value::uuid FROM jsonb_array_elements_text(module->'targetIds')) AS target_ids
FROM academic_term_preparation_runs ledger CROSS JOIN LATERAL jsonb_array_elements(ledger.outcome) module
WHERE module->>'module'='delivery';
CREATE TEMP TABLE academic147_preparation_created_proof ON COMMIT DROP AS
SELECT output.id,created.offerings,created.groups,created.revisions,created.date_count,created.starts_on,
       output.created_count,
       (SELECT count(*) FROM learning_offerings offering WHERE offering.academic_term_id=output.target_academic_term_id AND offering.created_at=output.created_at) AS all_created_offerings,
       (SELECT count(*) FROM unnest(output.target_ids) candidate(id) WHERE
         (SELECT count(*) FROM (
             SELECT offering.id FROM learning_offerings offering WHERE offering.id=candidate.id AND offering.academic_term_id=output.target_academic_term_id
             UNION ALL SELECT learning_group.id FROM learning_groups learning_group WHERE learning_group.id=candidate.id AND learning_group.academic_term_id=output.target_academic_term_id
             UNION ALL SELECT revision.id FROM academic_term_change_sets revision WHERE revision.id=candidate.id AND revision.academic_term_id=output.target_academic_term_id
         ) matching)<>1) AS invalid_output_count,
       cardinality(output.target_ids)=(SELECT count(DISTINCT candidate.id) FROM unnest(output.target_ids) candidate(id)) AS unique_outputs
FROM academic147_preparation_delivery_outputs output
CROSS JOIN LATERAL(
    SELECT count(DISTINCT offering.id) FILTER(WHERE offering.created_at=output.created_at) AS offerings,
           count(DISTINCT learning_group.id) FILTER(WHERE learning_group.created_at=output.created_at) AS groups,
           count(DISTINCT revision.id) FILTER(WHERE revision.created_at=output.created_at) AS revisions,
           count(DISTINCT offering.starts_on) FILTER(WHERE offering.created_at=output.created_at) AS date_count,
           min(offering.starts_on) FILTER(WHERE offering.created_at=output.created_at) AS starts_on
    FROM unnest(output.target_ids) candidate(id)
    LEFT JOIN learning_offerings offering ON offering.id=candidate.id AND offering.academic_term_id=output.target_academic_term_id
    LEFT JOIN learning_groups learning_group ON learning_group.id=candidate.id AND learning_group.academic_term_id=output.target_academic_term_id
    LEFT JOIN academic_term_change_sets revision ON revision.id=candidate.id AND revision.academic_term_id=output.target_academic_term_id
) created;
DO $$ BEGIN
    IF EXISTS(SELECT ledger.id FROM academic_term_preparation_runs ledger LEFT JOIN academic147_preparation_delivery_outputs output ON output.id=ledger.id
        GROUP BY ledger.id,ledger.selected_modules HAVING count(output.id)<>CASE WHEN 'delivery'=ANY(ledger.selected_modules) THEN 1 ELSE 0 END)
       OR EXISTS(SELECT 1 FROM academic147_preparation_created_proof WHERE
         offerings+groups+revisions<>created_count OR revisions<>1 OR offerings<>all_created_offerings
         OR invalid_output_count<>0 OR NOT unique_outputs OR (offerings>0 AND date_count<>1))
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_PREPARATION_CREATED_INTENT_UNMAPPABLE'; END IF;
END $$;
CREATE TEMP TABLE academic147_expected_preparation_dates ON COMMIT DROP AS
SELECT ledger.id,CASE WHEN proof.offerings>0 THEN proof.starts_on END AS offering_starts_on
FROM academic_term_preparation_runs ledger LEFT JOIN academic147_preparation_created_proof proof ON proof.id=ledger.id;

-- Snapshot retained facts for the complete dependent relation graph. Only the
-- explicitly transformed fields are normalized; evidence contains hashes, no PII.
CREATE TEMP TABLE academic147_preservation_tables(table_name text PRIMARY KEY) ON COMMIT DROP;
INSERT INTO academic147_preservation_tables
WITH RECURSIVE dependent_relations(oid) AS (
    SELECT 'academic_years'::regclass::oid UNION SELECT 'academic_terms'::regclass::oid
    UNION SELECT constraint_record.conrelid FROM pg_constraint constraint_record
          JOIN dependent_relations parent ON parent.oid=constraint_record.confrelid
          WHERE constraint_record.contype='f'
)
SELECT DISTINCT relation.relname FROM dependent_relations dependency
JOIN pg_class relation ON relation.oid=dependency.oid
JOIN pg_namespace namespace ON namespace.oid=relation.relnamespace
WHERE namespace.nspname=current_schema() AND relation.relkind IN('r','p')
  AND relation.relname<>'attendance_days';
INSERT INTO academic147_preservation_tables VALUES('calendar_events'),('behavior_settings'),('behavior_cycles'),('behavior_entries')
ON CONFLICT DO NOTHING;
CREATE TEMP TABLE academic147_before(table_name text PRIMARY KEY,row_count bigint,facts_sha256 text) ON COMMIT DROP;
CREATE FUNCTION pg_temp.academic147_snapshot()
RETURNS TABLE(table_name text,row_count bigint,facts_sha256 text) LANGUAGE plpgsql AS $$
DECLARE item record; normalized text;
BEGIN
    FOR item IN SELECT preservation.table_name FROM academic147_preservation_tables preservation ORDER BY preservation.table_name LOOP
        normalized := 'to_jsonb(fact)';
        IF item.table_name='academic_years' THEN
            normalized := normalized || ' - ARRAY[''start_date'',''end_date'']';
        ELSIF item.table_name='academic_terms' THEN
            normalized := normalized || ' - ARRAY[''start_date'',''planned_end_date'',''closed_on'']';
        ELSIF item.table_name='calendar_event_targets' THEN
            normalized := normalized || ' - ''academic_year_id''';
        ELSIF item.table_name='academic_promotion_runs' THEN
            normalized := normalized || ' - ''target_placement_starts_on''';
        ELSIF item.table_name='learning_delivery_apply_runs' THEN
            normalized := normalized || ' - ''starts_on''';
        ELSIF item.table_name='academic_term_preparation_runs' THEN
            normalized := normalized || ' - ''offering_starts_on''';
        ELSIF item.table_name='academic_promotion_run_items' THEN
            normalized := normalized || ' - ''target_placement_starts_on''';
        ELSIF item.table_name='academic_promotion_run_commands' THEN
            normalized := normalized || ' || jsonb_build_object(''outcome'',pg_temp.academic147_strip_intent_dates(fact.outcome))';
        ELSIF item.table_name='academic_promotion_run_approvals' THEN
            normalized := normalized || ' || jsonb_build_object(''intent'',pg_temp.academic147_strip_intent_dates(fact.intent))';
        ELSIF item.table_name IN('academic_term_transition_receipts','academic_year_transition_receipts') THEN
            normalized := '(' || normalized || ' - ''canonical_request'') || jsonb_build_object(''outcome'',pg_temp.academic147_strip_replay_context(fact.outcome))';
        ELSIF item.table_name='behavior_settings' THEN
            normalized := normalized || ' || jsonb_build_object(''merit_reset'',CASE WHEN fact.merit_reset IN(''term'',''year'') THEN ''scheduled'' ELSE fact.merit_reset END,''deduction_reset'',CASE WHEN fact.deduction_reset IN(''term'',''year'') THEN ''scheduled'' ELSE fact.deduction_reset END)';
        END IF;
        RETURN QUERY EXECUTE format(
            'SELECT %L::text,count(*),encode(sha256(convert_to(COALESCE(string_agg(hash,'''' ORDER BY hash),''''),''UTF8'')),''hex'') FROM (SELECT encode(sha256(convert_to((%s)::text,''UTF8'')),''hex'') AS hash FROM %I.%I fact) rows',
            item.table_name,normalized,current_schema(),item.table_name);
    END LOOP;
END $$;
INSERT INTO academic147_before SELECT * FROM pg_temp.academic147_snapshot();

ALTER TABLE calendar_event_targets DISABLE TRIGGER update_calendar_event_targets_updated_at;
UPDATE calendar_event_targets target SET academic_year_id=mapping.academic_year_id
FROM academic147_calendar_mapping mapping WHERE mapping.id=target.id;
ALTER TABLE calendar_event_targets ENABLE TRIGGER update_calendar_event_targets_updated_at;
ALTER TABLE calendar_event_targets ADD CONSTRAINT calendar_scoped_targets_explicit_year
CHECK ((grade_level_id IS NULL AND homeroom_id IS NULL) OR academic_year_id IS NOT NULL);
INSERT INTO attendance_days(academic_term_id,date,counted,note)
SELECT academic_term_id,date,counted,note FROM academic147_expected_days ON CONFLICT DO NOTHING;

ALTER TABLE learning_delivery_apply_runs ADD COLUMN starts_on date;
UPDATE learning_delivery_apply_runs ledger SET starts_on=expected.starts_on
FROM academic147_expected_apply_dates expected WHERE expected.idempotency_key=ledger.idempotency_key;
ALTER TABLE academic_term_preparation_runs ADD COLUMN offering_starts_on date;
ALTER TABLE academic_term_preparation_runs DISABLE TRIGGER academic_term_preparation_runs_immutable;
UPDATE academic_term_preparation_runs ledger SET offering_starts_on=expected.offering_starts_on
FROM academic147_expected_preparation_dates expected WHERE expected.id=ledger.id;
ALTER TABLE academic_term_preparation_runs ENABLE TRIGGER academic_term_preparation_runs_immutable;

ALTER TABLE academic_term_transition_receipts ADD COLUMN canonical_request jsonb;
ALTER TABLE academic_term_transition_receipts DISABLE TRIGGER academic_term_transition_receipts_immutable;
ALTER TABLE academic_year_transition_receipts DISABLE TRIGGER academic_year_transition_receipts_immutable;
UPDATE academic_term_transition_receipts receipt SET canonical_request=expected.canonical_request,outcome=expected.outcome
FROM academic147_expected_term_receipts expected WHERE expected.request_id=receipt.request_id;
UPDATE academic_year_transition_receipts receipt SET outcome=expected.outcome
FROM academic147_expected_year_receipts expected WHERE expected.request_id=receipt.request_id;
ALTER TABLE academic_term_transition_receipts ENABLE TRIGGER academic_term_transition_receipts_immutable;
ALTER TABLE academic_year_transition_receipts ENABLE TRIGGER academic_year_transition_receipts_immutable;
ALTER TABLE academic_term_transition_receipts ALTER COLUMN canonical_request SET NOT NULL;
ALTER TABLE academic_term_transition_receipts ADD CHECK(jsonb_typeof(canonical_request)='object');

ALTER TABLE academic_promotion_run_items ADD COLUMN target_placement_starts_on date;
ALTER TABLE academic_promotion_run_items DISABLE TRIGGER academic_promotion_run_items_identity;
UPDATE academic_promotion_run_items item SET target_placement_starts_on=expected.target_placement_starts_on
FROM academic147_expected_promotion_item_dates expected WHERE expected.id=item.id;
ALTER TABLE academic_promotion_run_items ENABLE TRIGGER academic_promotion_run_items_identity;
ALTER TABLE academic_promotion_run_commands DISABLE TRIGGER academic_promotion_run_commands_immutable;
ALTER TABLE academic_promotion_run_approvals DISABLE TRIGGER academic_promotion_run_approvals_immutable;
UPDATE academic_promotion_run_commands command SET outcome=expected.outcome
FROM academic147_expected_promotion_commands expected WHERE expected.request_id=command.request_id;
UPDATE academic_promotion_run_approvals approval SET intent=expected.intent
FROM academic147_expected_promotion_approvals expected WHERE expected.id=approval.id;
ALTER TABLE academic_promotion_run_commands ENABLE TRIGGER academic_promotion_run_commands_immutable;
ALTER TABLE academic_promotion_run_approvals ENABLE TRIGGER academic_promotion_run_approvals_immutable;

-- The run date is a default for newly prepared placements; actual accepted
-- item dates remain independent and preserve valid mixed-date historical runs.
-- Promotion owns the intended date of its planned placements, not a year range.
ALTER TABLE academic_promotion_runs ADD COLUMN target_placement_starts_on date;
UPDATE academic_promotion_runs run SET target_placement_starts_on=year.start_date
FROM academic_years year WHERE year.id=run.target_year_id;
ALTER TABLE academic_promotion_runs ALTER COLUMN target_placement_starts_on SET NOT NULL;
CREATE OR REPLACE FUNCTION academic_protect_promotion_run_identity() RETURNS trigger AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'PROMOTION_RUN_HISTORY_RETAINED' USING ERRCODE='23514'; END IF;
    IF (NEW.id,NEW.source_year_id,NEW.target_year_id,NEW.target_placement_starts_on,NEW.policy_id,NEW.request_id,NEW.request_checksum,NEW.created_by,NEW.created_at)
       IS DISTINCT FROM
       (OLD.id,OLD.source_year_id,OLD.target_year_id,OLD.target_placement_starts_on,OLD.policy_id,OLD.request_id,OLD.request_checksum,OLD.created_by,OLD.created_at) THEN
        RAISE EXCEPTION 'PROMOTION_RUN_IDENTITY_IMMUTABLE' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TABLE behavior_reset_dates(
    side text NOT NULL CHECK(side IN('merit','deduction')),
    resets_on date NOT NULL,
    PRIMARY KEY(side,resets_on)
);
CREATE TEMP TABLE academic147_expected_resets ON COMMIT DROP AS
SELECT DISTINCT policy.side,term.boundary AS resets_on FROM behavior_settings settings
CROSS JOIN LATERAL (VALUES('merit',settings.merit_reset),('deduction',settings.deduction_reset)) policy(side,schedule)
CROSS JOIN LATERAL (
    SELECT COALESCE(term.closed_on,term.planned_end_date,year.end_date) AS boundary
    FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id
    WHERE policy.schedule='term' AND term.status IN('active','closing','closed') AND year.status IN('active','closing','closed','archived')
    UNION SELECT year.end_date FROM academic_years year
    WHERE policy.schedule='year' AND year.status IN('active','closing','closed','archived')
) term;
INSERT INTO behavior_reset_dates SELECT * FROM academic147_expected_resets;
ALTER TABLE behavior_settings DROP CONSTRAINT behavior_settings_merit_reset_check,
    DROP CONSTRAINT behavior_settings_deduction_reset_check;
UPDATE behavior_settings SET
    merit_reset=CASE WHEN merit_reset IN('term','year') THEN 'scheduled' ELSE merit_reset END,
    deduction_reset=CASE WHEN deduction_reset IN('term','year') THEN 'scheduled' ELSE deduction_reset END;
ALTER TABLE behavior_settings ADD CHECK(merit_reset IN('manual','monthly','scheduled')),
    ADD CHECK(deduction_reset IN('manual','monthly','scheduled'));

DO $$ BEGIN
    IF EXISTS(SELECT id,academic_year_id FROM academic147_calendar_mapping EXCEPT SELECT target.id,target.academic_year_id FROM calendar_event_targets target JOIN academic147_calendar_mapping mapping ON mapping.id=target.id)
       OR EXISTS(SELECT target.id,target.academic_year_id FROM calendar_event_targets target JOIN academic147_calendar_mapping mapping ON mapping.id=target.id EXCEPT SELECT id,academic_year_id FROM academic147_calendar_mapping)
       OR EXISTS(SELECT * FROM academic147_expected_promotion_dates EXCEPT SELECT id,target_placement_starts_on FROM academic_promotion_runs)
       OR EXISTS(SELECT id,target_placement_starts_on FROM academic_promotion_runs EXCEPT SELECT * FROM academic147_expected_promotion_dates)
       OR EXISTS(SELECT * FROM academic147_expected_days EXCEPT SELECT academic_term_id,date,counted,note FROM attendance_days)
       OR EXISTS(SELECT * FROM academic147_existing_days EXCEPT SELECT academic_term_id,date,counted,note FROM attendance_days)
       OR EXISTS(SELECT academic_term_id,date,counted,note FROM attendance_days EXCEPT
           (SELECT * FROM academic147_expected_days UNION SELECT * FROM academic147_existing_days))
       OR EXISTS(SELECT * FROM academic147_expected_resets EXCEPT SELECT * FROM behavior_reset_dates)
       OR EXISTS(SELECT * FROM behavior_reset_dates EXCEPT SELECT * FROM academic147_expected_resets)
       OR EXISTS(
           SELECT 1 FROM academic147_existing_targets old LEFT JOIN calendar_event_targets current ON current.id=old.id
           WHERE current.id IS NULL OR (old.academic_year_id IS NOT NULL AND current.academic_year_id IS DISTINCT FROM old.academic_year_id)
       ) OR EXISTS(SELECT * FROM academic147_before EXCEPT SELECT * FROM pg_temp.academic147_snapshot())
       OR EXISTS(SELECT * FROM pg_temp.academic147_snapshot() EXCEPT SELECT * FROM academic147_before)
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_RECONCILIATION_FAILED'; END IF;
END $$;

ALTER TABLE behavior_suppressions ALTER COLUMN start_date DROP NOT NULL,ALTER COLUMN end_date DROP NOT NULL;
ALTER TABLE behavior_suppressions ADD CONSTRAINT behavior_suppression_explicit_scope CHECK(
    (start_date IS NOT NULL AND end_date IS NOT NULL) OR
    (start_date IS NULL AND end_date IS NULL AND (academic_term_id IS NOT NULL OR academic_year_id IS NOT NULL))
);

CREATE TABLE academic_context_cutover_audit(
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    retained_facts jsonb NOT NULL,
    mapped_calendar_targets bigint NOT NULL CHECK(mapped_calendar_targets>=0),
    materialized_attendance_days bigint NOT NULL CHECK(materialized_attendance_days>=0),
    scheduled_behavior_resets bigint NOT NULL CHECK(scheduled_behavior_resets>=0),
    checked_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO academic_context_cutover_audit(singleton,retained_facts,mapped_calendar_targets,materialized_attendance_days,scheduled_behavior_resets)
SELECT true,(SELECT jsonb_object_agg(table_name,jsonb_build_object('rowCount',row_count,'sha256',facts_sha256)) FROM academic147_before),
       (SELECT count(*) FROM academic147_calendar_mapping),(SELECT count(*) FROM academic147_expected_days),(SELECT count(*) FROM academic147_expected_resets);

DROP TRIGGER academic_terms_within_year ON academic_terms;
DROP FUNCTION academic_assert_term_within_year();
DROP TRIGGER learning_offerings_availability_guard ON learning_offerings;
DROP FUNCTION academic_validate_offering_availability();

CREATE OR REPLACE VIEW academic_exam_eligible_sources AS
WITH ready_plan AS (
    SELECT plan.id,
           plan.learning_offering_id,
           plan.academic_term_id,
           plan.academic_year_id,
           plan.subject_version_id
    FROM course_assessment_plans plan
    JOIN course_offering_details detail
      ON detail.learning_offering_id = plan.learning_offering_id
     AND detail.subject_version_id = plan.subject_version_id
     AND detail.academic_term_id = plan.academic_term_id
     AND detail.academic_year_id = plan.academic_year_id
    JOIN academic_terms term ON term.id = plan.academic_term_id
    JOIN course_assessment_phases phase ON phase.plan_id = plan.id
    WHERE plan.assessment_coordinator_id IS NOT NULL
      AND EXISTS (
          SELECT 1
          FROM learning_groups coordinator_group
          JOIN learning_group_teachers coordinator_teacher
            ON coordinator_teacher.learning_group_id = coordinator_group.id
           AND coordinator_teacher.teacher_id = plan.assessment_coordinator_id
           AND coordinator_teacher.starts_on <= (now() AT TIME ZONE 'Asia/Bangkok')::date
           AND (
               coordinator_teacher.ends_on IS NULL
               OR coordinator_teacher.ends_on >= (now() AT TIME ZONE 'Asia/Bangkok')::date
           )
          JOIN users coordinator
            ON coordinator.id = coordinator_teacher.teacher_id
           AND coordinator.status = 'active'
          WHERE coordinator_group.learning_offering_id = plan.learning_offering_id
            AND coordinator_group.status <> 'closed'
      )
    GROUP BY plan.id, detail.assessment_total_score
    HAVING count(phase.id) = 4
       AND count(DISTINCT phase.phase_code) = 4
       AND sum(phase.max_score) =
           detail.assessment_total_score
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'exam-source:' || phase.id::text || ':' || learning_group.id::text
               || ':' || homeroom.id::text
       ) AS source_id,
       ready_plan.academic_term_id,
       ready_plan.academic_year_id,
       phase.id AS assessment_phase_id,
       ready_plan.id AS course_assessment_plan_id,
       ready_plan.learning_offering_id,
       learning_group.id AS learning_group_id,
       homeroom.id AS homeroom_id,
       version.subject_id,
       homeroom.grade_level_id,
       phase.phase_code AS exam_kind,
       phase.exam_duration_minutes AS duration_minutes
FROM ready_plan
JOIN course_assessment_phases phase ON phase.plan_id = ready_plan.id
JOIN subject_versions version ON version.id = ready_plan.subject_version_id
JOIN learning_groups learning_group
  ON learning_group.learning_offering_id = ready_plan.learning_offering_id
 AND learning_group.academic_term_id = ready_plan.academic_term_id
 AND learning_group.academic_year_id = ready_plan.academic_year_id
 AND learning_group.status <> 'closed'
JOIN learning_group_homerooms coverage
  ON coverage.learning_group_id = learning_group.id
JOIN homerooms homeroom
  ON homeroom.id = coverage.homeroom_id
 AND homeroom.academic_year_id = ready_plan.academic_year_id
 AND homeroom.is_active
WHERE phase.phase_code IN ('midterm', 'final')
  AND phase.exam_arrangement = 'in_timetable'
  AND phase.exam_duration_minutes IS NOT NULL
  AND phase.exam_duration_minutes > 0;

ALTER TABLE academic_years DROP COLUMN start_date,DROP COLUMN end_date;
ALTER TABLE academic_terms DROP COLUMN start_date,DROP COLUMN planned_end_date,DROP COLUMN closed_on;

DO $$ BEGIN
    IF EXISTS(SELECT * FROM academic147_expected_promotion_dates EXCEPT SELECT id,target_placement_starts_on FROM academic_promotion_runs)
       OR EXISTS(SELECT id,target_placement_starts_on FROM academic_promotion_runs EXCEPT SELECT * FROM academic147_expected_promotion_dates)
       OR EXISTS(SELECT id,academic_year_id FROM academic147_calendar_mapping EXCEPT SELECT target.id,target.academic_year_id FROM calendar_event_targets target JOIN academic147_calendar_mapping mapping ON mapping.id=target.id)
       OR EXISTS(SELECT idempotency_key,starts_on FROM learning_delivery_apply_runs EXCEPT SELECT idempotency_key,starts_on FROM academic147_expected_apply_dates)
       OR EXISTS(SELECT idempotency_key,starts_on FROM academic147_expected_apply_dates EXCEPT SELECT idempotency_key,starts_on FROM learning_delivery_apply_runs)
       OR EXISTS(SELECT id,offering_starts_on FROM academic_term_preparation_runs EXCEPT SELECT * FROM academic147_expected_preparation_dates)
       OR EXISTS(SELECT * FROM academic147_expected_preparation_dates EXCEPT SELECT id,offering_starts_on FROM academic_term_preparation_runs)
       OR EXISTS(SELECT request_id,canonical_request,outcome FROM academic_term_transition_receipts EXCEPT SELECT * FROM academic147_expected_term_receipts)
       OR EXISTS(SELECT * FROM academic147_expected_term_receipts EXCEPT SELECT request_id,canonical_request,outcome FROM academic_term_transition_receipts)
       OR EXISTS(SELECT request_id,outcome FROM academic_year_transition_receipts EXCEPT SELECT * FROM academic147_expected_year_receipts)
       OR EXISTS(SELECT * FROM academic147_expected_year_receipts EXCEPT SELECT request_id,outcome FROM academic_year_transition_receipts)
       OR EXISTS(SELECT id,run_id,target_placement_starts_on FROM academic_promotion_run_items EXCEPT SELECT * FROM academic147_expected_promotion_item_dates)
       OR EXISTS(SELECT * FROM academic147_expected_promotion_item_dates EXCEPT SELECT id,run_id,target_placement_starts_on FROM academic_promotion_run_items)
       OR EXISTS(SELECT request_id,outcome FROM academic_promotion_run_commands EXCEPT SELECT * FROM academic147_expected_promotion_commands)
       OR EXISTS(SELECT * FROM academic147_expected_promotion_commands EXCEPT SELECT request_id,outcome FROM academic_promotion_run_commands)
       OR EXISTS(SELECT id,intent FROM academic_promotion_run_approvals EXCEPT SELECT * FROM academic147_expected_promotion_approvals)
       OR EXISTS(SELECT * FROM academic147_expected_promotion_approvals EXCEPT SELECT id,intent FROM academic_promotion_run_approvals)
       OR EXISTS(SELECT * FROM academic147_before EXCEPT SELECT * FROM pg_temp.academic147_snapshot())
       OR EXISTS(SELECT * FROM pg_temp.academic147_snapshot() EXCEPT SELECT * FROM academic147_before) THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_POST_CLEANUP_RECONCILIATION_FAILED';
    END IF;
END $$;
CREATE TRIGGER academic_context_cutover_audit_immutable
BEFORE UPDATE OR DELETE ON academic_context_cutover_audit
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
