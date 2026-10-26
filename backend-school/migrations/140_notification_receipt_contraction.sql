-- Contract only after 139 has reconciled authoritative inbox and legacy state.
LOCK TABLE notification_dispatch_jobs, notification_presentations, notifications,
    attendance_notifications,process_notification_sources,notification_inbox_first_evidence IN ACCESS EXCLUSIVE MODE;
CREATE TEMP TABLE notification_contract_observed ON COMMIT DROP AS
SELECT 'inbox'::text owner,id,sha256(convert_to(to_jsonb(n)::text,'UTF8')) row_hash FROM notifications n
UNION ALL SELECT 'receipt',id,sha256(convert_to(to_jsonb(j)::text,'UTF8')) FROM notification_dispatch_jobs j
UNION ALL SELECT 'attendance',id,sha256(convert_to(to_jsonb(a)::text,'UTF8')) FROM attendance_notifications a
UNION ALL SELECT 'process',id,sha256(convert_to(to_jsonb(s)::text,'UTF8')) FROM process_notification_sources s
UNION ALL SELECT 'presentation',receipt_id,sha256(convert_to(to_jsonb(p)::text,'UTF8')) FROM notification_presentations p;
DO $$ BEGIN
    IF EXISTS(SELECT * FROM notification_inbox_first_evidence EXCEPT SELECT * FROM notification_contract_observed)
       OR EXISTS(SELECT * FROM notification_contract_observed EXCEPT SELECT * FROM notification_inbox_first_evidence)
       OR EXISTS(SELECT 1 FROM notification_dispatch_jobs j JOIN notifications n ON n.id=j.notification_id
            WHERE n.user_id IS DISTINCT FROM j.recipient_id OR n.title IS DISTINCT FROM j.title
             OR n.message IS DISTINCT FROM j.message OR n.type IS DISTINCT FROM j.type OR n.link IS DISTINCT FROM j.link)
    THEN RAISE EXCEPTION 'NOTIFICATION_RECEIPT_CONTRACTION_STALE_EVIDENCE'; END IF;
END $$;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM notification_dispatch_jobs j LEFT JOIN notification_presentations p ON p.receipt_id=j.id
        WHERE p.receipt_id IS NULL OR p.outcome IS DISTINCT FROM 'legacy_skipped' OR p.consumed_at IS NULL)
    THEN RAISE EXCEPTION 'NOTIFICATION_RECEIPT_CONTRACTION_UNRECONCILED'; END IF;
END $$;
CREATE TEMP TABLE notification_contract_inbox ON COMMIT DROP AS SELECT to_jsonb(n) value FROM notifications n;
CREATE TEMP TABLE notification_contract_expected ON COMMIT DROP AS
SELECT j.id,j.notification_id,j.recipient_id,j.source_kind,j.source_id,j.dedup_key idempotency_key,
    sha256(convert_to(jsonb_build_array(j.title,j.message,j.type,j.link)::text,'UTF8')) payload_hash,
    j.created_at,coalesce(j.stored_at,n.created_at) inbox_accepted_at,j.published_at legacy_published_at,
    'alert_once'::text presentation,p.consumed_at presentation_consumed_at,p.outcome presentation_outcome
FROM notification_dispatch_jobs j JOIN notification_presentations p ON p.receipt_id=j.id
LEFT JOIN notifications n ON n.id=j.notification_id;

ALTER TABLE notification_dispatch_jobs RENAME TO notification_receipts;
ALTER TABLE notification_receipts RENAME COLUMN dedup_key TO idempotency_key;
ALTER TABLE notification_receipts RENAME COLUMN stored_at TO inbox_accepted_at;
ALTER TABLE notification_receipts RENAME COLUMN published_at TO legacy_published_at;
ALTER TABLE notification_receipts ADD COLUMN payload_hash BYTEA,
    ADD COLUMN presentation TEXT NOT NULL DEFAULT 'alert_once' CHECK(presentation IN ('alert_once','inbox_only')),
    ADD COLUMN presentation_consumed_at TIMESTAMPTZ,
    ADD COLUMN presentation_outcome TEXT CHECK(presentation_outcome IN ('attempted','skipped','expired','legacy_skipped'));
UPDATE notification_receipts j SET payload_hash=e.payload_hash,inbox_accepted_at=e.inbox_accepted_at,
    presentation_consumed_at=e.presentation_consumed_at,presentation_outcome=e.presentation_outcome
FROM notification_contract_expected e WHERE e.id=j.id;
ALTER TABLE notification_receipts ALTER COLUMN payload_hash SET NOT NULL;
ALTER TABLE notification_receipts ADD CONSTRAINT notification_receipts_hash_length CHECK(octet_length(payload_hash)=32),
    ADD CONSTRAINT notification_receipts_presentation_terminal CHECK((presentation_consumed_at IS NULL)=(presentation_outcome IS NULL));
DROP INDEX notification_dispatch_due_idx;
ALTER TABLE notification_receipts DROP COLUMN title,DROP COLUMN message,DROP COLUMN type,DROP COLUMN link,
    DROP COLUMN next_attempt_at,DROP COLUMN attempts,DROP COLUMN lease_token;
DELETE FROM notification_presentations WHERE consumed_at IS NOT NULL;
DROP INDEX notification_presentations_pending_idx;
DROP INDEX notification_presentations_cleanup_idx;
ALTER TABLE notification_presentations DROP COLUMN consumed_at,DROP COLUMN outcome;

DO $$ BEGIN
    IF EXISTS(SELECT * FROM notification_contract_expected EXCEPT SELECT id,notification_id,recipient_id,source_kind,source_id,idempotency_key,payload_hash,created_at,inbox_accepted_at,legacy_published_at,presentation,presentation_consumed_at,presentation_outcome FROM notification_receipts)
       OR EXISTS(SELECT id,notification_id,recipient_id,source_kind,source_id,idempotency_key,payload_hash,created_at,inbox_accepted_at,legacy_published_at,presentation,presentation_consumed_at,presentation_outcome FROM notification_receipts EXCEPT SELECT * FROM notification_contract_expected)
       OR EXISTS(SELECT value FROM notification_contract_inbox EXCEPT SELECT to_jsonb(n) FROM notifications n)
       OR EXISTS(SELECT to_jsonb(n) FROM notifications n EXCEPT SELECT value FROM notification_contract_inbox)
       OR (SELECT count(*) FROM notification_receipts) <> (SELECT count(*) FROM notification_contract_expected)
       OR EXISTS(SELECT 1 FROM notification_presentations)
    THEN RAISE EXCEPTION 'NOTIFICATION_RECEIPT_CONTRACTION_PRESERVATION_FAILED'; END IF;
END $$;

DROP TABLE notification_inbox_first_evidence;
