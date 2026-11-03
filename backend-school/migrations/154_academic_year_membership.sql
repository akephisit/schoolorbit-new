-- Annual preparation is scoped by academic year, without invented civil dates.
-- Existing dated memberships and accepted promotion evidence remain unchanged.
LOCK TABLE homeroom_placements, academic_promotion_runs,
    academic_promotion_run_items, academic_promotion_run_commands,
    academic_promotion_run_approvals, academic_promotion_execution_batches,
    academic_promotion_execution_receipts IN SHARE ROW EXCLUSIVE MODE;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_promotion_runs WHERE status='executing')
       OR EXISTS(
           SELECT 1 FROM academic_promotion_execution_batches batch
           CROSS JOIN LATERAL unnest(batch.item_ids) item(id)
           LEFT JOIN academic_promotion_execution_receipts receipt
             ON receipt.item_id=item.id AND receipt.run_id=batch.run_id
           WHERE receipt.item_id IS NULL
       ) THEN
        RAISE EXCEPTION 'ACADEMIC_MEMBERSHIP154_EXECUTION_UNRESOLVED' USING ERRCODE='23514';
    END IF;
END $$;

CREATE TEMP TABLE academic_membership154_before ON COMMIT DROP AS
SELECT name,count(*) AS row_count,
       encode(sha256(convert_to(coalesce(string_agg(fact::text,'' ORDER BY fact::text),''),'UTF8')),'hex') AS fingerprint
FROM (
    SELECT 'placements' AS name,to_jsonb(row) AS fact FROM homeroom_placements row
    UNION ALL SELECT 'runs',to_jsonb(row) FROM academic_promotion_runs row
    UNION ALL SELECT 'items',to_jsonb(row) FROM academic_promotion_run_items row
    UNION ALL SELECT 'commands',to_jsonb(row) FROM academic_promotion_run_commands row
    UNION ALL SELECT 'approvals',to_jsonb(row) FROM academic_promotion_run_approvals row
    UNION ALL SELECT 'batches',to_jsonb(row) FROM academic_promotion_execution_batches row
    UNION ALL SELECT 'receipts',to_jsonb(row) FROM academic_promotion_execution_receipts row
) facts GROUP BY name;

ALTER TABLE homeroom_placements
    ADD COLUMN placement_scope text NOT NULL DEFAULT 'dated',
    ALTER COLUMN start_date DROP NOT NULL,
    ADD CONSTRAINT homeroom_placements_scope_check CHECK(
        (placement_scope='dated' AND start_date IS NOT NULL)
        OR (placement_scope='academic_year' AND start_date IS NULL)
    );
ALTER TABLE academic_promotion_runs ALTER COLUMN target_placement_starts_on DROP NOT NULL;

-- Context selection remains the caller's responsibility. NULL lower bounds
-- mean annual membership, while actual transfers retain their dated bounds.
CREATE FUNCTION academic_placement_applies_on(scope text,starts_on date,ends_on date,on_date date)
RETURNS boolean LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT on_date IS NOT NULL
       AND ((scope='academic_year' AND starts_on IS NULL)
            OR (scope='dated' AND starts_on IS NOT NULL AND starts_on<=on_date))
       AND (ends_on IS NULL OR ends_on>=on_date)
$$;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM homeroom_placements WHERE placement_scope<>'dated')
       OR EXISTS(
           (SELECT * FROM academic_membership154_before
            EXCEPT
            SELECT name,count(*),encode(sha256(convert_to(coalesce(string_agg(fact::text,'' ORDER BY fact::text),''),'UTF8')),'hex')
            FROM (
                SELECT 'placements' AS name,to_jsonb(row)-'placement_scope' AS fact FROM homeroom_placements row
                UNION ALL SELECT 'runs',to_jsonb(row) FROM academic_promotion_runs row
                UNION ALL SELECT 'items',to_jsonb(row) FROM academic_promotion_run_items row
                UNION ALL SELECT 'commands',to_jsonb(row) FROM academic_promotion_run_commands row
                UNION ALL SELECT 'approvals',to_jsonb(row) FROM academic_promotion_run_approvals row
                UNION ALL SELECT 'batches',to_jsonb(row) FROM academic_promotion_execution_batches row
                UNION ALL SELECT 'receipts',to_jsonb(row) FROM academic_promotion_execution_receipts row
            ) facts GROUP BY name)
       ) THEN
        RAISE EXCEPTION 'ACADEMIC_MEMBERSHIP154_PRESERVATION_FAILED' USING ERRCODE='23514';
    END IF;
END $$;
