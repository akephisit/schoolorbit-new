-- Quiesce old writers under maintenance. Recover forward after this cutover.
LOCK TABLE notification_dispatch_jobs, notifications, attendance_notifications,
    process_notification_sources, work_items, work_item_assignees, push_subscriptions IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE notification_inbox_before ON COMMIT DROP AS
SELECT to_jsonb(n) value FROM notifications n;
CREATE TEMP TABLE notification_receipts_before ON COMMIT DROP AS
SELECT to_jsonb(j) value FROM notification_dispatch_jobs j;

DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM notification_dispatch_jobs j JOIN notifications n ON n.id=j.notification_id
        WHERE n.user_id IS DISTINCT FROM j.recipient_id OR n.title IS DISTINCT FROM j.title
        OR n.message IS DISTINCT FROM j.message OR n.type IS DISTINCT FROM j.type
        OR n.link IS DISTINCT FROM j.link)
        OR EXISTS (SELECT 1 FROM notification_dispatch_jobs WHERE length(title)>255)
    THEN RAISE EXCEPTION 'NOTIFICATION_INBOX_FIRST_IDENTITY_CONFLICT'; END IF;
    IF EXISTS(SELECT 1 FROM notification_dispatch_jobs j JOIN attendance_notifications a ON a.id=j.source_id
        WHERE j.source_kind='attendance' AND (a.notification_id IS DISTINCT FROM j.notification_id OR a.recipient_id IS DISTINCT FROM j.recipient_id))
       OR EXISTS(SELECT 1 FROM notification_dispatch_jobs j JOIN process_notification_sources s ON s.id=j.source_id
        WHERE j.source_kind='process' AND s.recipient_id IS DISTINCT FROM j.recipient_id)
    THEN RAISE EXCEPTION 'NOTIFICATION_INBOX_FIRST_SOURCE_IDENTITY_CONFLICT'; END IF;
    IF EXISTS(SELECT 1 FROM notification_dispatch_jobs WHERE dedup_key IS NOT NULL
        GROUP BY recipient_id,dedup_key HAVING count(*)>1)
    THEN RAISE EXCEPTION 'NOTIFICATION_INBOX_FIRST_IDEMPOTENCY_CONFLICT'; END IF;
END $$;

-- Only never-stored, live-source deferred work may be materialized.
-- Terminal/suppressed and previously stored absent rows intentionally remain absent.
CREATE TEMP TABLE notification_legacy_materialize ON COMMIT DROP AS
SELECT j.* FROM notification_dispatch_jobs j
WHERE j.stored_at IS NULL AND j.published_at IS NULL
    AND NOT EXISTS(SELECT 1 FROM notifications n WHERE n.id=j.notification_id)
    AND (j.source_kind='direct'
      OR (j.source_kind='attendance' AND EXISTS(SELECT 1 FROM attendance_notifications a
          WHERE a.id=j.source_id AND a.recipient_id=j.recipient_id AND a.notification_id=j.notification_id))
      OR (j.source_kind='process' AND EXISTS(SELECT 1 FROM process_notification_sources s
          JOIN process_instances i ON i.id=s.instance_id
          WHERE s.id=j.source_id AND s.recipient_id=j.recipient_id AND s.cancelled_at IS NULL
          AND (s.work_item_id IS NULL OR EXISTS(SELECT 1 FROM work_items w JOIN work_item_assignees a ON a.work_item_id=w.id
              WHERE w.id=s.work_item_id AND w.status='active' AND a.assignee_type='user'
                AND a.user_id=s.recipient_id AND a.status IN ('assigned','read'))))));
INSERT INTO notifications(id,user_id,title,message,type,link,created_at)
SELECT notification_id,recipient_id,title,message,type,link,created_at FROM notification_legacy_materialize;

