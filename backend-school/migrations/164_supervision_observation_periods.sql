-- Expand and reconcile exact saved evidence. Old writers must be drained.
SET LOCAL lock_timeout = '30s';
LOCK TABLE supervision_observations, supervision_evaluators,
    supervision_evaluator_responses, supervision_actions IN ACCESS EXCLUSIVE MODE;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM supervision_observations o WHERE jsonb_typeof(o.lesson_snapshot)<>'object'
        OR EXISTS(SELECT 1 FROM jsonb_each(CASE WHEN jsonb_typeof(o.lesson_snapshot)='object' THEN o.lesson_snapshot ELSE '{}'::jsonb END) field WHERE field.key=ANY(ARRAY[
            'source','timetableBlockGroupId','subjectName','classroomLabel','roomLabel','observedAt','periodLabel'])
            AND jsonb_typeof(field.value) NOT IN ('string','null'))) THEN
        RAISE EXCEPTION 'SUPERVISION_PERIOD164_EVIDENCE_UNMAPPABLE' USING ERRCODE='check_violation';
    END IF;
    IF EXISTS(SELECT 1 FROM supervision_observations o
        JOIN learning_groups g ON g.id=o.learning_group_id
        JOIN academic_timetable_block_groups placed ON placed.id=o.timetable_block_group_id
        WHERE g.learning_offering_id<>placed.learning_offering_id) THEN
        RAISE EXCEPTION 'SUPERVISION_PERIOD164_SOURCE_AMBIGUOUS' USING ERRCODE='check_violation';
    END IF;
    IF EXISTS(SELECT 1 FROM supervision_observations WHERE lesson_snapshot->>'observedAt' IS NOT NULL
        AND lesson_snapshot->>'observedAt' !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}[Tt][0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?([Zz]|[+-][0-9]{2}:[0-9]{2})$') THEN
        RAISE EXCEPTION 'SUPERVISION_PERIOD164_EVIDENCE_UNMAPPABLE' USING ERRCODE='check_violation';
    END IF;
    PERFORM (lesson_snapshot->>'observedAt')::timestamptz,
        (lesson_snapshot->>'timetableBlockGroupId')::uuid FROM supervision_observations;
EXCEPTION WHEN invalid_datetime_format OR datetime_field_overflow OR invalid_text_representation THEN
    RAISE EXCEPTION 'SUPERVISION_PERIOD164_EVIDENCE_UNMAPPABLE' USING ERRCODE='check_violation';
END $$;

CREATE TABLE supervision_observation_periods (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    observation_id uuid NOT NULL,
    position integer NOT NULL CHECK (position > 0),
    academic_year_id uuid NOT NULL,
    academic_term_id uuid NOT NULL,
    timetable_block_group_id uuid,
    learning_offering_id uuid,
    learning_group_id uuid,
    homeroom_id uuid,
    observed_at timestamptz NOT NULL,
    ends_at timestamptz CHECK (ends_at > observed_at),
    manual_subject_name varchar(200),
    manual_classroom_label varchar(120),
    manual_room_label varchar(120),
    manual_period_label varchar(120),
    manual_reason text,
    lesson_snapshot jsonb NOT NULL CHECK (jsonb_typeof(lesson_snapshot)='object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (observation_id, position),
    UNIQUE (observation_id, timetable_block_group_id),
    FOREIGN KEY (timetable_block_group_id, academic_term_id, academic_year_id)
        REFERENCES academic_timetable_block_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id, academic_term_id, academic_year_id)
        REFERENCES learning_offerings(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_group_id, academic_term_id, academic_year_id)
        REFERENCES learning_groups(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (homeroom_id, academic_year_id)
        REFERENCES homerooms(id, academic_year_id) ON DELETE RESTRICT
);
ALTER TABLE supervision_observations ADD CONSTRAINT supervision_observations_period_context_key
    UNIQUE (id, academic_term_id, academic_year_id);
ALTER TABLE supervision_observation_periods ADD CONSTRAINT supervision_periods_observation_context_fkey
    FOREIGN KEY (observation_id, academic_term_id, academic_year_id)
    REFERENCES supervision_observations(id, academic_term_id, academic_year_id) ON DELETE CASCADE;

-- The old observation UUID is a deterministic evidence-period UUID. Do not guess
-- a historical bell end or normalize old timestamps/snapshots/manual values.
INSERT INTO supervision_observation_periods (
    id, observation_id, position, academic_year_id, academic_term_id,
    timetable_block_group_id, learning_offering_id, learning_group_id, homeroom_id,
    observed_at, ends_at, manual_subject_name, manual_classroom_label, manual_room_label,
    manual_period_label, manual_reason, lesson_snapshot, created_at, updated_at
)
SELECT o.id, o.id, 1, o.academic_year_id, o.academic_term_id,
    o.timetable_block_group_id, COALESCE(g.learning_offering_id,placed.learning_offering_id), o.learning_group_id, o.homeroom_id,
    o.observed_at, NULL, o.manual_subject_name, o.manual_classroom_label, o.manual_room_label,
    o.manual_period_label, o.manual_reason, o.lesson_snapshot, o.created_at, o.updated_at
FROM supervision_observations o LEFT JOIN learning_groups g ON g.id=o.learning_group_id
LEFT JOIN academic_timetable_block_groups placed ON placed.id=o.timetable_block_group_id;

CREATE TABLE supervision_period_cutover_receipt (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    observations bigint NOT NULL,
    parents_hash text NOT NULL,
    evidence_hash text NOT NULL,
    canonical_hash text NOT NULL,
    descendants_hash text NOT NULL
);
INSERT INTO supervision_period_cutover_receipt
SELECT true, count(*),
    encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(o) - ARRAY[
        'timetable_block_group_id','learning_group_id','homeroom_id','observed_at',
        'manual_subject_name','manual_classroom_label','manual_room_label','manual_period_label','manual_reason','lesson_snapshot'
    ] ORDER BY o.id),'[]')::text,'UTF8')),'hex'),
    encode(sha256(convert_to(COALESCE(jsonb_agg(jsonb_build_array(o.id,o.academic_year_id,o.academic_term_id,
        o.timetable_block_group_id,o.learning_group_id,o.homeroom_id,o.observed_at,
        o.manual_subject_name,o.manual_classroom_label,o.manual_room_label,o.manual_period_label,o.manual_reason,
        o.lesson_snapshot,o.created_at,o.updated_at) ORDER BY o.id),'[]')::text,'UTF8')),'hex'),
    (SELECT encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.observation_id,p.position,p.id),'[]')::text,'UTF8')),'hex') FROM supervision_observation_periods p),
    encode(sha256(convert_to(jsonb_build_array(
        (SELECT COALESCE(jsonb_agg(to_jsonb(e) ORDER BY e.id),'[]') FROM supervision_evaluators e),
        (SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]') FROM supervision_evaluator_responses r),
        (SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.id),'[]') FROM supervision_actions a)
    )::text,'UTF8')),'hex')
