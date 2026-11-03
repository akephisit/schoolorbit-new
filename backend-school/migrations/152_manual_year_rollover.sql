-- Manual confirmation keeps the existing promotion ledger. Historical policy,
-- result, approval and execution evidence is never rewritten.
LOCK TABLE academic_promotion_runs, academic_promotion_run_items,
    academic_promotion_run_commands, academic_promotion_run_approvals,
    academic_promotion_execution_batches, academic_promotion_execution_receipts
    IN SHARE ROW EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM academic_promotion_runs WHERE status='executing')
       OR EXISTS (
           SELECT 1 FROM academic_promotion_execution_batches batch
           CROSS JOIN LATERAL unnest(batch.item_ids) item(id)
           LEFT JOIN academic_promotion_execution_receipts receipt
             ON receipt.item_id=item.id AND receipt.run_id=batch.run_id
           WHERE receipt.item_id IS NULL
       ) THEN
        RAISE EXCEPTION 'MANUAL_ROLLOVER_LEGACY_EXECUTION_UNRESOLVED' USING ERRCODE='23514';
    END IF;
END;
$$;

CREATE TEMP TABLE manual_rollover152_preserved ON COMMIT DROP AS
SELECT name, count(*) AS rows, md5(coalesce(string_agg(value::text,'' ORDER BY value::text),'')) AS fingerprint
FROM (
    SELECT 'runs' AS name,to_jsonb(row) AS value FROM academic_promotion_runs row
    UNION ALL SELECT 'items',to_jsonb(row) FROM academic_promotion_run_items row
    UNION ALL SELECT 'commands',to_jsonb(row) FROM academic_promotion_run_commands row
    UNION ALL SELECT 'approvals',to_jsonb(row) FROM academic_promotion_run_approvals row
    UNION ALL SELECT 'batches',to_jsonb(row) FROM academic_promotion_execution_batches row
    UNION ALL SELECT 'receipts',to_jsonb(row) FROM academic_promotion_execution_receipts row
) facts GROUP BY name;

ALTER TABLE academic_promotion_runs ALTER COLUMN policy_id DROP NOT NULL;
ALTER TABLE academic_promotion_run_items ALTER COLUMN recommendation DROP NOT NULL;
ALTER TABLE academic_promotion_run_commands DROP CONSTRAINT academic_promotion_run_commands_action_check;
ALTER TABLE academic_promotion_run_commands ADD CONSTRAINT academic_promotion_run_commands_action_check
    CHECK(action IN ('calculate','approve','execute','confirm'));

DO $$
BEGIN
    IF EXISTS (
        SELECT * FROM manual_rollover152_preserved
        EXCEPT
        SELECT name,count(*),md5(coalesce(string_agg(value::text,'' ORDER BY value::text),''))
        FROM (
            SELECT 'runs' AS name,to_jsonb(row) AS value FROM academic_promotion_runs row
            UNION ALL SELECT 'items',to_jsonb(row) FROM academic_promotion_run_items row
            UNION ALL SELECT 'commands',to_jsonb(row) FROM academic_promotion_run_commands row
            UNION ALL SELECT 'approvals',to_jsonb(row) FROM academic_promotion_run_approvals row
            UNION ALL SELECT 'batches',to_jsonb(row) FROM academic_promotion_execution_batches row
            UNION ALL SELECT 'receipts',to_jsonb(row) FROM academic_promotion_execution_receipts row
        ) facts GROUP BY name
    ) THEN
        RAISE EXCEPTION 'MANUAL_ROLLOVER152_PRESERVATION_FAILED' USING ERRCODE='23514';
    END IF;
END;
$$;
