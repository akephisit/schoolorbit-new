-- ===================================================================
-- Migration 017: Consent Management System (PDPA Compliance)
-- Description: ระบบจัดการความยินยอมตาม พ.ร.บ. คุ้มครองข้อมูลส่วนบุคคล
-- Date: 2026-01-08
-- ===================================================================

-- ===================================================================
-- 1. Consent Records Table (บันทึกความยินยอม)
-- ===================================================================
CREATE TABLE IF NOT EXISTS consent_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- ผู้ให้ความยินยอม
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_type VARCHAR(50) NOT NULL, -- student, staff, parent
    
    -- ประเภทความยินยอม
    consent_type VARCHAR(100) NOT NULL,
    -- Types: 'data_collection', 'data_usage', 'marketing', 'third_party_sharing', 'medical_records'
    
    -- รายละเอียดความยินยอม
    purpose TEXT NOT NULL, -- วัตถุประสงค์ในการเก็บข้อมูล
    data_categories JSONB NOT NULL DEFAULT '[]', -- ประเภทข้อมูลที่ขอความยินยอม
    
    -- สถานะความยินยอม
    consent_status VARCHAR(20) NOT NULL DEFAULT 'pending', -- 'pending', 'granted', 'denied', 'withdrawn'
    granted_at TIMESTAMPTZ,
    withdrawn_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ, -- วันหมดอายุความยินยอม (NULL = ไม่หมดอายุ)
    
    -- บริบทการให้ความยินยอม
    consent_method VARCHAR(50) NOT NULL DEFAULT 'web_form', -- 'web_form', 'paper_form', 'verbal', 'implicit'
    ip_address INET,
    user_agent TEXT,
    consent_text TEXT, -- ข้อความที่แสดงเมื่อขอความยินยอม
    consent_version VARCHAR(20) NOT NULL DEFAULT '1.0', -- เวอร์ชันของข้อความความยินยอม
    
    -- ความยินยอมสำหรับผู้เยาว์ (Minor Consent)
    is_minor_consent BOOLEAN DEFAULT false, -- ความยินยอมสำหรับผู้เยาว์
    parent_guardian_id UUID REFERENCES users(id), -- ผู้ปกครองที่ให้ความยินยอมแทน
    parent_guardian_name VARCHAR(200), -- ชื่อผู้ปกครอง (สำหรับ audit)
    parent_relationship VARCHAR(50), -- 'father', 'mother', 'guardian'
    
    -- ข้อมูลเพิ่มเติม
    notes TEXT,
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_consent_user_id ON consent_records(user_id);
CREATE INDEX idx_consent_type ON consent_records(consent_type);
CREATE INDEX idx_consent_status ON consent_records(consent_status);
CREATE INDEX idx_consent_user_type ON consent_records(user_id, consent_type);
CREATE INDEX idx_consent_active ON consent_records(user_id, consent_type, consent_status) 
    WHERE consent_status = 'granted' AND (expires_at IS NULL OR expires_at > NOW());
CREATE INDEX idx_consent_parent ON consent_records(parent_guardian_id) WHERE parent_guardian_id IS NOT NULL;

COMMENT ON TABLE consent_records IS 'บันทึกความยินยอมในการเก็บและใช้ข้อมูลส่วนบุคคล (PDPA Compliance - มาตรา 19)';
COMMENT ON COLUMN consent_records.consent_type IS 'ประเภทความยินยอม: data_collection, data_usage, marketing, third_party_sharing';
COMMENT ON COLUMN consent_records.data_categories IS 'ประเภทข้อมูลที่ขอความยินยอม (JSON array): ["personal_info", "contact_info", "medical_info", "financial_info"]';
COMMENT ON COLUMN consent_records.consent_status IS 'สถานะ: pending (รอพิจารณา), granted (อนุญาต), denied (ปฏิเสธ), withdrawn (ถอนคืน)';
COMMENT ON COLUMN consent_records.expires_at IS 'วันหมดอายุความยินยอม (NULL = ไม่มีวันหมดอายุ)';

