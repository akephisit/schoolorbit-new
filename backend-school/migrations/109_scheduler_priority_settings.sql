-- Migration 109: Scheduler — instructor priority + scheduler global settings
-- Phase A ของ AUTO_SCHEDULER_REDESIGN
-- หมายเหตุ: ใช้ชื่อ scheduler_settings ไม่ใช่ school_settings เพราะ school_settings
-- มีอยู่แล้ว (migration 063) สำหรับ logo/branding

-- 1. Priority ต่อครู (สำหรับ sort order ใน scheduler)
ALTER TABLE instructor_preferences
    ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 100;

CREATE INDEX IF NOT EXISTS idx_instructor_prefs_year_priority
    ON instructor_preferences(academic_year_id, priority);

COMMENT ON COLUMN instructor_preferences.priority IS
    'ลำดับการ assign (1 = สำคัญสุด — ได้คาบดี ๆ ก่อน). default 100';

-- 2. Scheduler-specific key-value settings
CREATE TABLE IF NOT EXISTS scheduler_settings (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO scheduler_settings (key, value) VALUES
    ('default_max_consecutive', '4'::jsonb)
ON CONFLICT (key) DO NOTHING;

COMMENT ON TABLE scheduler_settings IS 'Key-value config ของ auto-scheduler';
COMMENT ON COLUMN scheduler_settings.key IS 'ชื่อ setting (snake_case)';
