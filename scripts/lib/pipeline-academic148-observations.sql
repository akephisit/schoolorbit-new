-- Fixed observation only. $1 is the trusted compiled 001..148 SHA-384 manifest.
-- No report is returned for unsupported history, exceeded budgets or a writable snapshot.
WITH
expected AS MATERIALIZED (
    SELECT CASE WHEN entry->>'version' ~ '^[0-9]{1,3}$'
                     AND jsonb_typeof(entry->'version')='number'
                THEN (entry->>'version')::bigint END AS version,
           entry->>'checksum' AS checksum,
           CASE WHEN jsonb_typeof(entry)='object'
                THEN entry ?& ARRAY['version','checksum']
                     AND jsonb_typeof(entry->'checksum')='string'
                     AND entry-ARRAY['version','checksum']='{}'::jsonb
                ELSE false END AS valid_shape
    FROM jsonb_array_elements(CASE WHEN jsonb_typeof($1::jsonb)='array'
                                  THEN $1::jsonb ELSE '[]'::jsonb END) entry
),
history AS MATERIALIZED (
    SELECT version,checksum,success,installed_on FROM _sqlx_migrations LIMIT 149
),
history_state AS (
    SELECT max(version) AS version,
           count(*)=max(version) AND min(version)=1 AND max(version) IN (147,148)
           AND bool_and(success)
           AND NOT EXISTS (
               SELECT 1 FROM history actual LEFT JOIN expected USING(version)
               WHERE expected.version IS NULL OR encode(actual.checksum,'hex') IS DISTINCT FROM expected.checksum
           ) AS verified
    FROM history
),
receipt_sample AS MATERIALIZED (
    SELECT ledger.* FROM learning_delivery_apply_runs ledger LIMIT 100001
),
offering_sample AS MATERIALIZED (
    SELECT id,academic_term_id,created_at FROM learning_offerings LIMIT 100001
),
eligible AS MATERIALIZED (
    SELECT history_state.version
    FROM history_state
    WHERE history_state.verified
      AND current_setting('transaction_read_only')='on'
      AND current_setting('transaction_isolation')='repeatable read'
      AND (SELECT count(*)=148 AND count(DISTINCT version)=148
                  AND min(version)=1 AND max(version)=148
                  AND bool_and(valid_shape AND checksum IS NOT NULL AND checksum ~ '^[0-9a-f]{96}$')
           FROM expected)
      AND (SELECT count(*) FROM receipt_sample)<=100000
      AND (SELECT count(*) FROM offering_sample)<=100000
      AND (SELECT COALESCE(sum(cardinality(offering_ids)),0) FROM receipt_sample)<=1000000
),
receipts AS MATERIALIZED (
    SELECT sample.* FROM eligible CROSS JOIN receipt_sample sample
),
offerings AS MATERIALIZED (
    SELECT sample.* FROM eligible CROSS JOIN offering_sample sample
),
offering_cohorts AS (
    SELECT academic_term_id,created_at,count(*) AS count
    FROM offerings GROUP BY academic_term_id,created_at
),
receipt_cohorts AS (
    SELECT academic_term_id,created_at,count(*) AS count
    FROM receipts GROUP BY academic_term_id,created_at HAVING count(*)>1
),
outputs AS (
    -- Preserve 148's multiplicity: duplicate IDs count repeatedly as created outputs.
    SELECT ledger.idempotency_key,
           count(*) FILTER(WHERE offering.academic_term_id=ledger.academic_term_id
                             AND offering.created_at=ledger.created_at) AS created_count,
           count(DISTINCT offering.id) FILTER(WHERE offering.academic_term_id=ledger.academic_term_id
                                               AND offering.created_at=ledger.created_at) AS distinct_created_count,
           count(*) FILTER(WHERE output.id IS NOT NULL AND offering.id IS NULL) AS missing_count,
           count(*) FILTER(WHERE offering.id IS NOT NULL
                             AND offering.academic_term_id<>ledger.academic_term_id) AS wrong_context_count,
           count(*) FILTER(WHERE output.id IS NULL) AS null_count,
           count(output.id)-count(DISTINCT output.id) AS duplicate_count,
           count(DISTINCT output.id) AS unique_count
    FROM receipts ledger CROSS JOIN LATERAL unnest(ledger.offering_ids) output(id)
    LEFT JOIN offerings offering ON offering.id=output.id
    GROUP BY ledger.idempotency_key
),
facts AS (
    SELECT ledger.idempotency_key,ledger.created_at,ledger.created_offering_count,
           COALESCE(output.created_count,0) AS created_count,
           COALESCE(cohort.count,0) AS all_created_count,
           COALESCE(output.missing_count,0) AS missing_count,
           COALESCE(output.wrong_context_count,0) AS wrong_context_count,
           COALESCE(output.null_count,0) AS null_count,
           COALESCE(output.duplicate_count,0) AS duplicate_count,
           COALESCE(output.unique_count,0) AS distinct_output_count,
           COALESCE(cohort.count,0)-COALESCE(output.distinct_created_count,0) AS extra_count,
           cardinality(ledger.offering_ids)<>COALESCE(output.unique_count,0) AS nonunique,
           CASE WHEN installed.installed_on IS NULL OR ledger.created_at IS NULL THEN 'unknown050'
                WHEN ledger.created_at<installed.installed_on THEN 'before050'
                WHEN ledger.created_at=installed.installed_on THEN 'equal050'
                ELSE 'after050' END AS timing
    FROM receipts ledger LEFT JOIN outputs output USING(idempotency_key)
    LEFT JOIN offering_cohorts cohort ON cohort.academic_term_id=ledger.academic_term_id
                                    AND cohort.created_at=ledger.created_at
    LEFT JOIN history installed ON installed.version=50
),
classified AS MATERIALIZED (
    SELECT facts.*,
           (CASE WHEN created_count<>created_offering_count THEN 1 ELSE 0 END)
         + (CASE WHEN all_created_count<>created_count THEN 2 ELSE 0 END)
         + (CASE WHEN missing_count+wrong_context_count+null_count>0 THEN 4 ELSE 0 END)
         + (CASE WHEN nonunique THEN 8 ELSE 0 END) AS mask
    FROM facts
),
current_receipt_references AS MATERIALIZED (
    SELECT DISTINCT ledger.academic_term_id,ledger.created_at,output.id
    FROM receipts ledger CROSS JOIN LATERAL unnest(ledger.offering_ids) output(id)
    WHERE output.id IS NOT NULL
),
lower_output_entries AS (
    SELECT ledger.idempotency_key,ledger.created_at AS receipt_at,offering.created_at AS offering_at,
           reference.id IS NOT NULL AS current_receipt,
           CASE WHEN offering.created_at>ledger.created_at THEN EXISTS (
               SELECT 1 FROM academic_audit_events event
               WHERE event.event_code='academic_term_change_item.deleted'
                 AND event.entity_type='academic_term_change_item'
                 AND event.academic_term_id=ledger.academic_term_id
                 AND event.payload->>'action'='add_offering'
                 AND event.payload->>'learningOfferingId'=offering.id::text
                 AND event.created_at BETWEEN ledger.created_at AND offering.created_at
           ) ELSE false END AS delete_item_audit
    FROM classified lower JOIN receipts ledger USING(idempotency_key)
    CROSS JOIN LATERAL unnest(ledger.offering_ids) output(id)
    JOIN offerings offering ON offering.id=output.id AND offering.academic_term_id=ledger.academic_term_id
    LEFT JOIN current_receipt_references reference
      ON reference.academic_term_id=ledger.academic_term_id
     AND reference.created_at=offering.created_at AND reference.id=offering.id
    WHERE lower.created_count<lower.created_offering_count
    -- For later outputs, a receipt at offering_at necessarily differs from this
    -- receipt at receipt_at. Distinct references avoid multiplying output entries.
),
current_checkpoint AS (
    -- 138 and 141 hash row JSON in text order, including every original field.
    -- Only verified 148 adds starts_on. At 147, unexpected fields remain observable.
    SELECT count(*) AS count,
           md5(COALESCE(string_agg(md5(facts::text),'' ORDER BY facts::text),'')) AS checksum
    FROM (SELECT CASE WHEN (SELECT version FROM eligible)=148
                      THEN to_jsonb(ledger)-'starts_on' ELSE to_jsonb(ledger) END AS facts
          FROM receipts ledger) original
),
checkpoint_events AS (
    SELECT checkpoint.version,count(event.id) AS count,(array_agg(event.payload))[1] AS payload
    FROM (VALUES(138,'catalog.start_only_reconciled'),
                (141,'catalog.grade_snapshots_reconciled')) checkpoint(version,event_code)
    LEFT JOIN LATERAL (
        -- Two events suffice to prove non-uniqueness without collecting payload history.
        SELECT id,payload FROM academic_audit_events
        WHERE event_code=checkpoint.event_code LIMIT 2
    ) event ON true
    GROUP BY checkpoint.version
),
checkpoints AS (
    SELECT event.version,event.count>0 AS present,event.count=1 AS unique,
           COALESCE(event.count=1 AND event.payload->'migration'=to_jsonb(event.version)
                    AND event.payload->'passed'='true'::jsonb
                    AND event.payload->'references'->'learning_delivery_apply_runs'->'count'=to_jsonb(current.count),false) AS count_match,
           COALESCE(event.count=1 AND event.payload->'migration'=to_jsonb(event.version)
                    AND event.payload->'passed'='true'::jsonb
                    AND event.payload->'references'->'learning_delivery_apply_runs'->'contentChecksum'=to_jsonb(current.checksum),false) AS hash_match
    FROM checkpoint_events event CROSS JOIN current_checkpoint current
)
-- History verification does not attest receipt provenance. Timing and checkpoint
-- differences are correlations; legitimate later work or FK changes can alter them.
-- Timestamp JSON formatting also depends on session TimeZone, which old checkpoints
-- did not record. Preserve their exact algorithm; do not guess a historical zone.
-- Mismatch metrics count receipts; *_entries and extra_timestamp_offerings sum entries
-- per receipt. Shared timestamp cohorts are not proof of a shared transaction ID.
-- Divergence covers only lower-match receipts. Temporal metrics count extant,
-- same-context entries with multiplicity; declared distinct IDs also include missing
-- or wrong-context IDs. Reuse/deletion are correlations, not proof of the cause.
-- Historic deletion audits were appended after commit; absence does not refute deletion.
SELECT jsonb_build_object(
    'operation','academic148_curriculum','schema_version',2,
    'history_version',eligible.version,'history_verified',true,
    'counts',(SELECT jsonb_build_object(
        'receipts',count(*),'violating_receipts',count(*) FILTER(WHERE mask<>0),
        'created_count_mismatches',count(*) FILTER(WHERE created_count<>created_offering_count),
        'matched_created_lower',count(*) FILTER(WHERE created_count<created_offering_count),
        'matched_created_higher',count(*) FILTER(WHERE created_count>created_offering_count),
        'zero_recorded_positive_created',count(*) FILTER(WHERE created_offering_count=0 AND created_count>0),
        'all_created_count_mismatches',count(*) FILTER(WHERE all_created_count<>created_count),
        'invalid_output_receipts',count(*) FILTER(WHERE missing_count+wrong_context_count+null_count>0),
        'nonunique_output_receipts',count(*) FILTER(WHERE nonunique),
        'missing_output_entries',COALESCE(sum(missing_count),0),
        'wrong_context_output_entries',COALESCE(sum(wrong_context_count),0),
        'null_output_entries',COALESCE(sum(null_count),0),
        'duplicate_nonnull_output_entries',COALESCE(sum(duplicate_count),0),
        'extra_timestamp_offerings',COALESCE(sum(extra_count),0),
        'shared_timestamp_cohorts',(SELECT count(*) FROM receipt_cohorts),
        'shared_timestamp_receipts',(SELECT COALESCE(sum(count),0) FROM receipt_cohorts)
    ) FROM classified),
    'masks',(SELECT jsonb_agg(jsonb_build_object('mask',masks.mask,'count',
                     (SELECT count(*) FROM classified WHERE classified.mask=masks.mask)) ORDER BY masks.mask)
             FROM generate_series(0,15) masks(mask)),
    'timing',(SELECT jsonb_agg(jsonb_build_object('kind',timing.kind,'count',
                      (SELECT count(*) FROM classified WHERE classified.timing=timing.kind),
                      'violating',(SELECT count(*) FROM classified WHERE classified.timing=timing.kind AND mask<>0),
                      'zero_recorded_positive_created',(SELECT count(*) FROM classified
                         WHERE classified.timing=timing.kind AND created_offering_count=0 AND created_count>0)) ORDER BY timing.position)
              FROM (VALUES(1,'before050'),(2,'equal050'),(3,'after050'),(4,'unknown050')) timing(position,kind)),
    'checkpoints',(SELECT jsonb_agg(to_jsonb(checkpoints) ORDER BY version) FROM checkpoints),
    'divergence',(SELECT jsonb_build_object(
        'lower_receipts',count(*),
        'recorded_created_total',COALESCE(sum(created_offering_count),0),
        'timestamp_created_total',COALESCE(sum(created_count),0),
        'difference_total',COALESCE(sum(created_offering_count-created_count),0),
        'distinct_output_total',COALESCE(sum(distinct_output_count),0),
        'recorded_above_output_receipts',count(*) FILTER(WHERE created_offering_count>distinct_output_count),
        'output_entries_earlier',(SELECT count(*) FROM lower_output_entries WHERE offering_at<receipt_at),
        'output_entries_equal',(SELECT count(*) FROM lower_output_entries WHERE offering_at=receipt_at),
        'output_entries_later',(SELECT count(*) FROM lower_output_entries WHERE offering_at>receipt_at),
        'later_entries_with_current_receipt',(SELECT count(*) FROM lower_output_entries WHERE offering_at>receipt_at AND current_receipt),
        'later_entries_with_delete_item_audit',(SELECT count(*) FROM lower_output_entries WHERE offering_at>receipt_at AND delete_item_audit)
    ) FROM classified WHERE created_count<created_offering_count)
)
FROM eligible;