FROM supervision_observations o;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM supervision_observations o FULL JOIN supervision_observation_periods p
        ON p.observation_id=o.id AND p.id=o.id AND p.position=1
        WHERE o.id IS NULL OR p.id IS NULL OR p.ends_at IS NOT NULL
        OR (p.academic_year_id,p.academic_term_id,p.timetable_block_group_id,p.learning_group_id,p.homeroom_id,p.observed_at,
            p.manual_subject_name,p.manual_classroom_label,p.manual_room_label,p.manual_period_label,p.manual_reason,
            p.lesson_snapshot,p.created_at,p.updated_at)
        IS DISTINCT FROM (o.academic_year_id,o.academic_term_id,o.timetable_block_group_id,o.learning_group_id,o.homeroom_id,o.observed_at,
            o.manual_subject_name,o.manual_classroom_label,o.manual_room_label,o.manual_period_label,o.manual_reason,
            o.lesson_snapshot,o.created_at,o.updated_at)) THEN
        RAISE EXCEPTION 'SUPERVISION_PERIOD164_RECONCILIATION_FAILED' USING ERRCODE='check_violation';
    END IF;
END $$;
CREATE INDEX supervision_periods_source_idx ON supervision_observation_periods(timetable_block_group_id)
    WHERE timetable_block_group_id IS NOT NULL;
CREATE INDEX supervision_periods_group_idx ON supervision_observation_periods(learning_group_id)
    WHERE learning_group_id IS NOT NULL;
CREATE INDEX supervision_periods_observed_at_idx ON supervision_observation_periods(observed_at);
