-- Migration 042: Create timetable locked slots table
-- Pre-assigned timetable slots that cannot be changed by auto-scheduler

CREATE TABLE timetable_locked_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,
    
    -- Scope (กำหนดขอบเขต)
    scope_type VARCHAR(20) NOT NULL,
    -- 'CLASSROOM': เฉพาะห้องเรียนที่ระบุ
    -- 'GRADE_LEVEL': ทั้งชั้น (เช่น ทุกห้อง ม.4)
    -- 'ALL_SCHOOL': ทั้งโรงเรียน
    
    scope_ids JSONB, -- classroom_ids or grade_level_ids (null if ALL_SCHOOL)
    -- Format: ["uuid1", "uuid2"] or null
    
    -- Subject (วิชาที่ lock)
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    
    -- Time (เวลาที่ lock)
    day_of_week VARCHAR(3) NOT NULL,
    period_ids JSONB NOT NULL, -- Array of period UUIDs
    -- Format: ["period_uuid1", "period_uuid2"]
    
    -- Optional: Room (ถ้าต้องการระบุห้อง)
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
    
    -- Optional: Instructor (ถ้าต้องการระบุครู)
    instructor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Metadata
    reason TEXT,
    locked_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_scope CHECK (
        scope_type IN ('CLASSROOM', 'GRADE_LEVEL', 'ALL_SCHOOL')
    ),
    CONSTRAINT valid_day CHECK (
        day_of_week IN ('MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN')
    ),
    CONSTRAINT valid_scope_ids CHECK (
        (scope_type = 'ALL_SCHOOL' AND scope_ids IS NULL) OR
        (scope_type != 'ALL_SCHOOL' AND scope_ids IS NOT NULL)
    )
);

CREATE INDEX idx_locked_slots_semester ON timetable_locked_slots(academic_semester_id);
CREATE INDEX idx_locked_slots_subject ON timetable_locked_slots(subject_id);
CREATE INDEX idx_locked_slots_day ON timetable_locked_slots(day_of_week);
CREATE INDEX idx_locked_slots_scope ON timetable_locked_slots(scope_type);

-- GIN index for JSONB queries
CREATE INDEX idx_locked_slots_scope_ids ON timetable_locked_slots USING gin(scope_ids);
CREATE INDEX idx_locked_slots_period_ids ON timetable_locked_slots USING gin(period_ids);

COMMENT ON TABLE timetable_locked_slots IS 'ช่วงเวลาที่ถูก lock ไว้ล่วงหน้า ห้ามแก้ไข';
COMMENT ON COLUMN timetable_locked_slots.scope_type IS 'ขอบเขต: CLASSROOM, GRADE_LEVEL, ALL_SCHOOL';
COMMENT ON COLUMN timetable_locked_slots.scope_ids IS 'IDs ของห้อง/ชั้น (null ถ้า ALL_SCHOOL)';
COMMENT ON COLUMN timetable_locked_slots.reason IS 'เหตุผลที่ lock (เช่น โรงเรียนกำหนด, นโยบาย)';
