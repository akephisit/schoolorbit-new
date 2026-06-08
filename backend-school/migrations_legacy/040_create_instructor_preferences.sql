-- Migration 040: Create instructor preferences table
-- Store instructor time preferences and unavailability

CREATE TABLE instructor_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- Unavailable time slots (HARD constraint)
    hard_unavailable_slots JSONB DEFAULT '[]'::jsonb,
    -- Format: [{"day": "MON", "period_id": "uuid"}, ...]
    
    -- Preferred time slots (SOFT constraint)
    preferred_slots JSONB DEFAULT '[]'::jsonb,
    -- Format: [{"day": "MON", "period_id": "uuid"}, ...]
    
    -- Daily load preferences
    max_periods_per_day INTEGER DEFAULT 7,
    min_periods_per_day INTEGER DEFAULT 0,
    
    -- Day preferences
    preferred_days JSONB DEFAULT '[]'::jsonb,
    -- Format: ["MON", "TUE", ...]
    
    avoid_days JSONB DEFAULT '[]'::jsonb,
    -- Format: ["SAT", "SUN"]
    
    -- Notes
    notes TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- One preference record per instructor per year
    CONSTRAINT unique_instructor_year_pref UNIQUE(instructor_id, academic_year_id)
);

CREATE INDEX idx_instructor_prefs_instructor ON instructor_preferences(instructor_id);
CREATE INDEX idx_instructor_prefs_year ON instructor_preferences(academic_year_id);

COMMENT ON TABLE instructor_preferences IS 'ความต้องการและข้อจำกัดด้านเวลาของครู';
COMMENT ON COLUMN instructor_preferences.hard_unavailable_slots IS 'ช่วงเวลาที่ครูไม่ว่างเด็ดขาด (HARD constraint)';
COMMENT ON COLUMN instructor_preferences.preferred_slots IS 'ช่วงเวลาที่ครูอยากสอน (SOFT constraint)';
COMMENT ON COLUMN instructor_preferences.max_periods_per_day IS 'จำนวนคาบสูงสุดต่อวัน';
COMMENT ON COLUMN instructor_preferences.preferred_days IS 'วันที่ชอบสอน';
COMMENT ON COLUMN instructor_preferences.avoid_days IS 'วันที่อยากหลีกเลี่ยง';
