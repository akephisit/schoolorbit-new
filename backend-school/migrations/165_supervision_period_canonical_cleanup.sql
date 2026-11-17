-- Contract only after the complete unchanged legacy graph reconciles. Pre-165
-- binaries are incompatible; keep maintenance active and recover by rolling forward.
SET LOCAL lock_timeout = '30s';
LOCK TABLE supervision_observations, supervision_observation_periods,
    supervision_evaluators, supervision_evaluator_responses, supervision_actions,
    supervision_period_cutover_receipt IN ACCESS EXCLUSIVE MODE;
DO $$
DECLARE receipt supervision_period_cutover_receipt%ROWTYPE;
    parent_count bigint; parent_hash text; evidence_hash text; canonical_hash text; descendants_hash text;
BEGIN
    SELECT * INTO receipt FROM supervision_period_cutover_receipt WHERE singleton;
    IF NOT FOUND THEN RAISE EXCEPTION 'SUPERVISION_PERIOD165_RECEIPT_MISSING' USING ERRCODE='check_violation'; END IF;
    SELECT count(*), encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(o) - ARRAY[
        'timetable_block_group_id','learning_group_id','homeroom_id','observed_at',
        'manual_subject_name','manual_classroom_label','manual_room_label','manual_period_label','manual_reason','lesson_snapshot'
    ] ORDER BY o.id),'[]')::text,'UTF8')),'hex') INTO parent_count,parent_hash FROM supervision_observations o;
    SELECT encode(sha256(convert_to(COALESCE(jsonb_agg(jsonb_build_array(p.observation_id,p.academic_year_id,p.academic_term_id,
        p.timetable_block_group_id,p.learning_group_id,p.homeroom_id,p.observed_at,
        p.manual_subject_name,p.manual_classroom_label,p.manual_room_label,p.manual_period_label,p.manual_reason,
        p.lesson_snapshot,p.created_at,p.updated_at) ORDER BY p.observation_id),'[]')::text,'UTF8')),'hex')
        INTO evidence_hash FROM supervision_observation_periods p;
    SELECT encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.observation_id,p.position,p.id),'[]')::text,'UTF8')),'hex')
        INTO canonical_hash FROM supervision_observation_periods p;
    SELECT encode(sha256(convert_to(jsonb_build_array(
        (SELECT COALESCE(jsonb_agg(to_jsonb(e) ORDER BY e.id),'[]') FROM supervision_evaluators e),
        (SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]') FROM supervision_evaluator_responses r),
        (SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.id),'[]') FROM supervision_actions a)
    )::text,'UTF8')),'hex') INTO descendants_hash;
    IF (parent_count,parent_hash,evidence_hash,canonical_hash,descendants_hash) IS DISTINCT FROM
       (receipt.observations,receipt.parents_hash,receipt.evidence_hash,receipt.canonical_hash,receipt.descendants_hash)
       OR (SELECT count(*) FROM supervision_observation_periods)<>parent_count
       OR EXISTS(SELECT 1 FROM supervision_observations o FULL JOIN supervision_observation_periods p
          ON p.observation_id=o.id AND p.id=o.id AND p.position=1
          WHERE o.id IS NULL OR p.id IS NULL OR p.ends_at IS NOT NULL
          OR (p.timetable_block_group_id,p.learning_group_id,p.homeroom_id,p.observed_at,p.manual_subject_name,
              p.manual_classroom_label,p.manual_room_label,p.manual_period_label,p.manual_reason,p.lesson_snapshot)
          IS DISTINCT FROM (o.timetable_block_group_id,o.learning_group_id,o.homeroom_id,o.observed_at,o.manual_subject_name,
              o.manual_classroom_label,o.manual_room_label,o.manual_period_label,o.manual_reason,o.lesson_snapshot)) THEN
        RAISE EXCEPTION 'SUPERVISION_PERIOD165_RECONCILIATION_FAILED' USING ERRCODE='check_violation';
    END IF;
END $$;
-- RESTRICT deliberately refuses unknown external dependencies instead of cascading.
ALTER TABLE supervision_observations
    DROP COLUMN timetable_block_group_id RESTRICT,
    DROP COLUMN learning_group_id RESTRICT,
    DROP COLUMN homeroom_id RESTRICT,
    DROP COLUMN observed_at RESTRICT,
    DROP COLUMN manual_subject_name RESTRICT,
    DROP COLUMN manual_classroom_label RESTRICT,
    DROP COLUMN manual_room_label RESTRICT,
    DROP COLUMN manual_period_label RESTRICT,
    DROP COLUMN manual_reason RESTRICT,
    DROP COLUMN lesson_snapshot RESTRICT;
DROP TABLE supervision_period_cutover_receipt RESTRICT;
