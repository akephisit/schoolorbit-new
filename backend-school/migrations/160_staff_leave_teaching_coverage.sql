-- Additive dated teaching duties. Existing leave documents, hashes and evidence are untouched.
CREATE TABLE staff_leave_coverage_settings (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 revision bigint NOT NULL CHECK(revision > 0), half_day_boundary time
);
INSERT INTO staff_leave_coverage_settings(singleton,revision) VALUES(true,1);
CREATE TABLE staff_leave_coverage_plans (
 request_id uuid PRIMARY KEY REFERENCES staff_leave_requests(id),
 revision bigint NOT NULL CHECK(revision > 0),
 academic_year_id uuid NOT NULL REFERENCES academic_years(id),
 academic_term_id uuid NOT NULL REFERENCES academic_terms(id),
 document jsonb NOT NULL, plan_hash text NOT NULL,
 FOREIGN KEY(academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id),
 emergency boolean NOT NULL DEFAULT false
);
CREATE TABLE staff_leave_coverage_plan_revisions (
 request_id uuid NOT NULL REFERENCES staff_leave_requests(id),
 revision bigint NOT NULL CHECK(revision > 0), document jsonb NOT NULL,
 plan_hash text NOT NULL, PRIMARY KEY(request_id,revision),
 UNIQUE(request_id,revision,plan_hash)
);
CREATE TRIGGER immutable_leave_coverage_plan BEFORE UPDATE OR DELETE ON staff_leave_coverage_plan_revisions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TABLE staff_leave_coverage_assignment_revisions (
 request_id uuid NOT NULL, plan_revision bigint NOT NULL, assignment_id uuid NOT NULL,
 teacher_id uuid NOT NULL REFERENCES users(id), assignment_hash text NOT NULL, document jsonb NOT NULL,
 PRIMARY KEY(request_id,plan_revision,assignment_id),
 UNIQUE(request_id,plan_revision,assignment_id,teacher_id,assignment_hash),
 FOREIGN KEY(request_id,plan_revision) REFERENCES staff_leave_coverage_plan_revisions(request_id,revision)
);
CREATE TRIGGER immutable_leave_coverage_assignment BEFORE UPDATE OR DELETE ON staff_leave_coverage_assignment_revisions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TABLE staff_leave_coverage_confirmations (
 id uuid PRIMARY KEY, request_id uuid NOT NULL, plan_revision bigint NOT NULL,
 plan_hash text NOT NULL, assignment_id uuid NOT NULL, teacher_id uuid NOT NULL REFERENCES users(id), assignment_hash text NOT NULL,
 decision text NOT NULL CHECK(decision IN ('confirmed','declined')),
 accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(request_id,plan_revision,plan_hash) REFERENCES staff_leave_coverage_plan_revisions(request_id,revision,plan_hash),
 FOREIGN KEY(request_id,plan_revision,assignment_id,teacher_id,assignment_hash) REFERENCES staff_leave_coverage_assignment_revisions(request_id,plan_revision,assignment_id,teacher_id,assignment_hash),
 UNIQUE(request_id,plan_revision,assignment_id,teacher_id)
);
CREATE TRIGGER immutable_leave_coverage_confirmation BEFORE UPDATE OR DELETE ON staff_leave_coverage_confirmations FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TABLE academic_teaching_coverage_duties (
 id uuid PRIMARY KEY, request_id uuid NOT NULL REFERENCES staff_leave_requests(id),
 plan_revision bigint NOT NULL CHECK(plan_revision > 0),
 academic_year_id uuid NOT NULL REFERENCES academic_years(id),
 academic_term_id uuid NOT NULL REFERENCES academic_terms(id),
 occurrence_date date NOT NULL, start_time time NOT NULL, end_time time NOT NULL,
 teacher_id uuid NOT NULL REFERENCES users(id), original_teacher_id uuid NOT NULL REFERENCES users(id),
 source_block_id uuid, source_allocation_id uuid,
 learning_group_id uuid, room_id uuid,
 kind text NOT NULL CHECK(kind IN ('absence','substitution','reciprocal')),
 pair_id uuid, pair_leg smallint CHECK(pair_leg IN (1,2)),
 state text NOT NULL CHECK(state IN ('reserved','approved','released')),
 source_snapshot jsonb, teaching_content jsonb, source_pupils uuid[] NOT NULL DEFAULT '{}', taught_offering_id uuid, taught_learning_group_id uuid, taught_activity text,
 CHECK(start_time < end_time),
 CHECK((kind='absence' AND source_allocation_id IS NULL AND source_snapshot IS NULL) OR
       (kind<>'absence' AND source_allocation_id IS NOT NULL AND source_snapshot IS NOT NULL)),
 CHECK((kind='reciprocal' AND pair_id IS NOT NULL AND pair_leg IS NOT NULL) OR
       (kind<>'reciprocal' AND pair_id IS NULL AND pair_leg IS NULL)),
 CHECK(taught_offering_id IS NULL OR taught_activity IS NULL),
 CHECK((kind='absence' AND teaching_content IS NULL) OR (kind<>'absence' AND teaching_content IS NOT NULL)),
 FOREIGN KEY(request_id,plan_revision) REFERENCES staff_leave_coverage_plan_revisions(request_id,revision),
 FOREIGN KEY(academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id),
 UNIQUE(pair_id,pair_leg)
);
CREATE UNIQUE INDEX academic_coverage_live_source ON academic_teaching_coverage_duties(academic_term_id,occurrence_date,source_allocation_id,original_teacher_id) WHERE state<>'released' AND kind<>'absence';
CREATE INDEX academic_coverage_teacher_date ON academic_teaching_coverage_duties(teacher_id,occurrence_date,state);
CREATE INDEX academic_coverage_request ON academic_teaching_coverage_duties(request_id,state);
-- Serialize even direct writers by the same global teacher/date key used by the provider.
CREATE FUNCTION guard_dated_teaching_overlap160() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.state='released' THEN RETURN NEW; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('teaching-duty:'||NEW.teacher_id::text||':'||NEW.occurrence_date::text,0));
 IF EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE d.id<>NEW.id AND d.state<>'released' AND d.teacher_id=NEW.teacher_id AND d.occurrence_date=NEW.occurrence_date AND d.start_time<NEW.end_time AND NEW.start_time<d.end_time AND d.request_id<>NEW.request_id) THEN
  RAISE EXCEPTION 'TEACHING_COVERAGE_OVERLAP' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER guard_dated_teaching_overlap BEFORE INSERT OR UPDATE ON academic_teaching_coverage_duties FOR EACH ROW EXECUTE FUNCTION guard_dated_teaching_overlap160();
