-- Migration 041: Create instructor room assignments table
-- Store fixed room assignments for instructors

CREATE TABLE instructor_room_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- Priority
    is_preferred BOOLEAN DEFAULT false,  -- ชอบใช้ห้องนี้ (soft)
    is_required BOOLEAN DEFAULT false,   -- ต้องใช้ห้องนี้ (hard)
    
    -- Conditions
    for_subjects JSONB DEFAULT '[]'::jsonb, -- ระบุเฉพาะวิชา [], empty=ทุกวิชา
    -- Format: ["subject_code1", "subject_code2"] or []
    
    -- Notes
    reason TEXT,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT check_priority CHECK (is_preferred OR is_required),
    -- Unique: instructor can have multiple room assignments for different subjects
    CONSTRAINT unique_instructor_room_year UNIQUE(instructor_id, room_id, academic_year_id)
);

CREATE INDEX idx_instructor_room_instructor ON instructor_room_assignments(instructor_id);
CREATE INDEX idx_instructor_room_room ON instructor_room_assignments(room_id);
CREATE INDEX idx_instructor_room_year ON instructor_room_assignments(academic_year_id);

COMMENT ON TABLE instructor_room_assignments IS 'ห้องประจำของครู';
COMMENT ON COLUMN instructor_room_assignments.is_preferred IS 'ชอบใช้ห้องนี้ (SOFT constraint)';
COMMENT ON COLUMN instructor_room_assignments.is_required IS 'ต้องใช้ห้องนี้เท่านั้น (HARD constraint)';
COMMENT ON COLUMN instructor_room_assignments.for_subjects IS 'ระบุเฉพาะวิชา ([] = ทุกวิชา)';