CREATE TABLE notification_presentations (
    receipt_id UUID PRIMARY KEY REFERENCES notification_dispatch_jobs(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    outcome TEXT CHECK(outcome IN ('attempted','skipped','expired','legacy_skipped')),
    CHECK ((consumed_at IS NULL) = (outcome IS NULL))
);
CREATE INDEX notification_presentations_pending_idx ON notification_presentations(receipt_id)
    WHERE consumed_at IS NULL;
CREATE INDEX notification_presentations_cleanup_idx ON notification_presentations(consumed_at,receipt_id)
    WHERE consumed_at IS NOT NULL;

-- Preserve the original receipt, including historical published_at's suppression meaning.
-- A cutover never creates a fresh alert for previously accepted/attempted work.
INSERT INTO notification_presentations(receipt_id,expires_at,consumed_at,outcome)
SELECT id,created_at+interval '15 minutes',now(),'legacy_skipped' FROM notification_dispatch_jobs;

DO $$ BEGIN
    IF EXISTS(SELECT value FROM notification_inbox_before EXCEPT SELECT to_jsonb(n) FROM notifications n)
       OR EXISTS(SELECT value FROM notification_receipts_before EXCEPT SELECT to_jsonb(j) FROM notification_dispatch_jobs j)
       OR EXISTS(SELECT to_jsonb(j) FROM notification_dispatch_jobs j EXCEPT SELECT value FROM notification_receipts_before)
       OR (SELECT count(*) FROM notifications) <> (SELECT count(*) FROM notification_inbox_before)+(SELECT count(*) FROM notification_legacy_materialize)
       OR EXISTS(SELECT 1 FROM notification_legacy_materialize j LEFT JOIN notifications n ON n.id=j.notification_id
          WHERE n.id IS NULL OR n.user_id IS DISTINCT FROM j.recipient_id OR n.title IS DISTINCT FROM j.title
             OR n.message IS DISTINCT FROM j.message OR n.type IS DISTINCT FROM j.type OR n.link IS DISTINCT FROM j.link
             OR n.created_at IS DISTINCT FROM j.created_at OR n.read_at IS NOT NULL)
       OR (SELECT count(*) FROM notification_presentations) <> (SELECT count(*) FROM notification_receipts_before)
    THEN RAISE EXCEPTION 'NOTIFICATION_INBOX_FIRST_PRESERVATION_FAILED'; END IF;
END $$;

ALTER TABLE notification_dispatch_jobs DROP CONSTRAINT notification_dispatch_jobs_source_kind_check;
ALTER TABLE notification_dispatch_jobs DROP CONSTRAINT notification_dispatch_jobs_check;
ALTER TABLE notification_dispatch_jobs ADD CONSTRAINT notification_dispatch_jobs_source_kind_check
    CHECK(source_kind IN ('direct','attendance','process','calendar_reminder'));
ALTER TABLE notification_dispatch_jobs ADD CONSTRAINT notification_dispatch_jobs_check
    CHECK((source_kind='direct' AND source_id IS NULL) OR (source_kind<>'direct' AND source_id IS NOT NULL));
-- One reminder source has many recipients; exact source+recipient remains unique.
DROP INDEX notification_dispatch_source_idx;
CREATE UNIQUE INDEX notification_dispatch_source_recipient_idx
    ON notification_dispatch_jobs(source_kind,source_id,recipient_id) WHERE source_id IS NOT NULL;

ALTER TABLE push_subscriptions
    ADD COLUMN installation_id UUID,
    ADD COLUMN label TEXT CHECK(length(label) BETWEEN 1 AND 80),
    ADD COLUMN version UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN binding_token UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN key_id TEXT,
    ADD COLUMN session_id UUID REFERENCES auth_sessions(id) ON DELETE CASCADE,
    ADD COLUMN last_confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN expires_at TIMESTAMPTZ NOT NULL DEFAULT now();
CREATE UNIQUE INDEX push_subscriptions_installation_idx ON push_subscriptions(installation_id)
    WHERE installation_id IS NOT NULL;
CREATE INDEX push_subscriptions_expiry_idx ON push_subscriptions(expires_at,id);
-- Legacy registrations have no proven account/session binding and cannot receive Push.

-- Migration-owned hashed evidence survives an interruption between 139 and 140.
-- It covers complete rows (including read_at and original timestamps), never private payload copies.
CREATE TABLE notification_inbox_first_evidence (
    owner TEXT NOT NULL CHECK(owner IN ('inbox','receipt','attendance','process','presentation','source_eligibility')),
    id UUID NOT NULL,
    row_hash BYTEA NOT NULL CHECK(octet_length(row_hash)=32),
    PRIMARY KEY(owner,id)
);
INSERT INTO notification_inbox_first_evidence
SELECT 'inbox',id,sha256(convert_to(to_jsonb(n)::text,'UTF8')) FROM notifications n
UNION ALL SELECT 'receipt',id,sha256(convert_to(to_jsonb(j)::text,'UTF8')) FROM notification_dispatch_jobs j
UNION ALL SELECT 'attendance',id,sha256(convert_to(to_jsonb(a)::text,'UTF8')) FROM attendance_notifications a
UNION ALL SELECT 'process',id,sha256(convert_to(to_jsonb(s)::text,'UTF8')) FROM process_notification_sources s
UNION ALL SELECT 'presentation',receipt_id,sha256(convert_to(to_jsonb(p)::text,'UTF8')) FROM notification_presentations p
UNION ALL SELECT 'source_eligibility',s.id,sha256(convert_to(jsonb_build_array(s.work_item_id,
    s.work_item_id IS NULL OR EXISTS(SELECT 1 FROM work_items w JOIN work_item_assignees a ON a.work_item_id=w.id
        WHERE w.id=s.work_item_id AND w.status='active' AND a.assignee_type='user' AND a.user_id=s.recipient_id
            AND a.status IN ('assigned','read')))::text,'UTF8')) FROM process_notification_sources s;
