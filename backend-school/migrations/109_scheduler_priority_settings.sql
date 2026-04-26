-- Migration 109: Scheduler — instructor priority + global settings
-- Phase A ของ AUTO_SCHEDULER_REDESIGN

-- 1. Priority ต่อครู (สำหรับ sort order ใน scheduler)
ALTER TABLE instructor_preferences
    ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 100;

CREATE INDEX IF NOT EXISTS idx_instructor_prefs_year_priority
    ON instructor_preferences(academic_year_id, priority);

COMMENT ON COLUMN instructor_preferences.priority IS
    'ลำดับการ assign (1 = สำคัญสุด — ได้คาบดี ๆ ก่อน). default 100';

-- 2. Global school settings (key-value)
CREATE TABLE IF NOT EXISTS school_settings (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO school_settings (key, value) VALUES
    ('default_max_consecutive', '4'::jsonb)
ON CONFLICT (key) DO NOTHING;

COMMENT ON TABLE school_settings IS 'Key-value config ระดับโรงเรียน';
COMMENT ON COLUMN school_settings.key IS 'ชื่อ setting (snake_case)';
