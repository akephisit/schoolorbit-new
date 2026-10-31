-- A finite, transaction-local projection supersedes 148's timestamp-history
-- premise. Accepted counts remain in their original physical column throughout.
-- The canonical runner executes this fixed preparation before native 148, then
-- native 149 repeats preparation and finalizes before the outer transaction commits.
CREATE OR REPLACE FUNCTION pg_temp.academic149_receipts(reconstructed boolean, legacy boolean)
RETURNS TABLE(facts jsonb) LANGUAGE plpgsql AS $$
BEGIN
    IF reconstructed THEN
        RETURN QUERY EXECUTE format(
            'SELECT (to_jsonb(receipt)-ARRAY[''created_offering_count'',''academic149_original_created_offering_count'',''starts_on'']) || jsonb_build_object(''created_offering_count'',receipt.academic149_original_created_offering_count) FROM %I.learning_delivery_apply_runs receipt',
            current_schema());
    ELSIF legacy THEN
        RETURN QUERY EXECUTE format('SELECT to_jsonb(receipt)-''starts_on'' FROM %I.learning_delivery_apply_runs receipt',current_schema());
    ELSE
        RETURN QUERY EXECUTE format('SELECT to_jsonb(receipt) FROM %I.learning_delivery_apply_runs receipt',current_schema());
    END IF;
END $$;

-- This is exactly 148's receipt-row digest: SHA256 of sorted SHA256 row hashes.
CREATE OR REPLACE FUNCTION pg_temp.academic149_receipt_summary(reconstructed boolean, legacy boolean)
RETURNS TABLE(row_count bigint,sha256 text) LANGUAGE plpgsql AS $$
BEGIN
    RETURN QUERY SELECT count(*),encode(sha256(convert_to(
        COALESCE(string_agg(row_hash,'' ORDER BY row_hash),''),'UTF8')),'hex')
    FROM (SELECT encode(sha256(convert_to(facts::text,'UTF8')),'hex') AS row_hash
          FROM pg_temp.academic149_receipts(reconstructed,legacy)) rows;
END $$;

CREATE OR REPLACE FUNCTION pg_temp.academic149_column_evidence()
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE result jsonb;
BEGIN
    SELECT jsonb_build_object(
        'relation',attribute.attrelid,'number',attribute.attnum,
        'type',attribute.atttypid,'modifier',attribute.atttypmod,
        'notNull',attribute.attnotnull,'identity',attribute.attidentity,
        'generated',attribute.attgenerated,'default',default_record.adbin::text,
        'constraints',COALESCE((SELECT jsonb_agg(jsonb_build_object(
            'oid',rule.oid,'name',rule.conname,'kind',rule.contype,
            'keys',rule.conkey,'expression',rule.conbin::text,
            'validated',rule.convalidated,'deferrable',rule.condeferrable,
            'deferred',rule.condeferred) ORDER BY rule.oid)
            FROM pg_constraint rule WHERE rule.conrelid=attribute.attrelid
              AND attribute.attnum=ANY(rule.conkey)),'[]'::jsonb))
    INTO result
    FROM pg_attribute attribute
    LEFT JOIN pg_attrdef default_record ON default_record.adrelid=attribute.attrelid
                                      AND default_record.adnum=attribute.attnum
    WHERE attribute.attrelid='learning_delivery_apply_runs'::regclass
      AND attribute.attname=CASE WHEN EXISTS(
          SELECT 1 FROM pg_attribute original
          WHERE original.attrelid=attribute.attrelid AND NOT original.attisdropped
            AND original.attname='academic149_original_created_offering_count')
          THEN 'academic149_original_created_offering_count' ELSE 'created_offering_count' END
      AND NOT attribute.attisdropped;
    RETURN result;
END $$;

CREATE OR REPLACE FUNCTION pg_temp.academic149_prepare()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE selected_version bigint; receipt_relation oid; count_column smallint; state record;
        receipt_count bigint; recovered_count bigint; original_hash text;
        reconstructed_count bigint; reconstructed_hash text; projected_hash text;
        original_column jsonb;
