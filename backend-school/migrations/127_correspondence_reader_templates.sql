-- Preserve immutable historical reads before enabling the resource-key receipt API.
-- Invalid or ambiguous evidence must be reconciled through an owned forward migration.
DO $$
DECLARE
    row_data record;
    invalid_rows bigint := 0;
    ambiguous_reads bigint := 0;
BEGIN
    FOR row_data IN
        SELECT h.* FROM process_action_history h
        LEFT JOIN process_instances i ON i.id=h.instance_id
        WHERE h.entry->>'action'='read'
          AND (h.correspondence_id IS NOT NULL OR i.source_module='correspondence')
    LOOP
        BEGIN
            IF row_data.correspondence_id IS NULL
                OR row_data.entry->>'id' IS DISTINCT FROM row_data.id::text
                OR row_data.entry->>'actorId' IS DISTINCT FROM row_data.actor_id::text
                OR jsonb_typeof(row_data.entry->'documentRevision') IS DISTINCT FROM 'number'
                OR row_data.entry->>'createdAt' IS NULL
                OR jsonb_typeof(row_data.entry->'createdAt') IS DISTINCT FROM 'string'
                OR row_data.entry->>'createdAt' !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]{1,9})?(Z|[+-][0-9]{2}:[0-9]{2})$'
                OR NOT isfinite((row_data.entry->>'createdAt')::timestamptz)
                OR NOT EXISTS (
                    SELECT 1 FROM correspondence_document_revisions d
                    JOIN correspondence_records c ON c.id=d.correspondence_id
                    JOIN process_instances i ON i.id=row_data.instance_id
                    WHERE d.correspondence_id=row_data.correspondence_id
                      AND d.revision=(row_data.entry->>'documentRevision')::bigint
                      AND i.source_module='correspondence' AND i.source_type='correspondence'
                      AND i.source_id=c.id AND c.instance_id=i.id
                )
            THEN invalid_rows := invalid_rows+1;
            END IF;
        EXCEPTION WHEN invalid_text_representation OR invalid_datetime_format
            OR datetime_field_overflow OR numeric_value_out_of_range THEN
            invalid_rows := invalid_rows+1;
        END;
    END LOOP;
    IF invalid_rows > 0 THEN
        RAISE EXCEPTION 'CORRESPONDENCE_READ_BACKFILL_BLOCKED invalid_rows=%', invalid_rows;
    END IF;
    SELECT count(*) INTO ambiguous_reads FROM (
        SELECT correspondence_id,actor_id FROM (
            SELECT h.*,min((entry->>'createdAt')::timestamptz)
                OVER (PARTITION BY correspondence_id,actor_id) AS first_read
            FROM process_action_history h WHERE correspondence_id IS NOT NULL AND entry->>'action'='read'
        ) reads WHERE (entry->>'createdAt')::timestamptz=first_read
        GROUP BY correspondence_id,actor_id
        HAVING count(DISTINCT (entry->>'documentRevision')::bigint)>1
    ) ambiguous;
    IF ambiguous_reads > 0 THEN
        RAISE EXCEPTION 'CORRESPONDENCE_READ_BACKFILL_BLOCKED ambiguous_first_reads=%', ambiguous_reads;
    END IF;
END $$;

CREATE TABLE correspondence_read_receipts (
    correspondence_id uuid NOT NULL REFERENCES correspondence_records(id),
    user_id uuid NOT NULL REFERENCES users(id),
    document_revision bigint NOT NULL,
    first_read_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (correspondence_id,user_id),
    FOREIGN KEY (correspondence_id,document_revision)
        REFERENCES correspondence_document_revisions(correspondence_id,revision)
);
INSERT INTO correspondence_read_receipts(correspondence_id,user_id,document_revision,first_read_at)
SELECT DISTINCT ON (correspondence_id,actor_id)
    correspondence_id,actor_id,(entry->>'documentRevision')::bigint,(entry->>'createdAt')::timestamptz
FROM process_action_history
WHERE correspondence_id IS NOT NULL AND entry->>'action'='read'
ORDER BY correspondence_id,actor_id,(entry->>'createdAt')::timestamptz,id;
CREATE TRIGGER immutable_correspondence_read_receipts
BEFORE UPDATE OR DELETE ON correspondence_read_receipts
FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
