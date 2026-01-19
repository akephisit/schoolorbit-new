-- ===================================================================
-- Migration 023: Curriculum Basics (Subjects & Groups)
-- ===================================================================

-- 1. Subject Groups (กลุ่มสาระการเรียนรู้)
CREATE TABLE IF NOT EXISTS subject_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(20) NOT NULL UNIQUE,      -- รหัสกลุ่มสาระ (เช่น TH, MA, SC)
    name_th VARCHAR(200) NOT NULL,         -- ชื่อไทย (ภาษาไทย, คณิตศาสตร์)
    name_en VARCHAR(200) NOT NULL,         -- ชื่ออังกฤษ (Thai Language, Mathematics)
    display_order INTEGER DEFAULT 0,       -- ลำดับการแสดงผลในใบปพ.
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed Default Subject Groups (8 กลุ่มสาระ + 1 กิจกรรมพัฒนาผู้เรียน)
INSERT INTO subject_groups (code, name_th, name_en, display_order) VALUES
    ('TH', 'ภาษาไทย', 'Thai Language', 1),
    ('MA', 'คณิตศาสตร์', 'Mathematics', 2),
    ('SC', 'วิทยาศาสตร์และเทคโนโลยี', 'Science and Technology', 3),
    ('SO', 'สังคมศึกษา ศาสนา และวัฒนธรรม', 'Social Studies, Religion and Culture', 4),
    ('HP', 'สุขศึกษาและพลศึกษา', 'Health and Physical Education', 5),
    ('AR', 'ศิลปะ', 'Arts', 6),
    ('OC', 'การงานอาชีพ', 'Occupations', 7),
    ('EN', 'ภาษาต่างประเทศ', 'Foreign Languages', 8),
    ('AC', 'กิจกรรมพัฒนาผู้เรียน', 'Learner Development Activities', 9)
ON CONFLICT (code) DO NOTHING;


-- 2. Subjects (รายวิชาพื้นฐาน/เพิ่มเติม)
CREATE TABLE IF NOT EXISTS subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Identification
    code VARCHAR(20) NOT NULL,             -- รหัสวิชา (ท21101) - *Not unique globally because diff years might reuse*
    academic_year_start INTEGER,           -- ปีการศึกษาที่เริ่มใช้หลักสูตรนี้ (optional, for versioning)
    
    -- Names
    name_th VARCHAR(200) NOT NULL,
    name_en VARCHAR(200),
    
    -- Properties
    credit DECIMAL(3,1) NOT NULL DEFAULT 0.0,
    hours_per_semester INTEGER,            -- 40, 60, 80
    
    -- Classification
    type VARCHAR(50) NOT NULL,             -- BASIC (พื้นฐาน), ADDITIONAL (เพิ่มเติม), ACTIVITY (กิจกรรม)
    group_id UUID REFERENCES subject_groups(id),
    
    -- Grade Level Scope (เพื่อความง่ายในการ Filter ตามที่ขอ)
    level_scope VARCHAR(50),               -- 'P1', 'P2', ... 'M1', 'M4', 'ALL'
    
    description TEXT,
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Constraints
    -- รหัสวิชาควรไม่ซ้ำกัน (แต่อาจจะซ้ำได้ถ้าอยู่คนละหลักสูตร ในอนาคตอาจต้องเพิ่ม curriculum_id)
    -- เบื้องต้นให้ Unique ไว้ก่อนเพื่อความง่าย ถ้าระบบซับซ้อนขึ้นค่อยปลด
    CONSTRAINT subjects_code_key UNIQUE (code) 
);

-- Indices
CREATE INDEX idx_subjects_code ON subjects(code);
CREATE INDEX idx_subjects_group ON subjects(group_id);
CREATE INDEX idx_subjects_type ON subjects(type);
CREATE INDEX idx_subjects_level ON subjects(level_scope);

-- 3. Triggers for updated_at
DROP TRIGGER IF EXISTS update_subject_groups_updated_at ON subject_groups;
CREATE TRIGGER update_subject_groups_updated_at
    BEFORE UPDATE ON subject_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_subjects_updated_at ON subjects;
CREATE TRIGGER update_subjects_updated_at
    BEFORE UPDATE ON subjects
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Comments
COMMENT ON TABLE subject_groups IS 'กลุ่มสาระการเรียนรู้ (Learning Areas)';
COMMENT ON TABLE subjects IS 'รายวิชา (Courses/Subjects)';
COMMENT ON COLUMN subjects.type IS 'ประเภทวิชา: BASIC, ADDITIONAL, ACTIVITY';
