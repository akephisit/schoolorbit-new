-- ============================================
-- Timetable Management System
-- ============================================

-- 1. Academic Periods (คาบเวลา)
-- ตารางนี้เก็บข้อมูลคาบเวลามาตรฐานของโรงเรียนในแต่ละปีการศึกษา
CREATE TABLE IF NOT EXISTS academic_periods (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    
    -- ข้อมูลคาบ
    name VARCHAR(100) NOT NULL,           -- เช่น "คาบที่ 1", "พักเที่ยง", "โฮมรูม"
    start_time TIME NOT NULL,              -- เวลาเริ่ม เช่น 08:30
    end_time TIME NOT NULL,                -- เวลาจบ เช่น 09:20
    
    -- ประเภทคาบ
    type VARCHAR(20) NOT NULL CHECK (type IN ('TEACHING', 'BREAK', 'ACTIVITY', 'HOMEROOM')),
    -- TEACHING: คาบเรียนปกติ
    -- BREAK: คาบพัก
    -- ACTIVITY: คาบกิจกรรม/ชุมนุม
    -- HOMEROOM: คาบโฮมรูม/ปกครอง
    
    -- ลำดับการแสดงผล
    order_index INTEGER NOT NULL,
    
    -- วันที่ใช้งาน (ถ้าไม่ระบุ = ใช้ทุกวัน)
    applicable_days VARCHAR(50),           -- เช่น "MON,TUE,WED,THU,FRI" หรือ null = ทุกวัน
    
    -- Metadata
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    CONSTRAINT valid_time_range CHECK (end_time > start_time),
    CONSTRAINT unique_period_per_year UNIQUE (academic_year_id, order_index)
);

-- Indexes for performance
CREATE INDEX idx_academic_periods_year ON academic_periods(academic_year_id);
CREATE INDEX idx_academic_periods_type ON academic_periods(type);
CREATE INDEX idx_academic_periods_order ON academic_periods(academic_year_id, order_index);

-- ============================================

-- 2. Academic Timetable Entries (ตารางสอน)
-- ตารางนี้เก็บข้อมูลการลงตารางสอนจริง (เชื่อมโยงแผนการเรียนกับวันเวลา)
CREATE TABLE IF NOT EXISTS academic_timetable_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- เชื่อมโยงกับแผนการเรียน (แบบที่ทำไปแล้ว)
    classroom_course_id UUID NOT NULL REFERENCES classroom_courses(id) ON DELETE CASCADE,
    -- จากตารางนี้เราจะได้: classroom, subject, semester, instructor
    
    -- วันและเวลา
    day_of_week VARCHAR(3) NOT NULL CHECK (day_of_week IN ('MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN')),
    period_id UUID NOT NULL REFERENCES academic_periods(id) ON DELETE RESTRICT,
    
    -- ห้องเรียน (Override)
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
    -- NULL = ใช้ห้องประจำชั้น
    -- มีค่า = เรียนห้องพิเศษ (เช่น ห้องวิทยาศาสตร์, ห้องคอมพิวเตอร์)
    
    -- Note/Remark
    note TEXT,                             -- เช่น "สอบกลางภาค", "กิจกรรมพิเศษ"
    
    -- Status
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    
    -- Constraints: ป้องกันการลงตารางซ้ำในคาบเดียวกัน
    CONSTRAINT unique_entry_per_slot UNIQUE (classroom_course_id, day_of_week, period_id)
);

-- Indexes for performance และ conflict detection
CREATE INDEX idx_timetable_classroom_course ON academic_timetable_entries(classroom_course_id);
CREATE INDEX idx_timetable_day_period ON academic_timetable_entries(day_of_week, period_id);
CREATE INDEX idx_timetable_room ON academic_timetable_entries(room_id) WHERE room_id IS NOT NULL;

-- Index สำหรับการ query ตารางสอนของห้องเรียน
CREATE INDEX idx_timetable_by_classroom ON academic_timetable_entries(classroom_course_id, day_of_week, period_id);

-- ============================================

-- Comments
COMMENT ON TABLE academic_periods IS 'ตารางคาบเวลามาตรฐานของโรงเรียนในแต่ละปีการศึกษา';
COMMENT ON TABLE academic_timetable_entries IS 'ตารางสอนจริง เชื่อมโยงแผนการเรียนกับวันและเวลา';

COMMENT ON COLUMN academic_periods.type IS 'ประเภทคาบ: TEACHING=เรียน, BREAK=พัก, ACTIVITY=กิจกรรม, HOMEROOM=โฮมรูม';
COMMENT ON COLUMN academic_timetable_entries.classroom_course_id IS 'FK ไปยัง classroom_courses (มัดรวม classroom+subject+semester+instructor)';
COMMENT ON COLUMN academic_timetable_entries.room_id IS 'ห้องเรียนที่ใช้ (NULL=ห้องประจำชั้น, มีค่า=ห้องพิเศษ)';
