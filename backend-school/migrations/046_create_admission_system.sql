-- ===================================================================
-- Migration 046: Admission System (ระบบรับสมัครนักเรียน)
-- Description: สร้างตารางสำหรับระบบรับสมัครนักเรียนประจำปีการศึกษา
-- Date: 2026-03-05
-- ===================================================================

-- ===================================================================
-- 1. Admission Periods (รอบรับสมัคร)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    name VARCHAR(200) NOT NULL,                     -- ชื่อรอบรับสมัคร เช่น "รับสมัครนักเรียน ม.1 ปีการศึกษา 2568"
    description TEXT,
    open_date DATE NOT NULL,                        -- วันเปิดรับสมัคร
    close_date DATE NOT NULL,                       -- วันปิดรับสมัคร
    announcement_date DATE,                        -- วันประกาศผล
    confirmation_deadline DATE,                    -- deadline ยืนยันสิทธิ์
    status VARCHAR(20) NOT NULL DEFAULT 'draft'    -- draft | open | closed | announced | done
        CHECK (status IN ('draft', 'open', 'closed', 'announced', 'done')),
    
    -- เป้าหมายการรับ
    target_grade_level_ids UUID[] DEFAULT '{}',    -- ระดับชั้นที่รับสมัคร (array of grade_level UUIDs)
    capacity_per_class INT DEFAULT 0,              -- จำนวนนักเรียนต่อห้อง
    total_capacity INT DEFAULT 0,                  -- จำนวนทั้งหมด
    waitlist_capacity INT DEFAULT 0,               -- จำนวนรายชื่อสำรอง
    
    -- เอกสารที่ต้องใช้ (เก็บเป็น JSON config)
    required_documents JSONB DEFAULT '[]',         -- [{"key": "birth_cert", "label": "สูติบัตร", "required": true}]
    
    -- ค่าธรรมเนียม
    application_fee NUMERIC(10,2) DEFAULT 0,
    
    metadata JSONB DEFAULT '{}',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ===================================================================
-- 2. Admission Applications (ใบสมัคร)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_period_id UUID NOT NULL REFERENCES admission_periods(id) ON DELETE RESTRICT,
    application_number VARCHAR(20) UNIQUE NOT NULL,  -- เลขที่ใบสมัคร เช่น "ADM-2568-00001"
    
    -- ข้อมูลผู้สมัคร
    applicant_first_name VARCHAR(100) NOT NULL,
    applicant_last_name VARCHAR(100) NOT NULL,
    applicant_title VARCHAR(20),                     -- เด็กชาย, เด็กหญิง, นาย, นางสาว
    applicant_national_id VARCHAR(20),               -- เลขบัตรประชาชน
    applicant_date_of_birth DATE,
    applicant_gender VARCHAR(10),                    -- male | female
    applicant_nationality VARCHAR(50) DEFAULT 'ไทย',
    applicant_religion VARCHAR(50),
    applicant_blood_type VARCHAR(5),
    applicant_phone VARCHAR(20),
    applicant_email VARCHAR(200),
    applicant_address TEXT,
    applicant_photo_url TEXT,                        -- รูปถ่าย
    
    -- โรงเรียนเดิม
    previous_school VARCHAR(200),
    previous_grade VARCHAR(50),                      -- ชั้นที่กำลังเรียน/จบ
    previous_gpa NUMERIC(4,2),                       -- GPA
    
    -- ระดับชั้นที่สมัคร
    applying_grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
    applying_classroom_preference VARCHAR(100),     -- ความต้องการพิเศษ เช่น "EP", "ห้องพิเศษ"
    
    -- ผู้ปกครอง
    guardian_name VARCHAR(200),
    guardian_relationship VARCHAR(50),
    guardian_phone VARCHAR(20),
    guardian_email VARCHAR(200),
    guardian_occupation VARCHAR(100),
    guardian_national_id VARCHAR(20),
    
    -- สถานะ
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'reviewing', 'interview_scheduled', 'accepted', 'rejected', 'waitlisted', 'confirmed', 'cancelled')),
    
    -- หมายเหตุของเจ้าหน้าที่
    staff_notes TEXT,
    rejection_reason TEXT,
    
    -- คะแนน/ผลการสอบ
    interview_score NUMERIC(5,2),
    exam_score NUMERIC(5,2),
    total_score NUMERIC(5,2),                       -- คะแนนรวมสำหรับการจัดอันดับ
    
    submitted_at TIMESTAMPTZ,                        -- เวลาที่ยื่นสมัคร (null = ยังร่าง)
    reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- สร้าง sequence สำหรับ application_number
CREATE SEQUENCE IF NOT EXISTS admission_application_seq START 1;

-- ===================================================================
-- 3. Admission Documents (เอกสารแนบ)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    document_key VARCHAR(100) NOT NULL,             -- birth_cert, house_reg, transcript, photo, etc.
    document_label VARCHAR(200),
    file_url TEXT NOT NULL,
    file_name VARCHAR(500),
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL
);

