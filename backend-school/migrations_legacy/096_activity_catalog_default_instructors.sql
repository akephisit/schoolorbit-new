-- ============================================
-- Activity catalog default instructors
-- Pattern: mirror subject_default_instructors (migration 088)
-- ============================================
-- Admin กำหนดครูเริ่มต้นที่ catalog → Wand2 ตอนสร้าง slot auto-copy:
--   - synchronized mode → activity_slot_instructors (slot-level)
--   - independent mode  → activity_slot_classroom_assignments (per-classroom)
--                         (primary ตัวเดียวถูก copy ให้ทุกห้อง; admin override ได้)
-- ============================================

CREATE TABLE IF NOT EXISTS activity_catalog_default_instructors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    catalog_id UUID NOT NULL REFERENCES activity_catalog(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'primary' CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_catalog_default_instructor UNIQUE (catalog_id, instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_acdi_catalog ON activity_catalog_default_instructors(catalog_id);
CREATE INDEX IF NOT EXISTS idx_acdi_instructor ON activity_catalog_default_instructors(instructor_id);

COMMENT ON TABLE activity_catalog_default_instructors IS
    'Default team teachers per activity catalog version — copied into slot instructors when Wand2 generates a slot';
