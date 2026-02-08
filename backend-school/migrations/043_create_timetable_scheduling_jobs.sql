-- Migration 043: Create timetable scheduling jobs table
-- Track auto-scheduling job status and results

CREATE TYPE scheduling_status AS ENUM (
    'PENDING',      -- รอการประมวลผล
    'RUNNING',      -- กำลังทำงาน
    'COMPLETED',    -- สำเร็จ
    'FAILED',       -- ล้มเหลว
    'CANCELLED'     -- ยกเลิก
);

CREATE TYPE scheduling_algorithm AS ENUM (
    'GREEDY',       -- เร็ว แต่คุณภาพปานกลาง
    'BACKTRACKING', -- ช้า แต่คุณภาพดี
    'HYBRID'        -- ผสม
);

CREATE TABLE timetable_scheduling_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,
    
    -- Target
    classroom_ids JSONB NOT NULL, -- Array of classroom UUIDs to schedule
    -- Format: ["classroom_uuid1", "classroom_uuid2"]
    
    -- Configuration
    algorithm scheduling_algorithm NOT NULL DEFAULT 'GREEDY',
    config JSONB DEFAULT '{}'::jsonb,
    -- Format: {
    --   "force_overwrite": true,
    --   "respect_preferences": true,
    --   "allow_partial": false,
    --   "min_quality_score": 70.0,
    --   "timeout_seconds": 300
    -- }
    
    -- Status
    status scheduling_status NOT NULL DEFAULT 'PENDING',
    progress INTEGER DEFAULT 0, -- 0-100
    
    -- Results
    quality_score DECIMAL(5, 2),
    scheduled_courses INTEGER DEFAULT 0,
    total_courses INTEGER DEFAULT 0,
    failed_courses JSONB DEFAULT '[]'::jsonb,
    -- Format: [{"subject_code": "MATH", "classroom": "M4/1", "reason": "No available slots"}]
    
    -- Timing
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    duration_seconds INTEGER,
    
    -- Error info
    error_message TEXT,
    
    -- Metadata
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_scheduling_jobs_semester ON timetable_scheduling_jobs(academic_semester_id);
CREATE INDEX idx_scheduling_jobs_status ON timetable_scheduling_jobs(status);
CREATE INDEX idx_scheduling_jobs_created_at ON timetable_scheduling_jobs(created_at DESC);
CREATE INDEX idx_scheduling_jobs_created_by ON timetable_scheduling_jobs(created_by);

-- GIN index for classroom_ids
CREATE INDEX idx_scheduling_jobs_classrooms ON timetable_scheduling_jobs USING gin(classroom_ids);

COMMENT ON TABLE timetable_scheduling_jobs IS 'งานจัดตารางอัตโนมัติ ติดตามสถานะและผลลัพธ์';
COMMENT ON COLUMN timetable_scheduling_jobs.classroom_ids IS 'รายการห้องเรียนที่ต้องการจัดตาราง';
COMMENT ON COLUMN timetable_scheduling_jobs.config IS 'การตั้งค่าเพิ่มเติม (JSON)';
COMMENT ON COLUMN timetable_scheduling_jobs.quality_score IS 'คะแนนคุณภาพของตาราง (0-100)';
COMMENT ON COLUMN timetable_scheduling_jobs.failed_courses IS 'รายการวิชาที่จัดไม่สำเร็จ';
