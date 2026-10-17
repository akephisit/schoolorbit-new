-- Stop the previous backend/dispatcher under maintenance before this cutover.
-- All copying, preservation gates and contraction share the migration transaction.
LOCK TABLE attendance_notifications, notification_dispatch_jobs, notifications IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM notification_dispatch_jobs)
        OR EXISTS (SELECT 1 FROM attendance_notifications
            GROUP BY notification_id HAVING count(*) > 1) THEN
        RAISE EXCEPTION 'NOTIFICATION_DISPATCH_IDENTITY_CONFLICT';
    END IF;
    IF EXISTS (SELECT 1 FROM attendance_notifications
        WHERE attempts < 0 OR length(title) > 255) THEN
        RAISE EXCEPTION 'NOTIFICATION_DISPATCH_PAYLOAD_UNMAPPABLE';
    END IF;
    IF EXISTS (
        SELECT 1 FROM attendance_notifications a
        JOIN notifications n ON n.id = a.notification_id
        WHERE n.user_id IS DISTINCT FROM a.recipient_id
            OR n.title IS DISTINCT FROM a.title
            OR n.message IS DISTINCT FROM a.message
            OR n.type IS DISTINCT FROM 'info'
            OR n.link IS DISTINCT FROM a.link
    ) THEN
        RAISE EXCEPTION 'NOTIFICATION_DISPATCH_INBOX_CONFLICT';
    END IF;
END $$;

-- Keep exact tenant-local evidence, including intentionally absent inbox rows.
-- Inbox creation timestamps differ from enqueue timestamps and are not remapped.
CREATE TEMP TABLE notification_dispatch_expected ON COMMIT DROP AS
SELECT id, notification_id, recipient_id, 'attendance'::text source_kind,
    id source_id, dedup_key, title, message, 'info'::text type, link,
    created_at, stored_at, published_at, next_attempt_at, attempts,
    NULL::uuid lease_token
FROM attendance_notifications;
CREATE TEMP TABLE notification_dispatch_ledger_expected ON COMMIT DROP AS
SELECT id, academic_term_id, recipient_id, student_id, dedup_key,
    notification_id, created_at FROM attendance_notifications;
CREATE TEMP TABLE notification_dispatch_inbox_expected ON COMMIT DROP AS
SELECT to_jsonb(n) value FROM notifications n;

INSERT INTO notification_dispatch_jobs (
    id, notification_id, recipient_id, source_kind, source_id, dedup_key,
    title, message, type, link, created_at, stored_at, published_at,
    next_attempt_at, attempts, lease_token
)
SELECT id, notification_id, recipient_id, source_kind, source_id, dedup_key,
    title, message, type, link, created_at, stored_at, published_at,
    next_attempt_at, attempts, lease_token
FROM notification_dispatch_expected;

DO $$
BEGIN
    IF EXISTS (SELECT * FROM notification_dispatch_expected
        EXCEPT SELECT * FROM notification_dispatch_jobs)
        OR EXISTS (SELECT * FROM notification_dispatch_jobs
        EXCEPT SELECT * FROM notification_dispatch_expected)
        OR (SELECT count(*) FROM notification_dispatch_expected)
            <> (SELECT count(*) FROM notification_dispatch_jobs)
        OR EXISTS (SELECT * FROM notification_dispatch_ledger_expected EXCEPT
            SELECT id, academic_term_id, recipient_id, student_id, dedup_key,
                notification_id, created_at FROM attendance_notifications)
        OR EXISTS (SELECT id, academic_term_id, recipient_id, student_id, dedup_key,
                notification_id, created_at FROM attendance_notifications
            EXCEPT SELECT * FROM notification_dispatch_ledger_expected)
        OR EXISTS (SELECT * FROM notification_dispatch_inbox_expected
            EXCEPT SELECT to_jsonb(n) FROM notifications n)
        OR EXISTS (SELECT to_jsonb(n) FROM notifications n
            EXCEPT SELECT * FROM notification_dispatch_inbox_expected) THEN
        RAISE EXCEPTION 'NOTIFICATION_DISPATCH_PRESERVATION_FAILED';
    END IF;
END $$;

DROP INDEX attendance_notification_due_idx;
ALTER TABLE attendance_notifications
    DROP COLUMN title,
    DROP COLUMN message,
    DROP COLUMN link,
    DROP COLUMN stored_at,
    DROP COLUMN published_at,
    DROP COLUMN next_attempt_at,
    DROP COLUMN attempts;

DO $$
BEGIN
    IF EXISTS (SELECT * FROM notification_dispatch_ledger_expected
        EXCEPT SELECT * FROM attendance_notifications)
        OR EXISTS (SELECT * FROM attendance_notifications
        EXCEPT SELECT * FROM notification_dispatch_ledger_expected)
        OR EXISTS (SELECT * FROM notification_dispatch_expected
        EXCEPT SELECT * FROM notification_dispatch_jobs)
        OR EXISTS (SELECT * FROM notification_dispatch_jobs
        EXCEPT SELECT * FROM notification_dispatch_expected)
        OR EXISTS (SELECT * FROM notification_dispatch_inbox_expected
            EXCEPT SELECT to_jsonb(n) FROM notifications n)
        OR EXISTS (SELECT to_jsonb(n) FROM notifications n
            EXCEPT SELECT * FROM notification_dispatch_inbox_expected) THEN
        RAISE EXCEPTION 'NOTIFICATION_DISPATCH_PRESERVATION_FAILED';
    END IF;
END $$;