BEGIN
    SELECT max(history.version) INTO selected_version FROM _sqlx_migrations history;
    IF selected_version NOT IN(147,148) OR selected_version IS NULL
       OR (SELECT count(*)<>selected_version OR min(history.version)<>1 OR NOT bool_and(success)
           FROM _sqlx_migrations history)
       OR current_setting('transaction_read_only')<>'off'
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;

    receipt_relation:='learning_delivery_apply_runs'::regclass;
    IF to_regclass('academic_context_receipt_recovery_audit') IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID';
    END IF;
    IF to_regclass('pg_temp.academic149_phase') IS NOT NULL THEN
        -- Native 149 repeats this prefix after native 148. The original snapshot
        -- is never recaptured and the physical-column projection is never repeated.
        IF to_regclass('pg_temp.academic149_original_receipts') IS NULL
           OR (SELECT count(*) FROM pg_temp.academic149_phase)<>1 THEN
            RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID';
        END IF;
        SELECT * INTO state FROM pg_temp.academic149_phase WHERE singleton;
        IF state.table_oid IS DISTINCT FROM receipt_relation
           OR state.source_version NOT IN(147,148)
           OR (state.source_version=148 AND selected_version<>148)
           OR (state.source_version=147 AND NOT EXISTS(
               SELECT 1 FROM pg_attribute WHERE attrelid=receipt_relation
                 AND attname='academic149_original_created_offering_count' AND NOT attisdropped))
           OR (state.source_version=148 AND EXISTS(
               SELECT 1 FROM pg_attribute WHERE attrelid=receipt_relation
                 AND attname='academic149_original_created_offering_count' AND NOT attisdropped))
           OR pg_temp.academic149_column_evidence() IS DISTINCT FROM state.column_evidence
        THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;
        IF EXISTS(
            (SELECT facts FROM pg_temp.academic149_original_receipts
             EXCEPT ALL SELECT facts FROM pg_temp.academic149_receipts(state.source_version=147,false))
            UNION ALL
            (SELECT facts FROM pg_temp.academic149_receipts(state.source_version=147,false)
             EXCEPT ALL SELECT facts FROM pg_temp.academic149_original_receipts)
        ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;
        SELECT row_count,sha256 INTO reconstructed_count,reconstructed_hash
        FROM pg_temp.academic149_receipt_summary(state.source_version=147,false);
        IF reconstructed_count IS DISTINCT FROM state.receipt_count
           OR reconstructed_hash IS DISTINCT FROM state.original_sha256
           OR reconstructed_hash IS DISTINCT FROM state.reconstructed_sha256
        THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;
        IF state.source_version=147 THEN
            SELECT sha256 INTO projected_hash FROM pg_temp.academic149_receipt_summary(false,true);
            IF projected_hash IS DISTINCT FROM state.projected_sha256 THEN
                RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED';
            END IF;
        END IF;
        RETURN;
    END IF;

    IF EXISTS(SELECT 1 FROM pg_class relation
              WHERE relation.relnamespace IN(pg_my_temp_schema(),current_schema()::regnamespace)
                AND relation.relname LIKE 'academic149\_%' ESCAPE '\')
       OR EXISTS(SELECT 1 FROM pg_type type_record
                 WHERE type_record.typnamespace IN(pg_my_temp_schema(),current_schema()::regnamespace)
                   AND type_record.typname LIKE 'academic149\_%' ESCAPE '\')
       OR EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid=receipt_relation
                 AND NOT attisdropped AND attname LIKE 'academic149\_%' ESCAPE '\')
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;

    -- Refuse schema drift rather than silently adapting the historical contract.
    IF NOT EXISTS(SELECT 1 FROM pg_class WHERE oid=receipt_relation AND relkind='r'
                  AND NOT relrowsecurity AND NOT relforcerowsecurity AND NOT relispartition)
       OR EXISTS(SELECT 1 FROM pg_inherits WHERE inhrelid=receipt_relation OR inhparent=receipt_relation)
       OR EXISTS(
           WITH expected(name,kind,required) AS (VALUES
               ('idempotency_key','uuid'::regtype,true),('academic_term_id','uuid'::regtype,true),
               ('request_hash','bpchar'::regtype,true),('source_hash','bpchar'::regtype,true),
               ('offering_ids','uuid[]'::regtype,true),('actor_user_id','uuid'::regtype,false),
               ('created_at','timestamptz'::regtype,true),('group_ids','uuid[]'::regtype,true),
               ('created_offering_count','int4'::regtype,true),('retained_offering_count','int4'::regtype,true),
               ('created_group_count','int4'::regtype,true),('retained_group_count','int4'::regtype,true),
               ('skipped_count','int4'::regtype,true),('curriculum_sources','jsonb'::regtype,true)
           ), allowed AS (
               SELECT * FROM expected UNION ALL SELECT 'starts_on','date'::regtype,false WHERE selected_version=148
           ), actual AS (
               SELECT attname,atttypid,attnotnull,atttypmod,attidentity,attgenerated
               FROM pg_attribute WHERE attrelid=receipt_relation AND attnum>0 AND NOT attisdropped
           )
           SELECT 1 FROM allowed FULL JOIN actual ON actual.attname=allowed.name
           WHERE actual.attname IS NULL OR allowed.name IS NULL OR actual.atttypid<>allowed.kind
              OR actual.attnotnull<>allowed.required OR actual.attidentity<>'' OR actual.attgenerated<>''
              OR actual.atttypmod<>CASE WHEN allowed.name IN('request_hash','source_hash') THEN 68 ELSE -1 END
       ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE'; END IF;

    SELECT attnum INTO count_column FROM pg_attribute
    WHERE attrelid=receipt_relation AND attname='created_offering_count' AND NOT attisdropped;
    IF EXISTS(SELECT 1 FROM pg_trigger WHERE tgrelid=receipt_relation AND NOT tgisinternal)
       OR EXISTS(SELECT 1 FROM pg_rewrite WHERE ev_class=receipt_relation)
       OR EXISTS(SELECT 1 FROM pg_constraint WHERE contype='f' AND confrelid=receipt_relation)
       OR EXISTS(SELECT 1 FROM pg_depend WHERE refclassid='pg_class'::regclass
                 AND refobjid=receipt_relation AND classid IN('pg_rewrite'::regclass,'pg_proc'::regclass))
       OR EXISTS(SELECT 1 FROM pg_depend WHERE refclassid='pg_class'::regclass
                 AND refobjid=receipt_relation AND refobjsubid=count_column
                 AND classid NOT IN('pg_constraint'::regclass,'pg_attrdef'::regclass))
       OR EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid=receipt_relation AND count_column=ANY(conkey)
                 AND NOT(contype='n' OR (contype='c' AND pg_get_constraintdef(oid)='CHECK ((created_offering_count >= 0))')))
       OR NOT EXISTS(SELECT 1 FROM pg_attrdef WHERE adrelid=receipt_relation AND adnum=count_column
                     AND pg_get_expr(adbin,adrelid)='0')
       OR NOT EXISTS(SELECT 1 FROM pg_constraint WHERE conrelid=receipt_relation AND contype='c'
                     AND pg_get_constraintdef(oid)='CHECK ((created_offering_count >= 0))' AND convalidated)
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY'; END IF;

    IF (SELECT count(*) FROM learning_delivery_apply_runs)>100000
       OR (SELECT count(*) FROM learning_offerings)>100000
       OR (SELECT COALESCE(sum(cardinality(offering_ids)),0) FROM learning_delivery_apply_runs)>1000000
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE'; END IF;

    recovered_count:=0;
    IF selected_version=147 THEN
        -- Mask 0 remains valid. Only a lower timestamp count, by itself, is
        -- superseded; absent/foreign/null/duplicate outputs and other guards refuse.
        IF EXISTS(
            SELECT 1 FROM learning_delivery_apply_runs ledger
            CROSS JOIN LATERAL(
                SELECT count(*) AS created_count FROM unnest(ledger.offering_ids) output(id)
                JOIN learning_offerings offering ON offering.id=output.id
                WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at
            ) created
            WHERE ledger.created_offering_count<0
               OR created.created_count>ledger.created_offering_count
               OR ledger.created_offering_count>cardinality(ledger.offering_ids)
               OR (SELECT count(*) FROM learning_offerings offering
                   WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at)<>created.created_count
               OR EXISTS(SELECT 1 FROM unnest(ledger.offering_ids) output(id)
                   LEFT JOIN learning_offerings offering ON offering.id=output.id AND offering.academic_term_id=ledger.academic_term_id
                   WHERE offering.id IS NULL)
               OR cardinality(ledger.offering_ids)<>(SELECT count(DISTINCT output.id) FROM unnest(ledger.offering_ids) output(id))
        ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE'; END IF;
        SELECT count(*) INTO recovered_count FROM learning_delivery_apply_runs ledger
        WHERE ledger.created_offering_count>(SELECT count(*) FROM unnest(ledger.offering_ids) output(id)
            JOIN learning_offerings offering ON offering.id=output.id
            WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at);
    END IF;

    CREATE TEMP TABLE academic149_original_receipts(facts jsonb NOT NULL) ON COMMIT DROP;
    INSERT INTO academic149_original_receipts SELECT facts FROM pg_temp.academic149_receipts(false,false);
    SELECT row_count,sha256 INTO receipt_count,original_hash FROM pg_temp.academic149_receipt_summary(false,false);
    original_column:=pg_temp.academic149_column_evidence();
    CREATE TEMP TABLE academic149_phase(
        singleton boolean PRIMARY KEY CHECK(singleton),source_version bigint NOT NULL,
        receipt_count bigint NOT NULL,recovered_receipt_count bigint NOT NULL,
        original_sha256 text NOT NULL,reconstructed_sha256 text NOT NULL,projected_sha256 text,
        column_evidence jsonb NOT NULL,table_oid oid NOT NULL
    ) ON COMMIT DROP;
    IF selected_version=147 THEN
        ALTER TABLE learning_delivery_apply_runs RENAME COLUMN created_offering_count TO academic149_original_created_offering_count;
        ALTER TABLE learning_delivery_apply_runs ADD COLUMN created_offering_count integer;
        UPDATE learning_delivery_apply_runs ledger SET created_offering_count=(
            SELECT count(*) FROM unnest(ledger.offering_ids) output(id)
            JOIN learning_offerings offering ON offering.id=output.id
            WHERE offering.academic_term_id=ledger.academic_term_id AND offering.created_at=ledger.created_at);
        SELECT sha256 INTO projected_hash FROM pg_temp.academic149_receipt_summary(false,true);
    END IF;
    IF pg_temp.academic149_column_evidence() IS DISTINCT FROM original_column OR EXISTS(
        (SELECT facts FROM academic149_original_receipts
         EXCEPT ALL SELECT facts FROM pg_temp.academic149_receipts(selected_version=147,false))
        UNION ALL
        (SELECT facts FROM pg_temp.academic149_receipts(selected_version=147,false)
         EXCEPT ALL SELECT facts FROM academic149_original_receipts)
    ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;
    SELECT row_count,sha256 INTO reconstructed_count,reconstructed_hash
    FROM pg_temp.academic149_receipt_summary(selected_version=147,false);
    IF reconstructed_count IS DISTINCT FROM receipt_count OR reconstructed_hash IS DISTINCT FROM original_hash THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED';
    END IF;
    INSERT INTO academic149_phase VALUES(true,selected_version,receipt_count,recovered_count,
        original_hash,reconstructed_hash,projected_hash,original_column,receipt_relation);
END $$;
SELECT pg_temp.academic149_prepare();
-- ACADEMIC_CONTEXT_149_FINALIZE
DO $$
DECLARE state record; native record; native_receipt jsonb; native_link jsonb;
        native_count bigint; native_hash text; final_count bigint; final_hash text;
        before_nonreceipt jsonb; after_nonreceipt jsonb; nonreceipt_hash text;
        legacy_dates boolean;
BEGIN
    IF (SELECT max(version) FROM _sqlx_migrations)<>148
       OR to_regclass('pg_temp.academic149_phase') IS NULL
       OR to_regclass('pg_temp.academic149_original_receipts') IS NULL
       OR (SELECT count(*) FROM pg_temp.academic149_phase)<>1
       OR (SELECT count(*) FROM academic_context_cutover_audit)<>1
       OR NOT EXISTS(SELECT 1 FROM pg_trigger
           WHERE tgrelid='academic_context_cutover_audit'::regclass
             AND tgname='academic_context_cutover_audit_immutable' AND tgenabled='O'
             AND tgfoid='academic_reject_official_result_mutation()'::regprocedure)
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;
    SELECT * INTO state FROM pg_temp.academic149_phase WHERE singleton;
    SELECT * INTO native FROM academic_context_cutover_audit WHERE singleton;
    native_receipt:=native.retained_facts->'learning_delivery_apply_runs';
    IF jsonb_typeof(native.retained_facts) IS DISTINCT FROM 'object'
       OR jsonb_typeof(native_receipt) IS DISTINCT FROM 'object'
       OR native_receipt-ARRAY['rowCount','sha256']<>'{}'::jsonb
       OR NOT native_receipt ?& ARRAY['rowCount','sha256']
       OR jsonb_typeof(native_receipt->'rowCount') IS DISTINCT FROM 'number'
       OR NOT COALESCE(native_receipt->>'rowCount' ~ '^(0|[1-9][0-9]{0,18})$',false)
       OR jsonb_typeof(native_receipt->'sha256') IS DISTINCT FROM 'string'
       OR NOT COALESCE(native_receipt->>'sha256' ~ '^[0-9a-f]{64}$',false)
       OR native.mapped_calendar_targets<0 OR native.materialized_attendance_days<0 OR native.scheduled_behavior_resets<0
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;
    IF (native_receipt->>'rowCount')::numeric>9223372036854775807 THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID';
    END IF;
    native_count:=(native_receipt->>'rowCount')::bigint;
    native_hash:=native_receipt->>'sha256';
    native_link:=jsonb_build_object('retainedFacts',native.retained_facts,
        'mappedCalendarTargets',native.mapped_calendar_targets,
        'materializedAttendanceDays',native.materialized_attendance_days,
        'scheduledBehaviorResets',native.scheduled_behavior_resets);

    IF state.source_version=147 THEN
        IF to_regprocedure('pg_temp.academic148_snapshot()') IS NULL
           OR to_regclass('pg_temp.academic148_preservation_tables') IS NULL
           OR native_count IS DISTINCT FROM state.receipt_count
           OR native_hash IS DISTINCT FROM state.projected_sha256
        THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;
        SELECT jsonb_object_agg(table_name,jsonb_build_object('rowCount',row_count,'sha256',facts_sha256))
        INTO before_nonreceipt FROM pg_temp.academic148_snapshot()
        WHERE table_name<>'learning_delivery_apply_runs';
        IF before_nonreceipt IS DISTINCT FROM native.retained_facts-'learning_delivery_apply_runs' THEN
            RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED';
        END IF;
        -- Drop only the derived projection; the original column, default and
        -- constraints survive. No other domain relation is mutated in this phase.
        ALTER TABLE learning_delivery_apply_runs DROP COLUMN created_offering_count RESTRICT;
        ALTER TABLE learning_delivery_apply_runs RENAME COLUMN academic149_original_created_offering_count TO created_offering_count;
        SELECT NOT EXISTS(SELECT 1 FROM learning_delivery_apply_runs WHERE starts_on IS NOT NULL) INTO legacy_dates;
        IF NOT legacy_dates THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;
        SELECT jsonb_object_agg(table_name,jsonb_build_object('rowCount',row_count,'sha256',facts_sha256))
        INTO after_nonreceipt FROM pg_temp.academic148_snapshot()
        WHERE table_name<>'learning_delivery_apply_runs';
        IF after_nonreceipt IS DISTINCT FROM before_nonreceipt THEN
            RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED';
        END IF;
        nonreceipt_hash:=encode(sha256(convert_to(before_nonreceipt::text,'UTF8')),'hex');
    ELSIF state.source_version<>148 OR state.recovered_receipt_count<>0 OR state.projected_sha256 IS NOT NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID';
    END IF;

    IF pg_temp.academic149_column_evidence() IS DISTINCT FROM state.column_evidence OR EXISTS(
        (SELECT facts FROM pg_temp.academic149_original_receipts
         EXCEPT ALL SELECT facts FROM pg_temp.academic149_receipts(false,state.source_version=147))
        UNION ALL
        (SELECT facts FROM pg_temp.academic149_receipts(false,state.source_version=147)
         EXCEPT ALL SELECT facts FROM pg_temp.academic149_original_receipts)
    ) THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;
    SELECT row_count,sha256 INTO final_count,final_hash
    FROM pg_temp.academic149_receipt_summary(false,state.source_version=147);
    IF final_count IS DISTINCT FROM state.receipt_count
       OR final_hash IS DISTINCT FROM state.original_sha256
       OR final_hash IS DISTINCT FROM state.reconstructed_sha256
       OR state.recovered_receipt_count<0 OR state.recovered_receipt_count>state.receipt_count
       OR EXISTS(SELECT 1 FROM pg_attribute WHERE attrelid='learning_delivery_apply_runs'::regclass
                 AND NOT attisdropped AND attname LIKE 'academic149\_%' ESCAPE '\')
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED'; END IF;

    CREATE TABLE academic_context_receipt_recovery_audit(
        singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
        evidence jsonb NOT NULL,checked_at timestamptz NOT NULL DEFAULT now()
    );
    INSERT INTO academic_context_receipt_recovery_audit(singleton,evidence)
    VALUES(true,jsonb_build_object(
        'protocolVersion',1,'sourceVersion',state.source_version,
        'receiptCount',state.receipt_count,'recoveredReceiptCount',state.recovered_receipt_count,
        'originalSha256',state.original_sha256,'reconstructedSha256',state.reconstructed_sha256,
        'projectedSha256',state.projected_sha256,'finalSha256',final_hash,
        'native148ReceiptSha256',native_hash,'native148ReceiptCount',native_count,
        'native148LinkSha256',encode(sha256(convert_to(native_link::text,'UTF8')),'hex'),
        'nonreceiptSha256',nonreceipt_hash,'legacyDatesUnbound',legacy_dates,
        'originalColumnPreserved',true,'phaseProofPassed',true));
    CREATE TRIGGER academic_context_receipt_recovery_audit_immutable
    BEFORE UPDATE OR DELETE ON academic_context_receipt_recovery_audit
    FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
    DROP TABLE pg_temp.academic149_original_receipts;
    DROP TABLE pg_temp.academic149_phase;
END $$;
DROP FUNCTION pg_temp.academic149_prepare();
DROP FUNCTION pg_temp.academic149_column_evidence();
DROP FUNCTION pg_temp.academic149_receipt_summary(boolean,boolean);
DROP FUNCTION pg_temp.academic149_receipts(boolean,boolean);
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM pg_class relation
              WHERE relation.relnamespace IN(pg_my_temp_schema(),current_schema()::regnamespace)
                AND relation.relname LIKE 'academic149\_%' ESCAPE '\')
       OR EXISTS(SELECT 1 FROM pg_proc routine
                 WHERE routine.pronamespace IN(pg_my_temp_schema(),current_schema()::regnamespace)
                   AND routine.proname LIKE 'academic149\_%' ESCAPE '\')
       OR EXISTS(SELECT 1 FROM pg_type type_record
                 WHERE type_record.typnamespace IN(pg_my_temp_schema(),current_schema()::regnamespace)
                   AND type_record.typname LIKE 'academic149\_%' ESCAPE '\')
       OR EXISTS(SELECT 1 FROM pg_attribute
                 WHERE attrelid='learning_delivery_apply_runs'::regclass AND NOT attisdropped
                   AND attname LIKE 'academic149\_%' ESCAPE '\')
    THEN RAISE EXCEPTION 'ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID'; END IF;
END $$;
