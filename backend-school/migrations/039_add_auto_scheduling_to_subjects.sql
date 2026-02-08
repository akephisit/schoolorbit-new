-- Migration 039: Add auto-scheduling support to subjects table
-- Add consecutive period requirements and other auto-scheduling related fields

-- Add consecutive period requirements
ALTER TABLE subjects 
ADD COLUMN IF NOT EXISTS min_consecutive_periods INTEGER DEFAULT 1,
ADD COLUMN IF NOT EXISTS max_consecutive_periods INTEGER DEFAULT 2,
ADD COLUMN IF NOT EXISTS allow_single_period BOOLEAN DEFAULT true;

-- Add period requirements (alternative to hours/credit)
ALTER TABLE subjects
ADD COLUMN IF NOT EXISTS periods_per_week INTEGER;

-- Add time preference
ALTER TABLE subjects
ADD COLUMN IF NOT EXISTS preferred_time_of_day VARCHAR(20) CHECK (preferred_time_of_day IN ('MORNING', 'AFTERNOON', 'ANYTIME', NULL));

-- Add room requirements
ALTER TABLE subjects
ADD COLUMN IF NOT EXISTS required_room_type VARCHAR(50);

-- Comments
COMMENT ON COLUMN subjects.min_consecutive_periods IS 'จำนวนคาบต่อเนื่องขั้นต่ำต่อวัน (1=ไม่บังคับ, 2+=ต้องติดกัน)';
COMMENT ON COLUMN subjects.max_consecutive_periods IS 'จำนวนคาบต่อเนื่องสูงสุดต่อวัน';
COMMENT ON COLUMN subjects.allow_single_period IS 'อนุญาตให้มี 1 คาบเดี่ยวได้ไหม (สำหรับคาบที่เหลือ)';
COMMENT ON COLUMN subjects.periods_per_week IS 'จำนวนคาบต่อสัปดาห์ (ใช้สำหรับจัดตาราง)';
COMMENT ON COLUMN subjects.preferred_time_of_day IS 'ช่วงเวลาที่เหมาะสมสำหรับวิชานี้ (MORNING, AFTERNOON, ANYTIME)';
COMMENT ON COLUMN subjects.required_room_type IS 'ประเภทห้องที่ต้องการ (LAB, FIELD, COMPUTER, etc)';

-- Default values for common subject types
-- PE: Must have 2 consecutive periods
UPDATE subjects SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 2,
    allow_single_period = true,
    preferred_time_of_day = 'AFTERNOON'
WHERE subject_type = 'PE';

-- CORE subjects: No strict requirements
UPDATE subjects SET 
    min_consecutive_periods = 1,
    max_consecutive_periods = 2,
    allow_single_period = true,
    preferred_time_of_day = 'MORNING'
WHERE subject_type = 'CORE';

-- ELECTIVE: Flexible
UPDATE subjects SET 
    min_consecutive_periods = 1,
    max_consecutive_periods = 2,
    allow_single_period = true,
    preferred_time_of_day = 'ANYTIME'
WHERE subject_type = 'ELECTIVE';

-- ACTIVITY: Usually 2 consecutive
UPDATE subjects SET 
    min_consecutive_periods = 2,
    max_consecutive_periods = 2,
    allow_single_period = false,
    preferred_time_of_day = 'AFTERNOON'
WHERE subject_type = 'ACTIVITY';
