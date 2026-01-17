-- ===================================================================
-- Academic Management Tables
-- ===================================================================

-- 1. Academic Years (ปีการศึกษา)
CREATE TABLE IF NOT EXISTS academic_years (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    year INTEGER NOT NULL, -- e.g., 2567
    name VARCHAR(100) NOT NULL, -- e.g., "ปีการศึกษา 2567"
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    is_active BOOLEAN DEFAULT false, -- Current active year
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(year)
);

-- Ensure only one active academic year
CREATE UNIQUE INDEX idx_academic_years_active ON academic_years(is_active) WHERE is_active = true;

-- 2. Semesters (ภาคเรียน)
CREATE TABLE IF NOT EXISTS semesters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    term VARCHAR(20) NOT NULL, -- e.g., "1", "2", "Summer"
    name VARCHAR(100) NOT NULL, -- e.g., "ภาคเรียนที่ 1", "ภาคเรียนที่ 2"
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    is_active BOOLEAN DEFAULT false, -- Current active semester
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(academic_year_id, term)
);

-- Ensure only one active semester
CREATE UNIQUE INDEX idx_semesters_active ON semesters(is_active) WHERE is_active = true;

-- 3. Grade Levels (ระดับชั้น)
CREATE TABLE IF NOT EXISTS grade_levels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(20) NOT NULL UNIQUE, -- e.g., "P1", "P2", "M1", "M2"
    name VARCHAR(100) NOT NULL, -- e.g., "ประถมศึกษาปีที่ 1", "มัธยมศึกษาปีที่ 1"
    short_name VARCHAR(20) NOT NULL, -- e.g., "ป.1", "ม.1"
    level_order INTEGER NOT NULL, -- For sorting and promotion logic (e.g. 1, 2, 3...)
    next_grade_level_id UUID REFERENCES grade_levels(id), -- Points to next level (for promotion)
    is_active BOOLEAN DEFAULT true,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_grade_levels_order ON grade_levels(level_order);

-- 4. Classrooms (ห้องเรียน)
CREATE TABLE IF NOT EXISTS class_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) NOT NULL, -- e.g. "67-M1-1" (Year-Level-Room)
    name VARCHAR(100) NOT NULL, -- e.g., "ม.1/1"
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE CASCADE,
    room_number VARCHAR(20), -- e.g., "1", "2", "3" (just the room number part)
    advisor_id UUID REFERENCES staff_info(id), -- Primary Advisor
    co_advisor_id UUID REFERENCES staff_info(id), -- Co-Advisor
    is_active BOOLEAN DEFAULT true,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(academic_year_id, grade_level_id, room_number)
);

CREATE INDEX idx_class_rooms_year_level ON class_rooms(academic_year_id, grade_level_id);

-- 5. Student Enrollment History (ประวัตินักเรียนในห้องเรียน)
CREATE TABLE IF NOT EXISTS student_class_enrollments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    class_room_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE CASCADE,
    enrollment_date DATE NOT NULL DEFAULT CURRENT_DATE,
    status VARCHAR(20) DEFAULT 'active', -- active, transferred, dropped, completed
    enrollment_type VARCHAR(20) DEFAULT 'regular', -- regular, transferred_in, repeated
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, class_room_id)
);

CREATE INDEX idx_enrollments_student ON student_class_enrollments(student_id);
CREATE INDEX idx_enrollments_classroom ON student_class_enrollments(class_room_id);

-- Add updated_at triggers
CREATE TRIGGER update_academic_years_updated_at BEFORE UPDATE ON academic_years FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_semesters_updated_at BEFORE UPDATE ON semesters FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_grade_levels_updated_at BEFORE UPDATE ON grade_levels FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_class_rooms_updated_at BEFORE UPDATE ON class_rooms FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_student_class_enrollments_updated_at BEFORE UPDATE ON student_class_enrollments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Seed Default Grade Levels (Common Thai Structure)
INSERT INTO grade_levels (code, name, short_name, level_order) VALUES
('K1', 'อนุบาล 1', 'อ.1', 1),
('K2', 'อนุบาล 2', 'อ.2', 2),
('K3', 'อนุบาล 3', 'อ.3', 3),
('P1', 'ประถมศึกษาปีที่ 1', 'ป.1', 10),
('P2', 'ประถมศึกษาปีที่ 2', 'ป.2', 11),
('P3', 'ประถมศึกษาปีที่ 3', 'ป.3', 12),
('P4', 'ประถมศึกษาปีที่ 4', 'ป.4', 13),
('P5', 'ประถมศึกษาปีที่ 5', 'ป.5', 14),
('P6', 'ประถมศึกษาปีที่ 6', 'ป.6', 15),
('M1', 'มัธยมศึกษาปีที่ 1', 'ม.1', 20),
('M2', 'มัธยมศึกษาปีที่ 2', 'ม.2', 21),
('M3', 'มัธยมศึกษาปีที่ 3', 'ม.3', 22),
('M4', 'มัธยมศึกษาปีที่ 4', 'ม.4', 23),
('M5', 'มัธยมศึกษาปีที่ 5', 'ม.5', 24),
('M6', 'มัธยมศึกษาปีที่ 6', 'ม.6', 25)
ON CONFLICT (code) DO NOTHING;

-- Link next grade levels (Basic logic)
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'K2') WHERE code = 'K1';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'K3') WHERE code = 'K2';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P1') WHERE code = 'K3';

UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P2') WHERE code = 'P1';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P3') WHERE code = 'P2';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P4') WHERE code = 'P3';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P5') WHERE code = 'P4';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'P6') WHERE code = 'P5';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M1') WHERE code = 'P6';

UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M2') WHERE code = 'M1';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M3') WHERE code = 'M2';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M4') WHERE code = 'M3';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M5') WHERE code = 'M4';
UPDATE grade_levels SET next_grade_level_id = (SELECT id FROM grade_levels WHERE code = 'M6') WHERE code = 'M5';
