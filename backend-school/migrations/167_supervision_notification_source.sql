ALTER TABLE notification_receipts
    DROP CONSTRAINT notification_dispatch_jobs_source_kind_check,
    ADD CONSTRAINT notification_receipts_source_kind_check
        CHECK (source_kind IN ('direct', 'attendance', 'process', 'calendar_reminder', 'supervision_action'));