-- ===================================================================
-- 4. Admission Interviews (การสัมภาษณ์/ทดสอบ)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_interviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    interview_type VARCHAR(50) NOT NULL DEFAULT 'interview'  -- interview | exam | assessment
        CHECK (interview_type IN ('interview', 'exam', 'assessment')),
    scheduled_at TIMESTAMPTZ,
    location VARCHAR(300),
    interviewer_id UUID REFERENCES users(id) ON DELETE SET NULL,
    score NUMERIC(5,2),
    max_score NUMERIC(5,2) DEFAULT 100,
    notes TEXT,
    status VARCHAR(20) DEFAULT 'scheduled'         -- scheduled | completed | cancelled | no_show
        CHECK (status IN ('scheduled', 'completed', 'cancelled', 'no_show')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ===================================================================
-- 5. Admission Selections (รายชื่อผู้ผ่านการคัดเลือก)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_selections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    admission_period_id UUID NOT NULL REFERENCES admission_periods(id) ON DELETE CASCADE,
    selection_type VARCHAR(20) NOT NULL DEFAULT 'main'  -- main | waitlist
        CHECK (selection_type IN ('main', 'waitlist')),
    rank INT,                                           -- ลำดับ
    assigned_grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
    assigned_class_preference VARCHAR(100),             -- ห้องที่ได้รับ (ถ้ากำหนด)
    
    -- ยืนยันสิทธิ์
    is_confirmed BOOLEAN DEFAULT FALSE,
    confirmed_at TIMESTAMPTZ,
    confirmation_deadline TIMESTAMPTZ,
    
    -- หลังยืนยัน → สร้าง student account
    student_user_id UUID REFERENCES users(id) ON DELETE SET NULL,  -- student user ที่สร้าง
    
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(application_id)  -- แต่ละใบสมัครอยู่ในรายชื่อได้เพียง 1 ครั้ง
);

-- ===================================================================
-- 6. Admission Audit Logs (ประวัติการดำเนินการ)
-- ===================================================================
CREATE TABLE IF NOT EXISTS admission_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    action VARCHAR(100) NOT NULL,          -- status_changed, document_uploaded, interview_scheduled, etc.
    old_value TEXT,
    new_value TEXT,
    note TEXT,
    performed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    performed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ===================================================================
-- 7. Indexes
-- ===================================================================
CREATE INDEX IF NOT EXISTS idx_admission_periods_academic_year ON admission_periods(academic_year_id);
CREATE INDEX IF NOT EXISTS idx_admission_periods_status ON admission_periods(status);
CREATE INDEX IF NOT EXISTS idx_admission_applications_period ON admission_applications(admission_period_id);
CREATE INDEX IF NOT EXISTS idx_admission_applications_status ON admission_applications(status);
CREATE INDEX IF NOT EXISTS idx_admission_applications_number ON admission_applications(application_number);
CREATE INDEX IF NOT EXISTS idx_admission_documents_application ON admission_documents(application_id);
CREATE INDEX IF NOT EXISTS idx_admission_interviews_application ON admission_interviews(application_id);
CREATE INDEX IF NOT EXISTS idx_admission_selections_period ON admission_selections(admission_period_id);
CREATE INDEX IF NOT EXISTS idx_admission_selections_application ON admission_selections(application_id);
CREATE INDEX IF NOT EXISTS idx_admission_audit_application ON admission_audit_logs(application_id);

-- ===================================================================
-- 8. Trigger: updated_at
-- ===================================================================
CREATE OR REPLACE FUNCTION update_admission_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'tr_admission_periods_updated_at') THEN
        CREATE TRIGGER tr_admission_periods_updated_at
            BEFORE UPDATE ON admission_periods
            FOR EACH ROW EXECUTE FUNCTION update_admission_updated_at();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'tr_admission_applications_updated_at') THEN
        CREATE TRIGGER tr_admission_applications_updated_at
            BEFORE UPDATE ON admission_applications
            FOR EACH ROW EXECUTE FUNCTION update_admission_updated_at();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'tr_admission_interviews_updated_at') THEN
        CREATE TRIGGER tr_admission_interviews_updated_at
            BEFORE UPDATE ON admission_interviews
            FOR EACH ROW EXECUTE FUNCTION update_admission_updated_at();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'tr_admission_selections_updated_at') THEN
        CREATE TRIGGER tr_admission_selections_updated_at
            BEFORE UPDATE ON admission_selections
            FOR EACH ROW EXECUTE FUNCTION update_admission_updated_at();
    END IF;
END
$$;

COMMENT ON TABLE admission_periods IS 'รอบรับสมัครนักเรียน - กำหนดช่วงเวลาและเป้าหมายการรับ';
COMMENT ON TABLE admission_applications IS 'ใบสมัครนักเรียน - ข้อมูลผู้สมัครและสถานะ';
COMMENT ON TABLE admission_documents IS 'เอกสารแนบประกอบใบสมัคร';
COMMENT ON TABLE admission_interviews IS 'การสัมภาษณ์และทดสอบผู้สมัคร';
COMMENT ON TABLE admission_selections IS 'รายชื่อผู้ผ่านการคัดเลือก';
COMMENT ON TABLE admission_audit_logs IS 'ประวัติการดำเนินการกับใบสมัคร';