-- Scope protection to captured sources, exact allocations and actual new teacher/time
-- collisions. An unrelated lesson remains editable while duties are reserved.
CREATE FUNCTION guard_teaching_coverage_source160() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE target_id uuid; target_block uuid; source_used boolean:=false; lock_row record;
BEGIN
 IF TG_TABLE_NAME<>'academic_timetable_block_group_instructors' THEN
  IF TG_OP='DELETE' THEN target_id:=OLD.id; ELSE target_id:=NEW.id; END IF;
 END IF;
 IF TG_TABLE_NAME='academic_timetable_block_group_instructors' THEN
  IF TG_OP='DELETE' THEN target_id:=OLD.block_group_id; ELSE target_id:=NEW.block_group_id; END IF;
  SELECT block_id INTO target_block FROM academic_timetable_block_groups WHERE id=target_id;
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE d.source_allocation_id=target_id AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 ELSIF TG_TABLE_NAME='academic_timetable_blocks' THEN
  target_block:=target_id;
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE d.source_block_id=target_id AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 ELSIF TG_TABLE_NAME IN ('academic_timetable_block_groups','academic_timetable_block_teachers') THEN
  IF TG_OP='DELETE' THEN target_block:=OLD.block_id; ELSE target_block:=NEW.block_id; END IF;
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE d.source_allocation_id=target_id AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 ELSIF TG_TABLE_NAME='bell_schedule_periods' THEN
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE d.source_snapshot->>'bellSchedulePeriodId'=target_id::text AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 ELSIF TG_TABLE_NAME='learning_groups' THEN
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE (d.learning_group_id=target_id OR d.taught_learning_group_id=target_id) AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 ELSIF TG_TABLE_NAME IN ('learning_group_teachers','learning_group_students') THEN
  IF TG_OP='DELETE' THEN target_id:=OLD.learning_group_id; ELSE target_id:=NEW.learning_group_id; END IF;
  source_used:=EXISTS(SELECT 1 FROM academic_teaching_coverage_duties d WHERE (d.learning_group_id=target_id OR d.taught_learning_group_id=target_id) AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date);
 END IF;
 IF target_block IS NOT NULL THEN
  FOR lock_row IN SELECT DISTINCT d.teacher_id,d.occurrence_date FROM academic_teaching_coverage_duties d
   WHERE d.state<>'released' AND d.teacher_id IN (
    SELECT i.instructor_id FROM academic_timetable_block_groups g JOIN academic_timetable_block_group_instructors i ON i.block_group_id=g.id WHERE g.block_id=target_block AND g.is_active
    UNION SELECT t.teacher_id FROM academic_timetable_block_teachers t WHERE t.block_id=target_block AND t.is_active
   ) ORDER BY d.teacher_id,d.occurrence_date
  LOOP PERFORM pg_advisory_xact_lock(hashtextextended('teaching-duty:'||lock_row.teacher_id::text||':'||lock_row.occurrence_date::text,0)); END LOOP;
 END IF;
 IF source_used AND TG_OP<>'INSERT' THEN RAISE EXCEPTION 'TEACHING_COVERAGE_SOURCE_RESERVED' USING ERRCODE='23514'; END IF;
 IF source_used AND TG_TABLE_NAME IN ('academic_timetable_block_group_instructors','learning_group_teachers','learning_group_students') THEN RAISE EXCEPTION 'TEACHING_COVERAGE_SOURCE_RESERVED' USING ERRCODE='23514'; END IF;
 IF target_block IS NOT NULL AND TG_OP<>'DELETE' AND EXISTS(
  WITH occupied AS (
   SELECT b.id,b.academic_term_id,g.id allocation_id,i.instructor_id teacher_id,b.day_of_week,p.start_time,p.end_time
   FROM academic_timetable_blocks b JOIN bell_schedule_periods p ON p.id=b.bell_schedule_period_id
   JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active
   JOIN academic_timetable_block_group_instructors i ON i.block_group_id=g.id
   WHERE b.id=target_block AND b.is_active
   UNION ALL
   SELECT b.id,b.academic_term_id,t.id,t.teacher_id,b.day_of_week,p.start_time,p.end_time
   FROM academic_timetable_blocks b JOIN bell_schedule_periods p ON p.id=b.bell_schedule_period_id
   JOIN academic_timetable_block_teachers t ON t.block_id=b.id AND t.is_active
   WHERE b.id=target_block AND b.is_active
  ) SELECT 1 FROM occupied o JOIN academic_teaching_coverage_duties d ON d.teacher_id=o.teacher_id AND d.academic_term_id=o.academic_term_id
  AND d.state<>'released' AND d.occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date
  AND o.day_of_week=(ARRAY['MON','TUE','WED','THU','FRI','SAT','SUN'])[extract(isodow FROM d.occurrence_date)::int]
  AND o.start_time<d.end_time AND d.start_time<o.end_time
  WHERE NOT EXISTS(SELECT 1 FROM academic_teaching_coverage_duties captured WHERE captured.request_id=d.request_id AND captured.source_allocation_id=o.allocation_id AND captured.original_teacher_id=o.teacher_id AND captured.occurrence_date=d.occurrence_date AND captured.state<>'released')
 ) THEN RAISE EXCEPTION 'TEACHING_COVERAGE_TEACHER_RESERVED' USING ERRCODE='23514'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF; RETURN NEW;
