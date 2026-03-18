-- ===================================================================
-- Migration 049: Extend Admission Application Form
-- เพิ่มข้อมูลในฟอร์มสมัคร: ศาสนา/เชื้อชาติ/สัญชาติ, ที่อยู่ 2 แห่ง,
-- รายได้ผู้ปกครอง, สถานภาพบิดามารดา, เอกสารหลักฐาน
-- ===================================================================

-- === 1. เพิ่ม columns ใน admission_applications ===

ALTER TABLE admission_applications
    -- ข้อมูลส่วนตัว
    ADD COLUMN IF NOT EXISTS religion             VARCHAR(100),
    ADD COLUMN IF NOT EXISTS ethnicity            VARCHAR(100),
    ADD COLUMN IF NOT EXISTS nationality          VARCHAR(100) DEFAULT 'ไทย',

    -- ที่อยู่ตามทะเบียนบ้าน (เพิ่มเติมจาก address_line/sub_district/district/province/postal_code)
    -- address_line, sub_district, district, province, postal_code ยังคงอยู่เป็น home address
    ADD COLUMN IF NOT EXISTS home_house_no        VARCHAR(50),   -- บ้านเลขที่
    ADD COLUMN IF NOT EXISTS home_moo             VARCHAR(20),   -- หมู่ที่
    ADD COLUMN IF NOT EXISTS home_soi             VARCHAR(100),  -- ซอย
    ADD COLUMN IF NOT EXISTS home_road            VARCHAR(100),  -- ถนน
    ADD COLUMN IF NOT EXISTS home_phone           VARCHAR(20),   -- โทรศัพท์

    -- ที่อยู่ปัจจุบัน (ใหม่ทั้งหมด)
    ADD COLUMN IF NOT EXISTS current_house_no     VARCHAR(50),
    ADD COLUMN IF NOT EXISTS current_moo          VARCHAR(20),
    ADD COLUMN IF NOT EXISTS current_soi          VARCHAR(100),
    ADD COLUMN IF NOT EXISTS current_road         VARCHAR(100),
    ADD COLUMN IF NOT EXISTS current_sub_district VARCHAR(100),
    ADD COLUMN IF NOT EXISTS current_district     VARCHAR(100),
    ADD COLUMN IF NOT EXISTS current_province     VARCHAR(100),
    ADD COLUMN IF NOT EXISTS current_postal_code  VARCHAR(10),
    ADD COLUMN IF NOT EXISTS current_phone        VARCHAR(20),

    -- โรงเรียนเดิม
    ADD COLUMN IF NOT EXISTS previous_study_year      VARCHAR(100),  -- เช่น "ประถมศึกษาปีที่ 6"
    ADD COLUMN IF NOT EXISTS previous_school_province VARCHAR(100),

    -- ครอบครัว
    ADD COLUMN IF NOT EXISTS father_income        DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS mother_income        DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS guardian_occupation  VARCHAR(100),
    ADD COLUMN IF NOT EXISTS guardian_income      DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS guardian_is          VARCHAR(10) DEFAULT 'other',
    -- 'father' | 'mother' | 'other'

    ADD COLUMN IF NOT EXISTS parent_status        JSONB DEFAULT '[]'::jsonb,
    -- multi-select: ["อยู่ร่วมกัน", "หย่าร้าง", ...]
    ADD COLUMN IF NOT EXISTS parent_status_other  VARCHAR(200);

COMMENT ON COLUMN admission_applications.home_house_no IS 'บ้านเลขที่ (ที่อยู่ตามทะเบียนบ้าน)';
COMMENT ON COLUMN admission_applications.current_house_no IS 'บ้านเลขที่ (ที่อยู่ปัจจุบัน)';
COMMENT ON COLUMN admission_applications.guardian_is IS 'father | mother | other — ผู้ปกครองคือใคร';
COMMENT ON COLUMN admission_applications.parent_status IS 'JSON array: สถานภาพบิดามารดา (เลือกได้หลายอัน)';


-- === 2. สร้างตาราง admission_application_documents ===

CREATE TABLE IF NOT EXISTS admission_application_documents (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    file_id        UUID NOT NULL REFERENCES files(id) ON DELETE RESTRICT,
    doc_type       VARCHAR(60) NOT NULL,
    -- photo_1_5inch | transcript_por | certificate_por7
    -- id_card_student | id_card_father | id_card_mother | id_card_guardian
    -- house_reg_student | house_reg_father | house_reg_mother | house_reg_guardian
    -- name_change_doc | birth_cert
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at     TIMESTAMPTZ
);

-- Partial unique index: 1 active doc per type per application
CREATE UNIQUE INDEX IF NOT EXISTS uix_app_docs_active
    ON admission_application_documents(application_id, doc_type)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_app_docs_application
    ON admission_application_documents(application_id);

CREATE INDEX IF NOT EXISTS idx_app_docs_file
    ON admission_application_documents(file_id);

COMMENT ON TABLE admission_application_documents IS
    'เอกสารหลักฐานที่แนบกับใบสมัคร (1 ประเภทต่อ 1 ใบสมัคร, soft-delete ได้)';
