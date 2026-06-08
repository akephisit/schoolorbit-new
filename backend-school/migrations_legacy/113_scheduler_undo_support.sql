-- Migration 113: Phase G — Undo support
-- ติด job_id ให้ entries ที่ scheduler insert → ลบกลับได้

ALTER TABLE academic_timetable_entries
    ADD COLUMN IF NOT EXISTS scheduler_job_id UUID
        REFERENCES timetable_scheduling_jobs(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_entries_scheduler_job
    ON academic_timetable_entries(scheduler_job_id)
    WHERE scheduler_job_id IS NOT NULL;

COMMENT ON COLUMN academic_timetable_entries.scheduler_job_id IS
    'ถ้าไม่ NULL = entry ถูก insert จาก auto-scheduler job นี้ — ใช้สำหรับ undo';
