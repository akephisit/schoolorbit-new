-- ===================================================================
-- Migration 038: Study Plans (หลักสูตรสถานศึกษา / แผนการเรียน)
-- ===================================================================

-- 1. Study Plans Master (แม่บทแผนการเรียน)
-- เช่น "วิทยาศาสตร์-คณิตศาสตร์", "ศิลป์-ภาษา", "ภาษา-การงาน"
CREATE TABLE IF NOT EXISTS study_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) NOT NULL UNIQUE,           -- รหัสแผน เช่น "SCI-MATH", "ART-LANG"
    name_th VARCHAR(200) NOT NULL,              -- ชื่อไทย เช่น "วิทยาศาสตร์-คณิตศาสตร์"
    name_en VARCHAR(200),                       -- ชื่ออังกฤษ เช่น "Science-Mathematics Program"
    description TEXT,                           -- คำอธิบายแผนการเรียน
    level_scope VARCHAR(50),                    -- ระดับชั้นที่ใช้ เช่น 'M1-M3', 'M4-M6', 'ALL'
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_study_plans_code ON study_plans(code);
CREATE INDEX idx_study_plans_active ON study_plans(is_active);

-- 2. Study Plan Versions (ฉบับปรับปรุงหลักสูตร)
-- เก็บ version ของแผนการเรียนตามปีการศึกษาที่เริ่มใช้
-- เพื่อให้นักเรียนแต่ละรุ่นใช้หลักสูตรที่ถูกต้อง ไม่ปนกัน
CREATE TABLE IF NOT EXISTS study_plan_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    study_plan_id UUID NOT NULL REFERENCES study_plans(id) ON DELETE CASCADE,
    version_name VARCHAR(100) NOT NULL,         -- เช่น "ฉบับปี 2568", "ฉบับปรับปรุง 2569"
    
    -- ปีการศึกษาที่เริ่มใช้หลักสูตรนี้
    start_academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    
    -- ปีการศึกษาที่สิ้นสุดการใช้ (NULL = ยังใช้อยู่)
    end_academic_year_id UUID REFERENCES academic_years(id) ON DELETE RESTRICT,
    
    description TEXT,
    is_active BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- แผนเดียวกัน ไม่ควรมี version ที่ทับซ้อนกันในช่วงปีเดียวกัน
    CONSTRAINT unique_plan_version UNIQUE(study_plan_id, version_name)
);

CREATE INDEX idx_plan_versions_plan ON study_plan_versions(study_plan_id);
CREATE INDEX idx_plan_versions_start_year ON study_plan_versions(start_academic_year_id);
CREATE INDEX idx_plan_versions_active ON study_plan_versions(is_active);

-- 3. Study Plan Subjects (โครงสร้างรายวิชาในแผนการเรียน)
-- กำหนดว่าแต่ละชั้นปี แต่ละเทอม ต้องเรียนวิชาอะไรบ้าง
CREATE TABLE IF NOT EXISTS study_plan_subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    study_plan_version_id UUID NOT NULL REFERENCES study_plan_versions(id) ON DELETE CASCADE,
    
    -- ระดับชั้นและเทอม
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    term VARCHAR(20) NOT NULL,                  -- "1", "2", "SUMMER"
    
    -- วิชาที่ต้องเรียน (อ้างอิงด้วย subject_id แต่เก็บ code ไว้เพื่อความยืดหยุ่น)
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    subject_code VARCHAR(20) NOT NULL,          -- Redundant but useful for version stability
    
    -- ลำดับการแสดงผล
    display_order INTEGER DEFAULT 0,
    
    -- ข้อมูลเพิ่มเติม (เช่น บังคับ/เลือก)
    is_required BOOLEAN DEFAULT true,           -- วิชาบังคับหรือไม่
    metadata JSONB DEFAULT '{}'::jsonb,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- แผนเดียวกัน ชั้นเดียวกัน เทอมเดียวกัน ไม่ควรมีวิชาซ้ำ
    CONSTRAINT unique_plan_subject UNIQUE(study_plan_version_id, grade_level_id, term, subject_id)
);

CREATE INDEX idx_plan_subjects_version ON study_plan_subjects(study_plan_version_id);
CREATE INDEX idx_plan_subjects_grade ON study_plan_subjects(grade_level_id);
CREATE INDEX idx_plan_subjects_subject ON study_plan_subjects(subject_id);
CREATE INDEX idx_plan_subjects_code ON study_plan_subjects(subject_code);

-- 4. Add study_plan_version_id to class_rooms
-- เชื่อมห้องเรียนเข้ากับแผนการเรียน
ALTER TABLE class_rooms 
ADD COLUMN IF NOT EXISTS study_plan_version_id UUID NOT NULL REFERENCES study_plan_versions(id) ON DELETE RESTRICT;

CREATE INDEX idx_classrooms_plan_version ON class_rooms(study_plan_version_id);

-- 5. Triggers for updated_at
CREATE TRIGGER update_study_plans_updated_at 
    BEFORE UPDATE ON study_plans 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_study_plan_versions_updated_at 
    BEFORE UPDATE ON study_plan_versions 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_study_plan_subjects_updated_at 
    BEFORE UPDATE ON study_plan_subjects 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();

-- Comments
COMMENT ON TABLE study_plans IS 'แม่บทแผนการเรียน (Study Plan Master)';
COMMENT ON TABLE study_plan_versions IS 'ฉบับปรับปรุงหลักสูตร (Curriculum Versions for different cohorts)';
COMMENT ON TABLE study_plan_subjects IS 'โครงสร้างรายวิชาในแผนการเรียน (Subjects in each plan by grade/term)';
COMMENT ON COLUMN class_rooms.study_plan_version_id IS 'แผนการเรียนที่ห้องนี้ใช้ (Required - ห้องเรียนทุกห้องต้องใช้หลักสูตร)';