END $$;
CREATE TRIGGER guard_coverage_weekly_blocks AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_blocks FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_weekly_groups AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_groups FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_weekly_instructors AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_group_instructors FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_weekly_teachers AFTER INSERT OR UPDATE OR DELETE ON academic_timetable_block_teachers FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_delivery_groups AFTER UPDATE OR DELETE ON learning_groups FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_delivery_teachers AFTER INSERT OR UPDATE OR DELETE ON learning_group_teachers FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_delivery_pupils AFTER INSERT OR UPDATE OR DELETE ON learning_group_students FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE TRIGGER guard_coverage_bell_periods AFTER UPDATE OR DELETE ON bell_schedule_periods FOR EACH ROW EXECUTE FUNCTION guard_teaching_coverage_source160();
CREATE FUNCTION guard_coverage_pair160() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.pair_id IS NOT NULL AND NOT EXISTS(
  SELECT 1 FROM academic_teaching_coverage_duties a JOIN academic_teaching_coverage_duties b ON b.pair_id=a.pair_id AND b.pair_leg=2
  WHERE a.pair_id=NEW.pair_id AND a.request_id=NEW.request_id AND a.plan_revision=NEW.plan_revision AND a.pair_leg=1 AND a.request_id=b.request_id AND a.plan_revision=b.plan_revision
  AND a.teacher_id=b.original_teacher_id AND b.teacher_id=a.original_teacher_id
  AND (a.occurrence_date,a.source_allocation_id)<>(b.occurrence_date,b.source_allocation_id)
 ) THEN RAISE EXCEPTION 'TEACHING_COVERAGE_INCOMPLETE_PAIR' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER guard_coverage_pair AFTER INSERT OR UPDATE ON academic_teaching_coverage_duties DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION guard_coverage_pair160();

CREATE FUNCTION preserve_dated_teaching_evidence160() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR (to_jsonb(NEW)-'state') IS DISTINCT FROM (to_jsonb(OLD)-'state') THEN
  RAISE EXCEPTION 'TEACHING_COVERAGE_EVIDENCE_IMMUTABLE' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER preserve_dated_teaching_evidence BEFORE UPDATE OR DELETE ON academic_teaching_coverage_duties FOR EACH ROW EXECUTE FUNCTION preserve_dated_teaching_evidence160();
