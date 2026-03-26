-- ============================================
-- Activity Groups System (กิจกรรมพัฒนาผู้เรียน)
-- ลูกเสือ, ชุมนุม, แนะแนว, กิจกรรมเพื่อสังคม
-- ============================================

-- 1. Activity Groups (กลุ่มกิจกรรม)
CREATE TABLE IF NOT EXISTS activity_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- ข้อมูลพื้นฐาน
    name VARCHAR(200) NOT NULL,
    description TEXT,
    activity_type VARCHAR(20) NOT NULL CHECK (activity_type IN ('scout', 'club', 'guidance', 'social', 'other')),
    -- scout    = ลูกเสือ/เนตรนารี/ยุวกาชาด
    -- club     = ชุมนุม
    -- guidance = แนะแนว
    -- social   = กิจกรรมเพื่อสังคมและสาธารณประโยชน์
    -- other    = อื่น ๆ

    -- ขอบเขตภาคเรียน
    semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE CASCADE,

    -- ครูที่ดูแล
    instructor_id UUID REFERENCES staff(id) ON DELETE SET NULL,

    -- การรับสมาชิก
    registration_type VARCHAR(10) NOT NULL DEFAULT 'assigned' CHECK (registration_type IN ('self', 'assigned')),
    -- self     = นักเรียนเลือกเอง (ชุมนุม)
    -- assigned = ครู/admin จัดให้ (ลูกเสือ)

    max_capacity INTEGER,           -- NULL = ไม่จำกัด
    registration_open BOOLEAN NOT NULL DEFAULT false,

    -- กำหนดระดับชั้นที่รับ (JSONB array of grade_level_ids)
    -- NULL = รับทุกระดับชั้น
    allowed_grade_level_ids JSONB,

    -- ผู้สร้าง
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,

    -- Metadata
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_activity_groups_semester ON activity_groups(semester_id);
CREATE INDEX idx_activity_groups_instructor ON activity_groups(instructor_id);
CREATE INDEX idx_activity_groups_type ON activity_groups(activity_type);

-- ============================================

-- 2. Activity Group Members (สมาชิกกลุ่มกิจกรรม)
CREATE TABLE IF NOT EXISTS activity_group_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    activity_group_id UUID NOT NULL REFERENCES activity_groups(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,

    -- ผลการเรียน (ผ/มผ) — ใส่ภายหลังปลายภาค
    result VARCHAR(10) CHECK (result IN ('pass', 'fail')),

    -- ผู้เพิ่ม (ครู/admin หรือตัวนักเรียนเอง)
    enrolled_by UUID REFERENCES users(id) ON DELETE SET NULL,
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- ป้องกันนักเรียนอยู่ในกลุ่มเดิมซ้ำ
    CONSTRAINT unique_student_per_group UNIQUE (activity_group_id, student_id)
);

CREATE INDEX idx_activity_members_group ON activity_group_members(activity_group_id);
CREATE INDEX idx_activity_members_student ON activity_group_members(student_id);

-- ============================================

-- 3. เพิ่ม activity_group_id ใน timetable_entries
--    classroom_id เป็น nullable เพื่อรองรับ activity ที่ไม่ผูกกับห้องเรียน

ALTER TABLE academic_timetable_entries
    ADD COLUMN activity_group_id UUID REFERENCES activity_groups(id) ON DELETE CASCADE;

-- classroom_id เดิม NOT NULL → nullable สำหรับ activity entries
ALTER TABLE academic_timetable_entries
    ALTER COLUMN classroom_id DROP NOT NULL;

-- Check: ต้องมีอย่างใดอย่างหนึ่ง (classroom หรือ activity)
ALTER TABLE academic_timetable_entries
    ADD CONSTRAINT timetable_source_check CHECK (
        (classroom_id IS NOT NULL AND activity_group_id IS NULL) OR
        (classroom_id IS NULL AND activity_group_id IS NOT NULL) OR
        -- BREAK/HOMEROOM อาจไม่มีทั้งคู่ถ้า classroom_id ถูก drop
        (entry_type IN ('BREAK', 'HOMEROOM'))
    );

-- Index สำหรับ query ตารางกิจกรรม
CREATE INDEX idx_timetable_activity_group ON academic_timetable_entries(activity_group_id)
    WHERE activity_group_id IS NOT NULL;

-- Unique: กลุ่มกิจกรรมเดียวกันลงซ้ำ slot เดิมไม่ได้
CREATE UNIQUE INDEX unique_activity_group_slot
    ON academic_timetable_entries(activity_group_id, day_of_week, period_id)
    WHERE activity_group_id IS NOT NULL AND is_active = true;

-- ============================================

COMMENT ON TABLE activity_groups IS 'กลุ่มกิจกรรมพัฒนาผู้เรียน: ลูกเสือ ชุมนุม แนะแนว กิจกรรมเพื่อสังคม';
COMMENT ON TABLE activity_group_members IS 'สมาชิกในกลุ่มกิจกรรม พร้อมผลการเรียน ผ/มผ';
COMMENT ON COLUMN activity_groups.registration_type IS 'self=นักเรียนเลือกเอง (ชุมนุม), assigned=ครู/admin จัดให้ (ลูกเสือ)';
COMMENT ON COLUMN activity_groups.allowed_grade_level_ids IS 'JSONB array ของ grade_level_ids ที่รับ, NULL = รับทุกระดับ';
COMMENT ON COLUMN activity_group_members.result IS 'ผลการเรียน: pass=ผ, fail=มผ, NULL=ยังไม่บันทึก';
