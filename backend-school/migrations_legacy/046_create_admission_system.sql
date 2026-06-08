-- ===================================================================
-- Migration 046: Admission System (ระบบรับสมัครนักเรียน)
-- ===================================================================

-- === ตาราง 1: Admission Rounds (รอบรับสมัคร) ===
CREATE TABLE IF NOT EXISTS admission_rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,

    name VARCHAR(200) NOT NULL,         -- "รับสมัครนักเรียน ม.1 ปีการศึกษา 2569"
    description TEXT,

    -- ช่วงรับสมัคร
    apply_start_date DATE NOT NULL,
    apply_end_date DATE NOT NULL,

    -- ช่วงสอบ
    exam_date DATE,
    -- ช่วงประกาศผล
    result_announce_date DATE,

    -- ช่วงมอบตัว
    enrollment_start_date DATE,
    enrollment_end_date DATE,

    status VARCHAR(30) NOT NULL DEFAULT 'draft',
    -- draft | open | exam | scoring | announced | enrolling | closed

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_admission_rounds_year ON admission_rounds(academic_year_id);
CREATE INDEX IF NOT EXISTS idx_admission_rounds_status ON admission_rounds(status);

-- === ตาราง 2: Exam Subjects (วิชาที่สอบ — ยืดหยุ่น เพิ่ม/ลดได้เอง) ===
CREATE TABLE IF NOT EXISTS admission_exam_subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE CASCADE,

    name VARCHAR(200) NOT NULL,         -- "วิชาคณิตศาสตร์"
    code VARCHAR(50),                   -- "MATH"
    max_score NUMERIC(8,2) NOT NULL DEFAULT 100,
    display_order INT DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_exam_subjects_round ON admission_exam_subjects(admission_round_id);

-- === ตาราง 3: Admission Tracks (สายการเรียนที่รับสมัคร) ===
CREATE TABLE IF NOT EXISTS admission_tracks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE CASCADE,
    study_plan_id UUID NOT NULL REFERENCES study_plans(id) ON DELETE RESTRICT,

    name VARCHAR(200) NOT NULL,         -- "สายวิทย์-คณิต"

    -- capacity_override: NULL = คำนวณจาก class_rooms อัตโนมัติ
    capacity_override INT,

    -- วิชาที่ใช้เรียงคะแนน (array of exam_subject_id)
    -- ให้ user จัดการใน UI
    scoring_subject_ids JSONB DEFAULT '[]'::jsonb,
    -- เช่น: ["uuid-math", "uuid-sci"]

    -- Tie-breaking เมื่อคะแนนเท่ากัน
    tiebreak_method VARCHAR(30) NOT NULL DEFAULT 'applied_at',
    -- 'applied_at' = สมัครก่อนได้ก่อน | 'gpa' = GPA จากโรงเรียนเดิมสูงกว่าได้ก่อน

    display_order INT DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_admission_tracks_round ON admission_tracks(admission_round_id);

-- === ตาราง 4: Applications (ใบสมัคร) ===
CREATE TABLE IF NOT EXISTS admission_applications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE RESTRICT,
    admission_track_id UUID NOT NULL REFERENCES admission_tracks(id) ON DELETE RESTRICT,

    -- เลขที่ใบสมัคร (running per round, e.g. "2569-0001")
    application_number VARCHAR(50) UNIQUE,

    -- ข้อมูลผู้สมัคร
    national_id VARCHAR(13) NOT NULL,
    title VARCHAR(20),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    gender VARCHAR(10),
    date_of_birth DATE,
    phone VARCHAR(20),
    email VARCHAR(200),

    -- ที่อยู่
    address_line TEXT,
    sub_district VARCHAR(100),
    district VARCHAR(100),
    province VARCHAR(100),
    postal_code VARCHAR(10),

    -- ข้อมูลโรงเรียนเดิม
    previous_school VARCHAR(200),
    previous_grade VARCHAR(50),
    previous_gpa NUMERIC(4,2),

    -- ข้อมูลบิดา
    father_name VARCHAR(200),
    father_phone VARCHAR(20),
    father_occupation VARCHAR(100),
    father_national_id VARCHAR(13),

    -- ข้อมูลมารดา
    mother_name VARCHAR(200),
    mother_phone VARCHAR(20),
    mother_occupation VARCHAR(100),
    mother_national_id VARCHAR(13),

    -- ข้อมูลผู้ปกครอง (กรณีไม่ใช่บิดา/มารดา)
    guardian_name VARCHAR(200),
    guardian_phone VARCHAR(20),
    guardian_relation VARCHAR(100),
    guardian_national_id VARCHAR(13),

    -- Status workflow
    status VARCHAR(30) NOT NULL DEFAULT 'submitted',
    -- submitted | verified | rejected | accepted | enrolled | withdrawn

    -- การดำเนินการของครู
    verified_by UUID REFERENCES users(id) ON DELETE SET NULL,
    verified_at TIMESTAMPTZ,
    rejection_reason TEXT,

    -- มอบตัว
    enrolled_by UUID REFERENCES users(id) ON DELETE SET NULL,
    enrolled_at TIMESTAMPTZ,

    -- หลังมอบตัวสำเร็จ: user account ที่สร้างขึ้น
    created_user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    -- ข้อมูลเสริม
    metadata JSONB DEFAULT '{}'::jsonb,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 1 คน สมัครได้ 1 ครั้งต่อรอบ
    CONSTRAINT unique_national_id_per_round UNIQUE(national_id, admission_round_id)
);