-- ===================================================================
-- 2. Consent Types Table (ประเภทความยินยอม)
-- ===================================================================
CREATE TABLE IF NOT EXISTS consent_types (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    code VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    name_en VARCHAR(200),
    description TEXT,
    
    -- ความสำคัญ
    is_required BOOLEAN DEFAULT false, -- ความยินยอมที่จำเป็น (ต้องมีจึงใช้บริการได้)
    priority INTEGER DEFAULT 0, -- ลำดับความสำคัญ (ยิ่งสูงยิ่งสำคัญ)
    
    -- กลุ่มผู้ใช้ที่เกี่ยวข้อง
    applicable_user_types TEXT[] NOT NULL DEFAULT ARRAY['student', 'staff', 'parent'], -- ใช้กับผู้ใช้ประเภทไหน
    
    -- ข้อความความยินยอม
    consent_text_template TEXT NOT NULL, -- Template ข้อความขอความยินยอม
    consent_version VARCHAR(20) NOT NULL DEFAULT '1.0',
    
    -- ระยะเวลา
    default_duration_days INTEGER, -- จำนวนวันที่ความยินยอมมีผล (NULL = ไม่มีวันหมดอายุ)
    
    -- สถานะ
    is_active BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_consent_types_code ON consent_types(code);
CREATE INDEX idx_consent_types_required ON consent_types(is_required);
CREATE INDEX idx_consent_types_active ON consent_types(is_active);

COMMENT ON TABLE consent_types IS 'ประเภทความยินยอมที่ระบบรองรับ (Master data)';
COMMENT ON COLUMN consent_types.is_required IS 'ความยินยอมที่จำเป็น (mandatory consent) - ไม่อนุญาตจะใช้บริการไม่ได้';

-- ===================================================================
-- 3. Insert Default Consent Types
-- ===================================================================
INSERT INTO consent_types (code, name, name_en, description, is_required, priority, applicable_user_types, consent_text_template, consent_version, default_duration_days) VALUES

-- ความยินยอมพื้นฐาน (Required)
(
    'data_collection_essential',
    'การเก็บรวบรวมข้อมูลส่วนบุคคลที่จำเป็น',
    'Essential Personal Data Collection',
    'การเก็บข้อมูลส่วนบุคคลที่จำเป็นต่อการให้บริการการศึกษา',
    true, -- Required
    100,
    ARRAY['student', 'staff'],
    'ข้าพเจ้ายินยอมให้โรงเรียนเก็บรวบรวม ใช้ และเปิดเผยข้อมูลส่วนบุคคลของข้าพเจ้า ได้แก่ ชื่อ-นามสกุล เลขบัตรประชาชน ที่อยู่ หมายเลขโทรศัพท์ และอีเมล เพื่อวัตถุประสงค์ในการจัดการเรียนการสอน การติดต่อสื่อสาร และการปฏิบัติตามกฎหมาย',
    '1.0',
    NULL -- ไม่หมดอายุ
),

-- ความยินยอมสำหรับนักเรียน
(
    'student_data_collection',
    'การเก็บข้อมูลนักเรียน',
    'Student Data Collection',
    'การเก็บข้อมูลส่วนบุคคลของนักเรียนเพื่อการจัดการเรียนการสอน',
    true,
    90,
    ARRAY['student'],
    'ข้าพเจ้า (ผู้ปกครอง) ยินยอมให้โรงเรียนเก็บรวบรวมข้อมูลส่วนบุคคลของบุตร/ธิดา รวมถึงข้อมูลการเรียน ผลการเรียน การเข้าชั้นเรียน และข้อมูลอื่นๆ ที่เกี่ยวข้องกับการจัดการศึกษา',
    '1.0',
    NULL
),

-- ข้อมูลสุขภาพ (Sensitive)
(
    'medical_data_collection',
    'การเก็บข้อมูลสุขภาพ',
    'Medical Data Collection',
    'การเก็บข้อมูลสุขภาพที่ละเอียดอ่อน เช่น กรุ๊ปเลือด อาการแพ้ โรคประจำตัว',
    false, -- Optional but recommended
    80,
    ARRAY['student', 'staff'],
    'ข้าพเจ้ายินยอมให้โรงเรียนเก็บรวบรวมข้อมูลสุขภาพของข้าพเจ้า/บุตร-ธิดา ได้แก่ กรุ๊ปเลือด อาการแพ้ยา/อาหาร โรคประจำตัว เพื่อวัตถุประสงค์ในการดูแลสุขภาพและความปลอดภัย',
    '1.0',
    NULL
),

-- การติดต่อสื่อสาร (Optional)
(
    'communication_consent',
    'การติดต่อสื่อสารและการแจ้งข่าวสาร',
    'Communication Consent',
    'การส่งข่าวสาร ประกาศ และกิจกรรมของโรงเรียน',
    false,
    50,
    ARRAY['student', 'staff', 'parent'],
    'ข้าพเจ้ายินยอมให้โรงเรียนติดต่อสื่อสารกับข้าพเจ้าเพื่อแจ้งข่าวสาร ประกาศ กิจกรรม และข้อมูลที่เกี่ยวข้องกับการจัดการศึกษา ผ่านทางอีเมล โทรศัพท์ หรือ LINE',
    '1.0',
    365 -- หมดอายุทุก 1 ปี
),

-- การเผยแพร่ภาพและผลงาน (Optional)
(
    'media_publication_consent',
    'การเผยแพร่ภาพและผลงาน',
    'Media Publication Consent',
    'การเผยแพร่ภาพถ่าย วิดีโอ และผลงานของนักเรียนผ่านช่องทางต่างๆ',
    false,
    40,
    ARRAY['student'],
    'ข้าพเจ้า (ผู้ปกครอง) ยินยอมให้โรงเรียนถ่ายภาพ/วิดีโอ และเผยแพร่ภาพ ผลงาน และกิจกรรมของบุตร/ธิดา ผ่านเว็บไซต์โรงเรียน Facebook Line หรือช่องทางสื่อสารอื่นๆ ของโรงเรียน',
    '1.0',
    365 -- หมดอายุทุก 1 ปี
),

-- การแบ่งปันข้อมูลกับบุคคลที่สาม (Optional)
(
    'third_party_sharing',
    'การแบ่งปันข้อมูลกับบุคคลที่สาม',
    'Third Party Data Sharing',
    'การแบ่งปันข้อมูลกับหน่วยงานภายนอก เช่น สพฐ. กระทรวงศึกษาธิการ',
    false,
    60,
    ARRAY['student', 'staff'],
    'ข้าพเจ้ายินยอมให้โรงเรียนเปิดเผยข้อมูลส่วนบุคคลของข้าพเจ้าให้แก่หน่วยงานราชการที่เกี่ยวข้อง (เช่น สพฐ. กระทรวงศึกษาธิการ) เพื่อวัตถุประสงค์ในการรายงานและปฏิบัติตามกฎหมาย',
    '1.0',
    NULL
)

ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 4. Updated At Triggers
-- ===================================================================
DROP TRIGGER IF EXISTS update_consent_records_updated_at ON consent_records;
CREATE TRIGGER update_consent_records_updated_at
    BEFORE UPDATE ON consent_records
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_consent_types_updated_at ON consent_types;
CREATE TRIGGER update_consent_types_updated_at
    BEFORE UPDATE ON consent_types
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ===================================================================
-- 5. Helper Views
-- ===================================================================

-- View: Active Consents (ความยินยอมที่ใช้งานอยู่)
CREATE OR REPLACE VIEW active_consents AS
SELECT 
    cr.*,
    ct.name as consent_type_name,
    ct.is_required,
    u.first_name,
    u.last_name,
    u.email
FROM consent_records cr
JOIN consent_types ct ON cr.consent_type = ct.code
JOIN users u ON cr.user_id = u.id
WHERE cr.consent_status = 'granted'
  AND (cr.expires_at IS NULL OR cr.expires_at > NOW());

COMMENT ON VIEW active_consents IS 'ความยินยอมที่ใช้งานอยู่ (granted และยังไม่หมดอายุ)';

-- ===================================================================
-- 6. Verify Installation
-- ===================================================================
SELECT 
    code,
    name,
    is_required,
    is_active,
    array_length(applicable_user_types, 1) as user_types_count
FROM consent_types
ORDER BY priority DESC;
