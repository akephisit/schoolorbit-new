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
        AND rule.conname NOT IN('academic_years_date_order_check','academic_terms_date_order_check'))
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
       OR EXISTS(SELECT * FROM academic147_before EXCEPT SELECT * FROM pg_temp.academic147_snapshot())
       OR EXISTS(SELECT * FROM pg_temp.academic147_snapshot() EXCEPT SELECT * FROM academic147_before) THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_147_POST_CLEANUP_RECONCILIATION_FAILED';
    END IF;
END $$;
CREATE TRIGGER academic_context_cutover_audit_immutable
BEFORE UPDATE OR DELETE ON academic_context_cutover_audit
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