CREATE INDEX IF NOT EXISTS idx_applications_national_id ON admission_applications(national_id);
CREATE INDEX IF NOT EXISTS idx_applications_round ON admission_applications(admission_round_id);
CREATE INDEX IF NOT EXISTS idx_applications_track ON admission_applications(admission_track_id);
CREATE INDEX IF NOT EXISTS idx_applications_status ON admission_applications(status);
CREATE INDEX IF NOT EXISTS idx_applications_number ON admission_applications(application_number);

-- === ตาราง 5: Exam Scores (คะแนนสอบ per วิชา) ===
CREATE TABLE IF NOT EXISTS admission_exam_scores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    exam_subject_id UUID NOT NULL REFERENCES admission_exam_subjects(id) ON DELETE CASCADE,

    score NUMERIC(8,2),
    entered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    entered_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT unique_app_subject UNIQUE(application_id, exam_subject_id)
);

CREATE INDEX IF NOT EXISTS idx_exam_scores_application ON admission_exam_scores(application_id);
CREATE INDEX IF NOT EXISTS idx_exam_scores_subject ON admission_exam_scores(exam_subject_id);

-- === ตาราง 6: Room Assignments (ผลการจัดห้อง) ===
CREATE TABLE IF NOT EXISTS admission_room_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    class_room_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE RESTRICT,

    rank_in_track INT,      -- อันดับในสาย (1, 2, 3, ...)
    rank_in_room INT,       -- อันดับในห้อง (1, 2, 3, ...)
    total_score NUMERIC(10,2),   -- คะแนนรวมจากวิชาที่ใช้เรียง
    full_score NUMERIC(10,2),    -- คะแนนรวมทุกวิชา

    assigned_at TIMESTAMPTZ DEFAULT NOW(),
    assigned_by UUID REFERENCES users(id) ON DELETE SET NULL,

    -- นักเรียนยืนยัน
    student_confirmed BOOLEAN NOT NULL DEFAULT false,
    student_confirmed_at TIMESTAMPTZ,

    CONSTRAINT unique_application_assignment UNIQUE(application_id)
);

CREATE INDEX IF NOT EXISTS idx_room_assignments_room ON admission_room_assignments(class_room_id);
CREATE INDEX IF NOT EXISTS idx_room_assignments_application ON admission_room_assignments(application_id);

-- === ตาราง 7: Enrollment Forms (แบบมอบตัว) ===
CREATE TABLE IF NOT EXISTS admission_enrollment_forms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,

    -- ข้อมูลที่กรอกเพิ่มตอนมอบตัว (JSONB เพื่อความยืดหยุ่น)
    -- เช่น: { "shirt_size": "L", "blood_type": "A+", "allergy": "...", ... }
    form_data JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- นักเรียนกรอกล่วงหน้าออนไลน์
    pre_submitted_at TIMESTAMPTZ,
    -- ครูยืนยันที่โรงเรียน (วันมอบตัว)
    completed_at TIMESTAMPTZ,
    completed_by UUID REFERENCES users(id) ON DELETE SET NULL,

    CONSTRAINT unique_enrollment_form UNIQUE(application_id)
);

-- === Triggers for updated_at ===
CREATE TRIGGER update_admission_rounds_updated_at
    BEFORE UPDATE ON admission_rounds
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_admission_applications_updated_at
    BEFORE UPDATE ON admission_applications
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- === Comments ===
COMMENT ON TABLE admission_rounds IS 'รอบรับสมัครนักเรียน ต่อปีการศึกษา';
COMMENT ON TABLE admission_exam_subjects IS 'วิชาที่สอบ — เพิ่ม/ลดได้เอง ต่อรอบ';
COMMENT ON TABLE admission_tracks IS 'สายการเรียนที่รับสมัคร ผูกกับ study_plan';
COMMENT ON TABLE admission_applications IS 'ใบสมัครของผู้สมัคร (1 คน = 1 รอบ)';
COMMENT ON TABLE admission_exam_scores IS 'คะแนนสอบแต่ละวิชาต่อผู้สมัคร';
COMMENT ON TABLE admission_room_assignments IS 'ผลการจัดห้องเรียนหลังเรียงคะแนน';
COMMENT ON TABLE admission_enrollment_forms IS 'แบบฟอร์มมอบตัว';
COMMENT ON COLUMN admission_tracks.scoring_subject_ids IS 'UUID array ของวิชาที่ใช้เรียงคะแนน ต่อสาย';
COMMENT ON COLUMN admission_tracks.capacity_override IS 'NULL = คำนวณจาก class_rooms อัตโนมัติ';
